mod features;
mod ingestion;
mod memory;
mod sequences;

use anyhow::{Context, Result};
use clap::Parser;
use features::extractor::{LiquidityEventKind, extract_features_from_receipt, normalize_feature};
use ingestion::rpc::{RpcClient, RpcPolicy, TransactionReceipt, extract_tx_hash};
use memory::adaptive::{AdaptiveMemory, MatchOutcome, StabilizationConfig};
use sequences::transition::{SequenceEventKind, TransitionModel};
use std::cmp::min;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;

#[derive(Debug, Parser)]
#[command(author, version, about = "HAMN liquidity monitor (online runtime)")]
struct Cli {
    #[arg(long, env = "HAMN_RPC_URL")]
    rpc_url: String,

    #[arg(long)]
    start_block: u64,

    #[arg(long)]
    end_block: Option<u64>,

    #[arg(long, default_value_t = false)]
    full_tx: bool,

    #[arg(long, default_value_t = false)]
    fetch_logs: bool,

    #[arg(long, default_value_t = 0)]
    receipt_limit: usize,

    #[arg(long, default_value_t = false)]
    extract_features: bool,

    #[arg(long, default_value_t = false)]
    enable_sequences: bool,

    #[arg(long, default_value_t = 0.25)]
    sequence_smoothing_alpha: f64,

    #[arg(long, default_value_t = false)]
    enable_memory: bool,

    #[arg(long, default_value_t = 0.35)]
    memory_distance_threshold: f64,

    #[arg(long, default_value_t = 0.999)]
    memory_decay_per_block: f64,

    #[arg(long, default_value_t = 0.08)]
    memory_min_confidence: f64,

    #[arg(long, default_value_t = 50_000)]
    memory_max_inactive_blocks: u64,

    #[arg(long, default_value_t = 2)]
    memory_min_occurrences_for_retention: u64,

    #[arg(long, default_value_t = 2_000)]
    memory_noise_inactive_blocks: u64,

    #[arg(long, default_value_t = 50_000)]
    memory_max_patterns: usize,

    #[arg(long)]
    memory_snapshot_in: Option<String>,

    #[arg(long)]
    memory_snapshot_out: Option<String>,

    #[arg(long, default_value_t = 128)]
    pipeline_queue_capacity: usize,

    #[arg(long, default_value_t = 10_000)]
    rpc_timeout_ms: u64,

    #[arg(long, default_value_t = 3)]
    rpc_max_retries: u32,

    #[arg(long, default_value_t = 250)]
    rpc_backoff_ms: u64,

    #[arg(long, default_value_t = 2_000)]
    rpc_max_backoff_ms: u64,

    #[arg(long, default_value_t = false)]
    follow: bool,

    #[arg(long, default_value_t = 2_000)]
    poll_interval_ms: u64,
}

#[derive(Debug)]
enum PipelineMessage {
    Receipt {
        block_number: u64,
        enqueued_at: Instant,
        receipt: TransactionReceipt,
    },
    BlockBoundary {
        block_number: u64,
        block_lag: u64,
    },
}

#[derive(Debug, Default)]
struct ProducerStats {
    blocks_processed: u64,
    receipts_enqueued: u64,
    rpc_errors: u64,
    block_fetch_ms_total: u128,
    receipt_fetch_ms_total: u128,
    queue_backpressure_ms_total: u128,
}

#[derive(Debug, Default)]
struct ConsumerStats {
    receipts_processed: u64,
    features_processed: u64,
    processing_ms_total: u128,
    queue_latency_ms_total: u128,
    max_block_lag: u64,
}

struct ConsumerOutput {
    memory: Option<AdaptiveMemory>,
    stats: ConsumerStats,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let end_block = match cli.end_block {
        Some(value) => value,
        None if cli.follow => u64::MAX,
        None => anyhow::bail!("end_block is required when --follow is not enabled"),
    };

    if cli.start_block > end_block {
        anyhow::bail!("start_block must be <= end_block");
    }

    let policy = RpcPolicy {
        timeout_ms: cli.rpc_timeout_ms,
        max_retries: cli.rpc_max_retries,
        initial_backoff_ms: cli.rpc_backoff_ms,
        max_backoff_ms: cli.rpc_max_backoff_ms,
    };
    let rpc = RpcClient::new_with_policy(cli.rpc_url.clone(), policy);

