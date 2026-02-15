mod features;
mod ingestion;
mod memory;
mod sequences;

use anyhow::Result;
use clap::Parser;
use features::extractor::{LiquidityEventKind, extract_features_from_receipt, normalize_feature};
use ingestion::rpc::{RpcClient, RpcPolicy, extract_tx_hash};
use memory::adaptive::{AdaptiveMemory, MatchOutcome, StabilizationConfig};
use sequences::transition::{SequenceEventKind, TransitionModel};
use std::cmp::min;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Parser)]
#[command(author, version, about = "HAMN liquidity monitor (Stage 1 ingestion)")]
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
    let mut memory = if cli.enable_memory {
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
    let mut sequences = if cli.enable_sequences {
        Some(TransitionModel::new())
    } else {
        None
    };

    let mut next_block = cli.start_block;
    loop {
        if next_block > end_block {
            break;
        }

        let target_end = if cli.follow {
            let latest = rpc.get_latest_block_number().await?;
            min(latest, end_block)
        } else {
            end_block
        };

        if next_block > target_end {
            sleep(Duration::from_millis(cli.poll_interval_ms)).await;
            continue;
        }

        while next_block <= target_end {
            process_block(&rpc, &cli, next_block, &mut memory, &mut sequences).await?;
            if let Some(memory) = memory.as_mut() {
                let pruned = memory.stabilize(next_block);
                let metrics = memory.metrics();
                println!(
                    "memory_metrics block={} active_patterns={} match_ratio={:.4} churn_rate={:.4} pruned_now={} total_obs={} matched={} created={} pruned_total={}",
                    next_block,
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
                    cli.sequence_smoothing_alpha,
                );
                let add_to_remove = sequences.smoothed_probability(
                    SequenceEventKind::AddLiquidity,
                    SequenceEventKind::RemoveLiquidity,
                    cli.sequence_smoothing_alpha,
                );
                println!(
                    "sequence_metrics block={} entities={} total_transitions={} unique_transitions={} p_swap_add_raw={:.4} p_swap_add={:.4} p_add_remove={:.4}",
                    next_block,
                    metrics.tracked_entities,
                    metrics.total_transitions,
                    metrics.unique_transitions,
                    raw_swap_to_add,
                    swap_to_add,
                    add_to_remove,
                );
            }
            next_block += 1;
        }

        if !cli.follow {
            break;
        }
    }

    if let (Some(memory), Some(path)) = (&memory, &cli.memory_snapshot_out) {
        memory.save_snapshot(path)?;
        println!(
            "memory_snapshot_saved path={} patterns={}",
            path,
            memory.pattern_count()
        );
    }

    Ok(())
}

async fn process_block(
    rpc: &RpcClient,
    cli: &Cli,
    block_number: u64,
    memory: &mut Option<AdaptiveMemory>,
    sequences: &mut Option<TransitionModel>,
) -> Result<()> {
    let mut transaction_hashes = Vec::new();
    let block = rpc.get_block_by_number(block_number, cli.full_tx).await?;
    for tx in &block.transactions {
        if let Some(hash) = extract_tx_hash(tx) {
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
        let logs = rpc.get_logs(block_number, block_number).await?;
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

    if cli.receipt_limit > 0 {
        let mut fetched = 0usize;
        for tx_hash in transaction_hashes.into_iter().take(cli.receipt_limit) {
            if let Some(receipt) = rpc.get_transaction_receipt(&tx_hash).await? {
                println!(
                    "receipt tx={} block={} gas_used={} status={} logs={}",
                    receipt.transaction_hash,
                    receipt.block_number,
                    receipt.gas_used,
                    receipt.status,
                    receipt.logs.len(),
                );

                if cli.extract_features || memory.is_some() || sequences.is_some() {
                    let features = extract_features_from_receipt(&receipt);
                    println!("features_extracted={}", features.len());
                    for feature in features {
                        let normalized = normalize_feature(feature);
                        let event_label = match normalized.feature.event_kind {
                            LiquidityEventKind::Swap => "swap",
                            LiquidityEventKind::AddLiquidity => "add_liquidity",
                            LiquidityEventKind::RemoveLiquidity => "remove_liquidity",
                        };
                        println!(
                            "feature event={} tx={} pool={} volume_ln={:.6} imbalance={:.6} gas_ln={:.6}",
                            event_label,
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
                            let from_to_event = SequenceEventKind::from_liquidity_event(
                                &normalized.feature.event_kind,
                            );
                            let entity_key = normalized.feature.pool_address.clone();
                            sequences.observe_entity_event(entity_key, from_to_event);
                            println!(
                                "sequence_observe event={} key={}",
                                from_to_event.as_str(),
                                normalized.feature.pool_address
                            );
                        }
                    }
                }
                fetched += 1;
            }
        }
        println!("receipts_fetched={fetched}");
    }

    Ok(())
}
