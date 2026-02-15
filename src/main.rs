mod features;
mod ingestion;
mod memory;
mod sequences;
mod validation;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use features::extractor::{
    LiquidityEventKind, NormalizationProfile, extract_features_with_stats,
    normalize_feature_with_profile,
};
use ingestion::rpc::{LogFilter, RpcClient, RpcPolicy, TransactionReceipt, extract_tx_hash};
use memory::adaptive::{AdaptiveMemory, MatchOutcome, StabilizationConfig};
use sequences::transition::{SequenceEventKind, TransitionModel};
use validation::baseline::run_validation_set;
use std::cmp::min;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::time::sleep;

#[derive(Debug, Parser)]
#[command(author, version, about = "HAMN liquidity monitor (online runtime)")]
struct Cli {
    #[arg(long, env = "HAMN_RPC_URL")]
    rpc_url: Option<String>,

    #[arg(long, default_value_t = 359_066_951)]
    start_block: u64,

    #[arg(long)]
    end_block: Option<u64>,

    #[arg(long, default_value_t = false)]
    full_tx: bool,

    #[arg(long, default_value_t = false)]
    fetch_logs: bool,

    #[arg(long = "log-topic0")]
    log_topic0: Vec<String>,

    #[arg(long = "log-address")]
    log_address: Vec<String>,

    #[arg(long, default_value_t = 0)]
    receipt_limit: usize,

    #[arg(long, default_value_t = false)]
    extract_features: bool,

    #[arg(long, default_value_t = 18)]
    token0_decimals: u8,

    #[arg(long, default_value_t = 18)]
    token1_decimals: u8,

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

    #[arg(long, default_value_t = 10)]
    topic0_top_n: usize,

    #[arg(long, default_value_t = 100)]
    heartbeat_interval_blocks: u64,

    #[arg(long, default_value_t = 0)]
    snapshot_interval_blocks: u64,

    #[arg(long, value_enum, default_value_t = ErrorMode::FailSoft)]
    error_mode: ErrorMode,

    #[arg(long, default_value_t = false)]
    run_validation_set: bool,

    #[arg(long, default_value = "tests/fixtures/validation_set.json")]
    validation_set_path: String,

    #[arg(long, default_value_t = 0.8)]
    validation_min_precision: f64,

    #[arg(long, default_value_t = 0.8)]
    validation_min_recall: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ErrorMode {
    FailSoft,
    FailFast,
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
    logs_fetched_total: u64,
    blocks_with_logs: u64,
    topic0_counts: HashMap<String, u64>,
    rpc_errors: u64,
    block_fetch_ms_total: u128,
    receipt_fetch_ms_total: u128,
    queue_backpressure_ms_total: u128,
}

#[derive(Debug, Default)]
struct ConsumerStats {
    receipts_processed: u64,
    features_processed: u64,
    recognized_logs: u64,
    unknown_topic_logs: u64,
    malformed_logs: u64,
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

    if cli.run_validation_set {
        let report = run_validation_set(
            &cli.validation_set_path,
            cli.validation_min_precision,
            cli.validation_min_recall,
        )?;
        println!(
            "validation_status=pass cases={} tp={} fp={} fn={} precision_proxy={:.4} recall_proxy={:.4} recognized_logs={} unknown_topic_logs={} malformed_logs={}",
            report.cases,
            report.tp,
            report.fp,
            report.fn_,
            report.precision_proxy,
            report.recall_proxy,
            report.recognized_logs,
            report.unknown_topic_logs,
            report.malformed_logs,
        );
        return Ok(());
    }

    let end_block = match cli.end_block {
        Some(value) => value,
        None if cli.follow => u64::MAX,
        None => anyhow::bail!("end_block is required when --follow is not enabled"),
    };

    if cli.start_block > end_block {
        anyhow::bail!("start_block must be <= end_block");
    }

    let rpc_url = cli
        .rpc_url
        .clone()
        .context("rpc_url is required unless --run-validation-set is used")?;