    let stabilization = StabilizationConfig {
        confidence_decay_per_block: cli.memory_decay_per_block,
        min_confidence: cli.memory_min_confidence,
        max_inactive_blocks: cli.memory_max_inactive_blocks,
        min_occurrences_for_retention: cli.memory_min_occurrences_for_retention,
        noise_inactive_blocks: cli.memory_noise_inactive_blocks,
        max_patterns: cli.memory_max_patterns,
    };

    let memory = if cli.enable_memory {
        if let Some(path) = &cli.memory_snapshot_in {
            Some(AdaptiveMemory::load_snapshot_with_config(
                path,
                cli.memory_distance_threshold,
                stabilization,
            )?)
        } else {
            Some(AdaptiveMemory::new_with_config(
                cli.memory_distance_threshold,
                stabilization,
            ))
        }
    } else {
        None
    };

    let sequences = if cli.enable_sequences {
        Some(TransitionModel::new())
    } else {
        None
    };

    let queue_capacity = cli.pipeline_queue_capacity.max(1);
    let (tx, rx) = mpsc::channel::<PipelineMessage>(queue_capacity);

    let should_extract = cli.extract_features || cli.enable_memory || cli.enable_sequences;
    let smoothing_alpha = cli.sequence_smoothing_alpha;
    let consumer_handle = tokio::spawn(async move {
        consume_pipeline(rx, memory, sequences, should_extract, smoothing_alpha).await
    });

    let runtime_started = Instant::now();
    let mut producer_stats = ProducerStats::default();

    let mut next_block = cli.start_block;
    loop {
        if next_block > end_block {
            break;
        }

        let target_end = if cli.follow {
            match rpc.get_latest_block_number().await {
                Ok(latest) => min(latest, end_block),
                Err(err) => {
                    producer_stats.rpc_errors = producer_stats.rpc_errors.saturating_add(1);
                    eprintln!("latest_block_error error={err}");
                    sleep(Duration::from_millis(cli.poll_interval_ms)).await;
                    continue;
                }
            }
        } else {
            end_block
        };

        if next_block > target_end {
            sleep(Duration::from_millis(cli.poll_interval_ms)).await;
            continue;
        }

        while next_block <= target_end {
            let block_lag = target_end.saturating_sub(next_block);
            ingest_block_to_pipeline(
                &rpc,
                &cli,
                next_block,
                block_lag,
                &tx,
                &mut producer_stats,
            )
            .await?;
            next_block += 1;
        }

        if !cli.follow {
            break;
        }
    }

    drop(tx);

    let consumer_output = consumer_handle
        .await
        .context("pipeline consumer task join failed")??;

    if let (Some(memory), Some(path)) = (&consumer_output.memory, &cli.memory_snapshot_out) {
        memory.save_snapshot(path)?;
        println!(
            "memory_snapshot_saved path={} patterns={}",
            path,
            memory.pattern_count()
        );
    }

    print_runtime_metrics(runtime_started.elapsed(), &producer_stats, &consumer_output.stats);

    Ok(())
}

async fn ingest_block_to_pipeline(
    rpc: &RpcClient,
    cli: &Cli,
    block_number: u64,
    block_lag: u64,
    tx: &mpsc::Sender<PipelineMessage>,
    stats: &mut ProducerStats,
) -> Result<()> {
    let block_started = Instant::now();
    let block = match rpc.get_block_by_number(block_number, cli.full_tx).await {
        Ok(block) => block,
        Err(err) => {
            stats.rpc_errors = stats.rpc_errors.saturating_add(1);
            eprintln!("block_fetch_error block={} error={}", block_number, err);
            return Ok(());
        }
    };
    stats.blocks_processed = stats.blocks_processed.saturating_add(1);
    stats.block_fetch_ms_total = stats
        .block_fetch_ms_total
        .saturating_add(block_started.elapsed().as_millis());

    let mut transaction_hashes = Vec::new();
    for tx_item in &block.transactions {
        if let Some(hash) = extract_tx_hash(tx_item) {
            transaction_hashes.push(hash.to_owned());
        }
    }

    println!(
        "block={} hash={} parent={} txs={} timestamp={}",
        block.number,
        block.hash.as_deref().unwrap_or("<none>"),
        block.parent_hash,
        block.transactions.len(),
        block.timestamp,
    );

    if cli.fetch_logs {
        match rpc.get_logs(block_number, block_number).await {
            Ok(logs) => {
                println!(
                    "logs_range={}..={} total_logs={}",
                    block_number,
                    block_number,
                    logs.len(),
                );
                if let Some(first_log) = logs.first() {
                    println!(
                        "first_log block={} tx={} address={} topics={} data_len={}",
                        first_log.block_number,
                        first_log.transaction_hash,
                        first_log.address,
                        first_log.topics.len(),
                        first_log.data.len(),
                    );
                }
            }
            Err(err) => {
                stats.rpc_errors = stats.rpc_errors.saturating_add(1);
                eprintln!("logs_fetch_error block={} error={}", block_number, err);
            }
        }
    }

    if cli.receipt_limit > 0 {
        let mut fetched = 0usize;
        for tx_hash in transaction_hashes.into_iter().take(cli.receipt_limit) {
            let receipt_started = Instant::now();
            let receipt_opt = match rpc.get_transaction_receipt(&tx_hash).await {
                Ok(value) => value,
                Err(err) => {
                    stats.rpc_errors = stats.rpc_errors.saturating_add(1);
                    eprintln!("receipt_fetch_error tx={} error={}", tx_hash, err);
                    continue;
                }
            };
            stats.receipt_fetch_ms_total = stats
                .receipt_fetch_ms_total
                .saturating_add(receipt_started.elapsed().as_millis());

            if let Some(receipt) = receipt_opt {
                println!(
                    "receipt tx={} block={} gas_used={} status={} logs={}",
                    receipt.transaction_hash,
                    receipt.block_number,
                    receipt.gas_used,
                    receipt.status,
                    receipt.logs.len(),
                );

                let enqueued_at = Instant::now();
                let send_wait_started = Instant::now();
                tx.send(PipelineMessage::Receipt {
                    block_number,
                    enqueued_at,
                    receipt,
                })
                .await
                .context("failed to enqueue receipt into pipeline")?;
                stats.queue_backpressure_ms_total = stats
                    .queue_backpressure_ms_total
                    .saturating_add(send_wait_started.elapsed().as_millis());
                stats.receipts_enqueued = stats.receipts_enqueued.saturating_add(1);
                fetched += 1;
            }
        }
        println!("receipts_fetched={fetched}");
    }

    tx.send(PipelineMessage::BlockBoundary {
        block_number,
        block_lag,
    })
    .await
    .context("failed to enqueue block boundary")?;

    Ok(())
}

async fn consume_pipeline(
    mut rx: mpsc::Receiver<PipelineMessage>,
    mut memory: Option<AdaptiveMemory>,
    mut sequences: Option<TransitionModel>,
    should_extract_features: bool,
    smoothing_alpha: f64,
) -> Result<ConsumerOutput> {
    let mut stats = ConsumerStats::default();

    while let Some(message) = rx.recv().await {
        match message {
            PipelineMessage::Receipt {
                block_number,
                enqueued_at,
                receipt,
            } => {
                stats.receipts_processed = stats.receipts_processed.saturating_add(1);
                stats.queue_latency_ms_total = stats
                    .queue_latency_ms_total
                    .saturating_add(enqueued_at.elapsed().as_millis());

                if should_extract_features {
                    let started = Instant::now();
                    let features = extract_features_from_receipt(&receipt);
                    println!("features_extracted={}", features.len());
                    stats.features_processed = stats
                        .features_processed
                        .saturating_add(features.len() as u64);

                    for feature in features {
                        let normalized = normalize_feature(feature);
                        println!(
                            "feature event={} tx={} pool={} volume_ln={:.6} imbalance={:.6} gas_ln={:.6}",
                            event_label(&normalized.feature.event_kind),
                            normalized.feature.tx_hash,
                            normalized.feature.pool_address,
                            normalized.normalized_total_volume,
                            normalized.normalized_imbalance,
                            normalized.normalized_gas,
                        );

                        if let Some(memory) = memory.as_mut() {
                            match memory.observe(&normalized) {
                                MatchOutcome::Matched {
                                    pattern_id,
                                    distance,
                                    confidence,
                                } => {
                                    println!(
                                        "memory matched pattern_id={} distance={:.6} confidence={:.4}",
                                        pattern_id, distance, confidence
                                    );
                                }
                                MatchOutcome::Created { pattern_id } => {
                                    println!("memory created pattern_id={}", pattern_id);
                                }
                            }
                        }

                        if let Some(sequences) = sequences.as_mut() {
                            let event =
                                SequenceEventKind::from_liquidity_event(&normalized.feature.event_kind);
                            sequences.observe_entity_event(normalized.feature.pool_address.clone(), event);
                            println!(
                                "sequence_observe block={} event={} key={}",
                                block_number,
                                event.as_str(),
                                normalized.feature.pool_address,
                            );
                        }
                    }

                    stats.processing_ms_total = stats
                        .processing_ms_total
                        .saturating_add(started.elapsed().as_millis());
                }
            }
            PipelineMessage::BlockBoundary {
                block_number,
                block_lag,
            } => {
                stats.max_block_lag = stats.max_block_lag.max(block_lag);

                if let Some(memory) = memory.as_mut() {
                    let pruned = memory.stabilize(block_number);
                    let metrics = memory.metrics();
                    println!(
                        "memory_metrics block={} active_patterns={} match_ratio={:.4} churn_rate={:.4} pruned_now={} total_obs={} matched={} created={} pruned_total={}",
                        block_number,
                        metrics.active_patterns,
                        metrics.match_ratio,
                        metrics.churn_rate,
                        pruned,
                        metrics.total_observations,
                        metrics.matched_observations,
                        metrics.created_observations,
                        metrics.pruned_patterns_total,
                    );
                }

                if let Some(sequences) = sequences.as_ref() {
                    let metrics = sequences.metrics();
                    let raw_swap_to_add = sequences.transition_probability(
                        SequenceEventKind::Swap,
                        SequenceEventKind::AddLiquidity,
                    );
                    let swap_to_add = sequences.smoothed_probability(
                        SequenceEventKind::Swap,
                        SequenceEventKind::AddLiquidity,
                        smoothing_alpha,
                    );
                    let add_to_remove = sequences.smoothed_probability(
                        SequenceEventKind::AddLiquidity,
                        SequenceEventKind::RemoveLiquidity,
                        smoothing_alpha,
                    );
                    println!(
                        "sequence_metrics block={} entities={} total_transitions={} unique_transitions={} p_swap_add_raw={:.4} p_swap_add={:.4} p_add_remove={:.4}",
                        block_number,
                        metrics.tracked_entities,
                        metrics.total_transitions,
                        metrics.unique_transitions,
                        raw_swap_to_add,
                        swap_to_add,
                        add_to_remove,
                    );
                }
            }
        }
    }

    Ok(ConsumerOutput {
        memory,
        stats,
    })
}