    let policy = RpcPolicy {
        timeout_ms: cli.rpc_timeout_ms,
        max_retries: cli.rpc_max_retries,
        initial_backoff_ms: cli.rpc_backoff_ms,
        max_backoff_ms: cli.rpc_max_backoff_ms,
    };
    let rpc = RpcClient::new_with_policy(rpc_url, policy);

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
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = shutdown_tx.send(true);
    });

    let should_extract = cli.extract_features || cli.enable_memory || cli.enable_sequences;
    let normalization = NormalizationProfile {
        token0_decimals: cli.token0_decimals,
        token1_decimals: cli.token1_decimals,
    };
    let smoothing_alpha = cli.sequence_smoothing_alpha;
    let periodic_snapshot_path = cli.memory_snapshot_out.clone();
    let snapshot_interval_blocks = cli.snapshot_interval_blocks;
    let consumer_handle = tokio::spawn(async move {
        consume_pipeline(
            rx,
            memory,
            sequences,
            should_extract,
            smoothing_alpha,
            normalization,
            periodic_snapshot_path,
            snapshot_interval_blocks,
        )
        .await
    });

    let runtime_started = Instant::now();
    let mut producer_stats = ProducerStats::default();
    let mut heartbeat_blocks = 0u64;

    let mut next_block = cli.start_block;
    loop {
        if *shutdown_rx.borrow() {
            println!("shutdown_signal_received stage=producer");
            break;
        }
        if next_block > end_block {
            break;
        }

        let target_end = if cli.follow {
            match rpc.get_latest_block_number().await {
                Ok(latest) => min(latest, end_block),
                Err(err) => {
                    producer_stats.rpc_errors = producer_stats.rpc_errors.saturating_add(1);
                    eprintln!("latest_block_error error={err}");
                    tokio::select! {
                        _ = sleep(Duration::from_millis(cli.poll_interval_ms)) => {},
                        _ = wait_for_shutdown_change(&shutdown_rx) => {
                            println!("shutdown_signal_received stage=producer_poll_wait");
                            break;
                        }
                    }
                    continue;
                }
            }
        } else {
            end_block
        };

        if next_block > target_end {
            tokio::select! {
                _ = sleep(Duration::from_millis(cli.poll_interval_ms)) => {},
                _ = wait_for_shutdown_change(&shutdown_rx) => {
                    println!("shutdown_signal_received stage=producer_idle_wait");
                    break;
                }
            }
            continue;
        }

        while next_block <= target_end {
            if *shutdown_rx.borrow() {
                println!("shutdown_signal_received stage=producer_block_loop");
                break;
            }
            let block_lag = target_end.saturating_sub(next_block);
            ingest_block_to_pipeline(
                &rpc,
                &cli,
                next_block,
                block_lag,
                &tx,
                &mut producer_stats,
                cli.error_mode,
            )
            .await?;
            heartbeat_blocks = heartbeat_blocks.saturating_add(1);
            if cli.follow
                && cli.heartbeat_interval_blocks > 0
                && heartbeat_blocks % cli.heartbeat_interval_blocks == 0
            {
                println!(
                    "heartbeat block={} target_end={} lag={} blocks_processed={} receipts_enqueued={} rpc_errors={} queue_capacity={} error_mode={:?}",
                    next_block,
                    target_end,
                    block_lag,
                    producer_stats.blocks_processed,
                    producer_stats.receipts_enqueued,
                    producer_stats.rpc_errors,
                    queue_capacity,
                    cli.error_mode
                );
            }
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
    print_topic0_summary(&producer_stats, cli.topic0_top_n);

    Ok(())
}

async fn ingest_block_to_pipeline(
    rpc: &RpcClient,
    cli: &Cli,
    block_number: u64,
    block_lag: u64,
    tx: &mpsc::Sender<PipelineMessage>,
    stats: &mut ProducerStats,
    error_mode: ErrorMode,
) -> Result<()> {
    let block_started = Instant::now();
    let block = match rpc.get_block_by_number(block_number, cli.full_tx).await {
        Ok(block) => block,
        Err(err) => {
            stats.rpc_errors = stats.rpc_errors.saturating_add(1);
            eprintln!("block_fetch_error block={} error={}", block_number, err);
            if error_mode == ErrorMode::FailFast {
                return Err(err).context("fail-fast: block fetch error");
            }
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
        let log_filter = if cli.log_topic0.is_empty() && cli.log_address.is_empty() {
            None
        } else {
            Some(LogFilter {
                addresses: cli.log_address.clone(),
                topic0: cli.log_topic0.clone(),
            })
        };

        let logs_result = if let Some(filter) = log_filter.as_ref() {
            rpc.get_logs_filtered(block_number, block_number, Some(filter))
                .await
        } else {
            rpc.get_logs(block_number, block_number).await
        };

        match logs_result {
            Ok(logs) => {
                stats.logs_fetched_total = stats
                    .logs_fetched_total
                    .saturating_add(logs.len() as u64);
                if !logs.is_empty() {
                    stats.blocks_with_logs = stats.blocks_with_logs.saturating_add(1);
                }
                for log in &logs {
                    if let Some(topic0) = log.topics.first() {
                        let key = topic0.to_lowercase();
                        let count = stats.topic0_counts.entry(key).or_insert(0);
                        *count = count.saturating_add(1);
                    }
                }
                println!(
                    "logs_range={}..={} total_logs={} topic0_filters={} address_filters={}",
                    block_number,
                    block_number,
                    logs.len(),
                    cli.log_topic0.len(),
                    cli.log_address.len(),
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
                if error_mode == ErrorMode::FailFast {
                    return Err(err).context("fail-fast: logs fetch error");
                }
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
                    if error_mode == ErrorMode::FailFast {
                        return Err(err).context("fail-fast: receipt fetch error");
                    }
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
    normalization: NormalizationProfile,
    periodic_snapshot_path: Option<String>,
    snapshot_interval_blocks: u64,
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
                    let (features, extraction_stats) = extract_features_with_stats(&receipt);
                    println!("features_extracted={}", features.len());
                    stats.features_processed = stats
                        .features_processed
                        .saturating_add(features.len() as u64);
                    stats.recognized_logs = stats
                        .recognized_logs
                        .saturating_add(extraction_stats.recognized_logs);
                    stats.unknown_topic_logs = stats
                        .unknown_topic_logs
                        .saturating_add(extraction_stats.unknown_topic_logs);
                    stats.malformed_logs = stats
                        .malformed_logs
                        .saturating_add(extraction_stats.malformed_logs);
                    println!(
                        "feature_extraction_stats logs_total={} recognized={} unknown_topic={} malformed={}",
                        extraction_stats.logs_total,
                        extraction_stats.recognized_logs,
                        extraction_stats.unknown_topic_logs,
                        extraction_stats.malformed_logs,
                    );

                    for feature in features {
                        let normalized = normalize_feature_with_profile(feature, normalization);
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

                if snapshot_interval_blocks > 0
                    && block_number % snapshot_interval_blocks == 0
                    && periodic_snapshot_path.is_some()
                {
                    if let (Some(memory), Some(path)) =
                        (memory.as_ref(), periodic_snapshot_path.as_ref())
                    {
                        memory
                            .save_snapshot(path)
                            .with_context(|| format!("failed periodic snapshot at block={block_number}"))?;
                        println!(
                            "memory_snapshot_saved mode=periodic block={} path={} patterns={}",
                            block_number,
                            path,
                            memory.pattern_count()
                        );
                    }
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
    let logs_per_block = if producer.blocks_processed == 0 {
        0.0
    } else {
        producer.logs_fetched_total as f64 / producer.blocks_processed as f64
    };
    let log_hit_ratio = if producer.blocks_processed == 0 {
        0.0
    } else {
        producer.blocks_with_logs as f64 / producer.blocks_processed as f64
    };
    let features_per_block = if producer.blocks_processed == 0 {
        0.0
    } else {
        consumer.features_processed as f64 / producer.blocks_processed as f64
    };
    let feature_hit_ratio = if producer.receipts_enqueued == 0 {
        0.0
    } else {
        consumer.features_processed as f64 / producer.receipts_enqueued as f64
    };

    println!(
        "runtime_metrics elapsed_s={:.3} blocks={} receipts_in={} receipts_processed={} features={} recognized_logs={} unknown_topic_logs={} malformed_logs={} throughput_rps={:.3} avg_block_fetch_ms={:.3} avg_receipt_fetch_ms={:.3} avg_queue_backpressure_ms={:.3} avg_queue_latency_ms={:.3} avg_processing_ms={:.3} logs_total={} logs_per_block={:.3} log_hit_ratio={:.3} features_per_block={:.3} feature_hit_ratio={:.3} rpc_errors={} max_block_lag={}",
        elapsed_secs,
        producer.blocks_processed,
        producer.receipts_enqueued,
        consumer.receipts_processed,
        consumer.features_processed,
        consumer.recognized_logs,
        consumer.unknown_topic_logs,
        consumer.malformed_logs,
        throughput_rps,
        avg_block_fetch_ms,
        avg_receipt_fetch_ms,
        avg_queue_backpressure_ms,
        avg_queue_latency_ms,
        avg_processing_ms,
        producer.logs_fetched_total,
        logs_per_block,
        log_hit_ratio,
        features_per_block,
        feature_hit_ratio,
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

fn print_topic0_summary(producer: &ProducerStats, top_n: usize) {
    if producer.topic0_counts.is_empty() || top_n == 0 {
        return;
    }

    let mut rows: Vec<(&String, &u64)> = producer.topic0_counts.iter().collect();
    rows.sort_by(|a, b| b.1.cmp(a.1));

    for (rank, (topic, count)) in rows.into_iter().take(top_n).enumerate() {
        println!(
            "topic0_top rank={} count={} topic={}",
            rank + 1,
            count,
            topic
        );
    }
}

async fn wait_for_shutdown_change(shutdown_rx: &watch::Receiver<bool>) {
    let mut rx = shutdown_rx.clone();
    while !*rx.borrow() {
        if rx.changed().await.is_err() {
            break;
        }
    }
}