fn print_runtime_metrics(elapsed: Duration, producer: &ProducerStats, consumer: &ConsumerStats) {
    let elapsed_secs = elapsed.as_secs_f64().max(0.001);

    let avg_block_fetch_ms = if producer.blocks_processed == 0 {
        0.0
    } else {
        producer.block_fetch_ms_total as f64 / producer.blocks_processed as f64
    };

    let avg_receipt_fetch_ms = if producer.receipts_enqueued == 0 {
        0.0
    } else {
        producer.receipt_fetch_ms_total as f64 / producer.receipts_enqueued as f64
    };

    let avg_queue_backpressure_ms = if producer.receipts_enqueued == 0 {
        0.0
    } else {
        producer.queue_backpressure_ms_total as f64 / producer.receipts_enqueued as f64
    };

    let avg_processing_ms = if consumer.receipts_processed == 0 {
        0.0
    } else {
        consumer.processing_ms_total as f64 / consumer.receipts_processed as f64
    };

    let avg_queue_latency_ms = if consumer.receipts_processed == 0 {
        0.0
    } else {
        consumer.queue_latency_ms_total as f64 / consumer.receipts_processed as f64
    };

    let throughput_rps = consumer.receipts_processed as f64 / elapsed_secs;

    println!(
        "runtime_metrics elapsed_s={:.3} blocks={} receipts_in={} receipts_processed={} features={} throughput_rps={:.3} avg_block_fetch_ms={:.3} avg_receipt_fetch_ms={:.3} avg_queue_backpressure_ms={:.3} avg_queue_latency_ms={:.3} avg_processing_ms={:.3} rpc_errors={} max_block_lag={}",
        elapsed_secs,
        producer.blocks_processed,
        producer.receipts_enqueued,
        consumer.receipts_processed,
        consumer.features_processed,
        throughput_rps,
        avg_block_fetch_ms,
        avg_receipt_fetch_ms,
        avg_queue_backpressure_ms,
        avg_queue_latency_ms,
        avg_processing_ms,
        producer.rpc_errors,
        consumer.max_block_lag,
    );
}

fn event_label(kind: &LiquidityEventKind) -> &'static str {
    match kind {
        LiquidityEventKind::Swap => "swap",
        LiquidityEventKind::AddLiquidity => "add_liquidity",
        LiquidityEventKind::RemoveLiquidity => "remove_liquidity",
    }
}
