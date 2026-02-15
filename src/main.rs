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
use serde_json::{Map, Value, json};
use validation::baseline::run_validation_set;
use std::cmp::min;
use std::collections::HashMap;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;
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

    #[arg(long)]
    features_out: Option<String>,

    #[arg(long, default_value_t = 0)]
    features_out_rotate_records: u64,

    #[arg(long, default_value_t = 18)]
    token0_decimals: u8,

    #[arg(long, default_value_t = 18)]
    token1_decimals: u8,

    #[arg(long, default_value_t = false)]
    enable_alerts: bool,

    #[arg(long, default_value_t = 1.0)]
    alert_min_volume_ln: f64,

    #[arg(long, default_value_t = 0.2)]
    alert_min_abs_imbalance: f64,

    #[arg(long, default_value_t = 20_000.0)]
    alert_min_gas_used: f64,

    #[arg(long, default_value_t = 10.8)]
    alert_min_gas_ln_swap_spike: f64,

    #[arg(long, default_value_t = 20)]
    alert_cooldown_blocks: u64,

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

    #[arg(long, value_enum, default_value_t = LogFormat::Text)]
    log_format: LogFormat,

    #[arg(long, default_value_t = false)]
    emit_metrics_json: bool,

    #[arg(long, default_value_t = false)]
    skip_preflight: bool,

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum LogFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy)]
struct OutputConfig {
    log_format: LogFormat,
    emit_metrics_json: bool,
}

#[derive(Debug, Clone, Copy)]
enum AlertSeverity {
    Warning,
    Critical,
}

impl AlertSeverity {
    fn as_str(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct AlertConfig {
    enabled: bool,
    min_volume_ln: f64,
    min_abs_imbalance: f64,
    min_gas_used: f64,
    min_gas_ln_swap_spike: f64,
    cooldown_blocks: u64,
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
    alerts_emitted: u64,
    alerts_warning: u64,
    alerts_critical: u64,
    alerts_rule_high_imbalance_high_volume: u64,
    alerts_rule_swap_gas_spike: u64,
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
    let output = OutputConfig {
        log_format: cli.log_format,
        emit_metrics_json: cli.emit_metrics_json,
    };

    if cli.run_validation_set {
        let report = run_validation_set(
            &cli.validation_set_path,
            cli.validation_min_precision,
            cli.validation_min_recall,
        )?;
        emit_event(
            &output,
            "validation_status",
            vec![
                ("status", json!("pass")),
                ("cases", json!(report.cases)),
                ("tp", json!(report.tp)),
                ("fp", json!(report.fp)),
                ("fn", json!(report.fn_)),
                ("precision_proxy", json!(round_to(report.precision_proxy, 6))),
                ("recall_proxy", json!(round_to(report.recall_proxy, 6))),
                ("recognized_logs", json!(report.recognized_logs)),
                ("unknown_topic_logs", json!(report.unknown_topic_logs)),
                ("malformed_logs", json!(report.malformed_logs)),
            ],
        );
        emit_metric(
            &output,
            "validation_metrics",
            vec![
                ("cases", json!(report.cases)),
                ("tp", json!(report.tp)),
                ("fp", json!(report.fp)),
                ("fn", json!(report.fn_)),
                ("precision_proxy", json!(round_to(report.precision_proxy, 6))),
                ("recall_proxy", json!(round_to(report.recall_proxy, 6))),
                ("recognized_logs", json!(report.recognized_logs)),
                ("unknown_topic_logs", json!(report.unknown_topic_logs)),
                ("malformed_logs", json!(report.malformed_logs)),
            ],
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
    run_preflight(&rpc, &cli, &output).await?;

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

    let should_extract =
        cli.extract_features || cli.enable_memory || cli.enable_sequences || cli.enable_alerts;
    let normalization = NormalizationProfile {
        token0_decimals: cli.token0_decimals,
        token1_decimals: cli.token1_decimals,
    };
    let alert_config = AlertConfig {
        enabled: cli.enable_alerts,
        min_volume_ln: cli.alert_min_volume_ln,
        min_abs_imbalance: cli.alert_min_abs_imbalance,
        min_gas_used: cli.alert_min_gas_used,
        min_gas_ln_swap_spike: cli.alert_min_gas_ln_swap_spike,
        cooldown_blocks: cli.alert_cooldown_blocks,
    };
    let smoothing_alpha = cli.sequence_smoothing_alpha;
    let periodic_snapshot_path = cli.memory_snapshot_out.clone();
    let snapshot_interval_blocks = cli.snapshot_interval_blocks;
    let features_out_path = cli.features_out.clone();
    let features_out_rotate_records = cli.features_out_rotate_records;
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
            features_out_path,
            features_out_rotate_records,
            alert_config,
            output,
        )
        .await
    });

    let runtime_started = Instant::now();
    let mut producer_stats = ProducerStats::default();
    let mut heartbeat_blocks = 0u64;

    let mut next_block = cli.start_block;
    loop {
        if *shutdown_rx.borrow() {
            emit_event(
                &output,
                "shutdown_signal_received",
                vec![("stage", json!("producer"))],
            );
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
                            emit_event(
                                &output,
                                "shutdown_signal_received",
                                vec![("stage", json!("producer_poll_wait"))],
                            );
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
                    emit_event(
                        &output,
                        "shutdown_signal_received",
                        vec![("stage", json!("producer_idle_wait"))],
                    );
                    break;
                }
            }
            continue;
        }

        while next_block <= target_end {
            if *shutdown_rx.borrow() {
                emit_event(
                    &output,
                    "shutdown_signal_received",
                    vec![("stage", json!("producer_block_loop"))],
                );
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
                &output,
            )
            .await?;
            heartbeat_blocks = heartbeat_blocks.saturating_add(1);
            if cli.follow
                && cli.heartbeat_interval_blocks > 0
                && heartbeat_blocks % cli.heartbeat_interval_blocks == 0
            {
                emit_event(
                    &output,
                    "heartbeat",
                    vec![
                        ("block", json!(next_block)),
                        ("target_end", json!(target_end)),
                        ("lag", json!(block_lag)),
                        ("blocks_processed", json!(producer_stats.blocks_processed)),
                        ("receipts_enqueued", json!(producer_stats.receipts_enqueued)),
                        ("rpc_errors", json!(producer_stats.rpc_errors)),
                        ("queue_capacity", json!(queue_capacity)),
                        (
                            "error_mode",
                            json!(match cli.error_mode {
                                ErrorMode::FailSoft => "fail_soft",
                                ErrorMode::FailFast => "fail_fast",
                            }),
                        ),
                    ],
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
        emit_event(
            &output,
            "memory_snapshot_saved",
            vec![
                ("mode", json!("final")),
                ("path", json!(path)),
                ("patterns", json!(memory.pattern_count())),
            ],
        );
    }

    print_runtime_metrics(
        runtime_started.elapsed(),
        &producer_stats,
        &consumer_output.stats,
        &output,
    );
    print_topic0_summary(&producer_stats, cli.topic0_top_n, &output);

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
    output: &OutputConfig,
) -> Result<()> {
    let block_started = Instant::now();
    let block = match rpc.get_block_by_number(block_number, cli.full_tx).await {
        Ok(block) => block,
        Err(err) => {
            stats.rpc_errors = stats.rpc_errors.saturating_add(1);
            emit_event(
                output,
                "block_fetch_error",
                vec![
                    ("block", json!(block_number)),
                    ("error", json!(err.to_string())),
                ],
            );
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

    emit_event(
        output,
        "block",
        vec![
            ("number", json!(block.number)),
            ("hash", json!(block.hash.as_deref().unwrap_or("<none>"))),
            ("parent_hash", json!(block.parent_hash)),
            ("txs", json!(block.transactions.len())),
            ("timestamp", json!(block.timestamp)),
            ("lag", json!(block_lag)),
        ],
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
                emit_event(
                    output,
                    "logs_fetched",
                    vec![
                        ("from_block", json!(block_number)),
                        ("to_block", json!(block_number)),
                        ("total_logs", json!(logs.len())),
                        ("topic0_filters", json!(cli.log_topic0.len())),
                        ("address_filters", json!(cli.log_address.len())),
                    ],
                );
                if let Some(first_log) = logs.first() {
                    emit_event(
                        output,
                        "first_log",
                        vec![
                            ("block", json!(first_log.block_number)),
                            ("tx", json!(first_log.transaction_hash)),
                            ("address", json!(first_log.address)),
                            ("topics", json!(first_log.topics.len())),
                            ("data_len", json!(first_log.data.len())),
                        ],
                    );
                }
            }
            Err(err) => {
                stats.rpc_errors = stats.rpc_errors.saturating_add(1);
                emit_event(
                    output,
                    "logs_fetch_error",
                    vec![
                        ("block", json!(block_number)),
                        ("error", json!(err.to_string())),
                    ],
                );
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
                    emit_event(
                        output,
                        "receipt_fetch_error",
                        vec![("tx", json!(tx_hash)), ("error", json!(err.to_string()))],
                    );
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
                emit_event(
                    output,
                    "receipt",
                    vec![
                        ("tx", json!(receipt.transaction_hash)),
                        ("block", json!(receipt.block_number)),
                        ("gas_used", json!(receipt.gas_used)),
                        ("status", json!(receipt.status)),
                        ("logs", json!(receipt.logs.len())),
                    ],
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
        emit_event(
            output,
            "receipts_fetched",
            vec![("count", json!(fetched)), ("block", json!(block_number))],
        );
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
    features_out_path: Option<String>,
    features_out_rotate_records: u64,
    alert_config: AlertConfig,
    output: OutputConfig,
) -> Result<ConsumerOutput> {
    let mut stats = ConsumerStats::default();
    let mut last_alert_block_by_rule_pool: HashMap<String, u64> = HashMap::new();
    let mut features_out_part = 0_u64;
    let mut features_out_records_in_part = 0_u64;
    let mut features_out_current_path: Option<String> = None;
    let mut features_out = if let Some(path) = features_out_path.as_ref() {
        let (writer, resolved_path) =
            open_features_output_writer(path, features_out_rotate_records, features_out_part)?;
        features_out_current_path = Some(resolved_path.clone());
        emit_event(
            &output,
            "features_output_enabled",
            vec![
                ("path", json!(resolved_path)),
                ("rotate_records", json!(features_out_rotate_records)),
            ],
        );
        Some(writer)
    } else {
        None
    };

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
                    emit_event(
                        &output,
                        "features_extracted",
                        vec![
                            ("count", json!(features.len())),
                            ("tx", json!(receipt.transaction_hash)),
                            ("block", json!(block_number)),
                        ],
                    );
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
                    emit_event(
                        &output,
                        "feature_extraction_stats",
                        vec![
                            ("logs_total", json!(extraction_stats.logs_total)),
                            ("recognized", json!(extraction_stats.recognized_logs)),
                            ("unknown_topic", json!(extraction_stats.unknown_topic_logs)),
                            ("malformed", json!(extraction_stats.malformed_logs)),
                            ("tx", json!(receipt.transaction_hash)),
                        ],
                    );

                    for feature in features {
                        let normalized = normalize_feature_with_profile(feature, normalization);
                        emit_event(
                            &output,
                            "feature",
                            vec![
                                ("event", json!(event_label(&normalized.feature.event_kind))),
                                ("tx", json!(normalized.feature.tx_hash)),
                                ("pool", json!(normalized.feature.pool_address)),
                                (
                                    "volume_ln",
                                    json!(round_to(normalized.normalized_total_volume, 6)),
                                ),
                                (
                                    "imbalance",
                                    json!(round_to(normalized.normalized_imbalance, 6)),
                                ),
                                ("gas_ln", json!(round_to(normalized.normalized_gas, 6))),
                            ],
                        );
                        if let Some(writer) = features_out.as_mut() {
                            let record = json!({
                                "block": block_number,
                                "event": event_label(&normalized.feature.event_kind),
                                "tx": normalized.feature.tx_hash,
                                "pool": normalized.feature.pool_address,
                                "volume_ln": round_to(normalized.normalized_total_volume, 6),
                                "imbalance": round_to(normalized.normalized_imbalance, 6),
                                "gas_ln": round_to(normalized.normalized_gas, 6)
                            });
                            writeln!(writer, "{record}")
                                .context("failed to write feature record")?;
                            features_out_records_in_part =
                                features_out_records_in_part.saturating_add(1);
                            if features_out_rotate_records > 0
                                && features_out_records_in_part >= features_out_rotate_records
                            {
                                writer
                                    .flush()
                                    .context("failed to flush features output file before rotate")?;
                                features_out_part = features_out_part.saturating_add(1);
                                features_out_records_in_part = 0;
                                let (next_writer, next_path) = open_features_output_writer(
                                    features_out_path
                                        .as_ref()
                                        .context("features output path missing during rotate")?,
                                    features_out_rotate_records,
                                    features_out_part,
                                )?;
                                emit_event(
                                    &output,
                                    "features_output_rotated",
                                    vec![
                                        (
                                            "from",
                                            json!(features_out_current_path
                                                .clone()
                                                .unwrap_or_else(|| "<none>".to_string())),
                                        ),
                                        ("to", json!(next_path.clone())),
                                        ("part", json!(features_out_part)),
                                    ],
                                );
                                features_out_current_path = Some(next_path);
                                *writer = next_writer;
                            }
                        }

                        if let Some(memory) = memory.as_mut() {
                            match memory.observe(&normalized) {
                                MatchOutcome::Matched {
                                    pattern_id,
                                    distance,
                                    confidence,
                                } => {
                                    emit_event(
                                        &output,
                                        "memory_matched",
                                        vec![
                                            ("pattern_id", json!(pattern_id)),
                                            ("distance", json!(round_to(distance, 6))),
                                            ("confidence", json!(round_to(confidence, 6))),
                                        ],
                                    );
                                }
                                MatchOutcome::Created { pattern_id } => {
                                    emit_event(
                                        &output,
                                        "memory_created",
                                        vec![("pattern_id", json!(pattern_id))],
                                    );
                                }
                            }
                        }

                        if let Some(sequences) = sequences.as_mut() {
                            let event =
                                SequenceEventKind::from_liquidity_event(&normalized.feature.event_kind);
                            sequences.observe_entity_event(normalized.feature.pool_address.clone(), event);
                            emit_event(
                                &output,
                                "sequence_observe",
                                vec![
                                    ("block", json!(block_number)),
                                    ("event", json!(event.as_str())),
                                    ("key", json!(normalized.feature.pool_address)),
                                ],
                            );
                        }

                        if alert_config.enabled {
                            let abs_imbalance = normalized.normalized_imbalance.abs();
                            let gas_used = normalized.feature.gas_used as f64;
                            let pool = normalized.feature.pool_address.clone();
                            let passes_high_imbalance_volume = normalized.normalized_total_volume
                                >= alert_config.min_volume_ln
                                && abs_imbalance >= alert_config.min_abs_imbalance
                                && gas_used >= alert_config.min_gas_used;
                            if passes_high_imbalance_volume {
                                let kind = "high_imbalance_high_volume";
                                let should_emit = should_emit_alert(
                                    &last_alert_block_by_rule_pool,
                                    kind,
                                    &pool,
                                    block_number,
                                    alert_config.cooldown_blocks,
                                );
                                if should_emit {
                                    let severity = classify_alert_severity(
                                        normalized.normalized_total_volume,
                                        abs_imbalance,
                                        gas_used,
                                        &alert_config,
                                    );
                                    emit_event(
                                        &output,
                                        "alert",
                                        vec![
                                            (
                                                "kind",
                                                json!(kind),
                                            ),
                                            ("block", json!(block_number)),
                                            ("severity", json!(severity.as_str())),
                                            ("event", json!(event_label(&normalized.feature.event_kind))),
                                            ("tx", json!(normalized.feature.tx_hash)),
                                            ("pool", json!(pool.clone())),
                                            (
                                                "volume_ln",
                                                json!(round_to(
                                                    normalized.normalized_total_volume,
                                                    6,
                                                )),
                                            ),
                                            (
                                                "abs_imbalance",
                                                json!(round_to(abs_imbalance, 6)),
                                            ),
                                            ("gas_used", json!(normalized.feature.gas_used)),
                                        ],
                                    );
                                    emit_metric(
                                        &output,
                                        "alerts",
                                        vec![
                                            ("block", json!(block_number)),
                                            ("pool", json!(pool.clone())),
                                            ("kind", json!(kind)),
                                            ("severity", json!(severity.as_str())),
                                        ],
                                    );
                                    stats.alerts_emitted = stats.alerts_emitted.saturating_add(1);
                                    stats.alerts_rule_high_imbalance_high_volume = stats
                                        .alerts_rule_high_imbalance_high_volume
                                        .saturating_add(1);
                                    match severity {
                                        AlertSeverity::Warning => {
                                            stats.alerts_warning =
                                                stats.alerts_warning.saturating_add(1)
                                        }
                                        AlertSeverity::Critical => {
                                            stats.alerts_critical =
                                                stats.alerts_critical.saturating_add(1)
                                        }
                                    }
                                    remember_alert(
                                        &mut last_alert_block_by_rule_pool,
                                        kind,
                                        &pool,
                                        block_number,
                                    );
                                }
                            }

                            let is_swap =
                                matches!(normalized.feature.event_kind, LiquidityEventKind::Swap);
                            let passes_swap_gas_spike =
                                is_swap && normalized.normalized_gas >= alert_config.min_gas_ln_swap_spike;
                            if passes_swap_gas_spike {
                                let kind = "swap_gas_spike";
                                let should_emit = should_emit_alert(
                                    &last_alert_block_by_rule_pool,
                                    kind,
                                    &pool,
                                    block_number,
                                    alert_config.cooldown_blocks,
                                );
                                if should_emit {
                                    let severity = classify_swap_gas_spike_severity(
                                        normalized.normalized_gas,
                                        alert_config.min_gas_ln_swap_spike,
                                    );
                                    emit_event(
                                        &output,
                                        "alert",
                                        vec![
                                            ("kind", json!(kind)),
                                            ("block", json!(block_number)),
                                            ("severity", json!(severity.as_str())),
                                            ("event", json!(event_label(&normalized.feature.event_kind))),
                                            ("tx", json!(normalized.feature.tx_hash)),
                                            ("pool", json!(pool.clone())),
                                            (
                                                "gas_ln",
                                                json!(round_to(normalized.normalized_gas, 6)),
                                            ),
                                            ("gas_used", json!(normalized.feature.gas_used)),
                                        ],
                                    );
                                    emit_metric(
                                        &output,
                                        "alerts",
                                        vec![
                                            ("block", json!(block_number)),
                                            ("pool", json!(pool.clone())),
                                            ("kind", json!(kind)),
                                            ("severity", json!(severity.as_str())),
                                        ],
                                    );
                                    stats.alerts_emitted = stats.alerts_emitted.saturating_add(1);
                                    stats.alerts_rule_swap_gas_spike =
                                        stats.alerts_rule_swap_gas_spike.saturating_add(1);
                                    match severity {
                                        AlertSeverity::Warning => {
                                            stats.alerts_warning =
                                                stats.alerts_warning.saturating_add(1)
                                        }
                                        AlertSeverity::Critical => {
                                            stats.alerts_critical =
                                                stats.alerts_critical.saturating_add(1)
                                        }
                                    }
                                    remember_alert(
                                        &mut last_alert_block_by_rule_pool,
                                        kind,
                                        &pool,
                                        block_number,
                                    );
                                }
                            }
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
                    emit_event(
                        &output,
                        "memory_metrics",
                        vec![
                            ("block", json!(block_number)),
                            ("active_patterns", json!(metrics.active_patterns)),
                            ("match_ratio", json!(round_to(metrics.match_ratio, 6))),
                            ("churn_rate", json!(round_to(metrics.churn_rate, 6))),
                            ("pruned_now", json!(pruned)),
                            ("total_obs", json!(metrics.total_observations)),
                            ("matched", json!(metrics.matched_observations)),
                            ("created", json!(metrics.created_observations)),
                            ("pruned_total", json!(metrics.pruned_patterns_total)),
                        ],
                    );
                    emit_metric(
                        &output,
                        "memory_metrics",
                        vec![
                            ("block", json!(block_number)),
                            ("active_patterns", json!(metrics.active_patterns)),
                            ("match_ratio", json!(round_to(metrics.match_ratio, 6))),
                            ("churn_rate", json!(round_to(metrics.churn_rate, 6))),
                            ("pruned_now", json!(pruned)),
                            ("total_obs", json!(metrics.total_observations)),
                            ("matched", json!(metrics.matched_observations)),
                            ("created", json!(metrics.created_observations)),
                            ("pruned_total", json!(metrics.pruned_patterns_total)),
                        ],
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
                    emit_event(
                        &output,
                        "sequence_metrics",
                        vec![
                            ("block", json!(block_number)),
                            ("entities", json!(metrics.tracked_entities)),
                            ("total_transitions", json!(metrics.total_transitions)),
                            ("unique_transitions", json!(metrics.unique_transitions)),
                            ("p_swap_add_raw", json!(round_to(raw_swap_to_add, 6))),
                            ("p_swap_add", json!(round_to(swap_to_add, 6))),
                            ("p_add_remove", json!(round_to(add_to_remove, 6))),
                        ],
                    );
                    emit_metric(
                        &output,
                        "sequence_metrics",
                        vec![
                            ("block", json!(block_number)),
                            ("entities", json!(metrics.tracked_entities)),
                            ("total_transitions", json!(metrics.total_transitions)),
                            ("unique_transitions", json!(metrics.unique_transitions)),
                            ("p_swap_add_raw", json!(round_to(raw_swap_to_add, 6))),
                            ("p_swap_add", json!(round_to(swap_to_add, 6))),
                            ("p_add_remove", json!(round_to(add_to_remove, 6))),
                        ],
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
                        emit_event(
                            &output,
                            "memory_snapshot_saved",
                            vec![
                                ("mode", json!("periodic")),
                                ("block", json!(block_number)),
                                ("path", json!(path)),
                                ("patterns", json!(memory.pattern_count())),
                            ],
                        );
                    }
                }
                if let Some(writer) = features_out.as_mut() {
                    writer
                        .flush()
                        .context("failed to flush features output file")?;
                }
            }
        }
    }

    if let Some(writer) = features_out.as_mut() {
        writer
            .flush()
            .context("failed to flush features output file on shutdown")?;
    }

    Ok(ConsumerOutput {
        memory,
        stats,
    })
}

fn print_runtime_metrics(
    elapsed: Duration,
    producer: &ProducerStats,
    consumer: &ConsumerStats,
    output: &OutputConfig,
) {
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

    let fields = vec![
        ("elapsed_s", json!(round_to(elapsed_secs, 6))),
        ("blocks", json!(producer.blocks_processed)),
        ("receipts_in", json!(producer.receipts_enqueued)),
        ("receipts_processed", json!(consumer.receipts_processed)),
        ("features", json!(consumer.features_processed)),
        ("alerts_emitted", json!(consumer.alerts_emitted)),
        ("alerts_warning", json!(consumer.alerts_warning)),
        ("alerts_critical", json!(consumer.alerts_critical)),
        (
            "alerts_rule_high_imbalance_high_volume",
            json!(consumer.alerts_rule_high_imbalance_high_volume),
        ),
        (
            "alerts_rule_swap_gas_spike",
            json!(consumer.alerts_rule_swap_gas_spike),
        ),
        ("recognized_logs", json!(consumer.recognized_logs)),
        ("unknown_topic_logs", json!(consumer.unknown_topic_logs)),
        ("malformed_logs", json!(consumer.malformed_logs)),
        ("throughput_rps", json!(round_to(throughput_rps, 6))),
        ("avg_block_fetch_ms", json!(round_to(avg_block_fetch_ms, 6))),
        ("avg_receipt_fetch_ms", json!(round_to(avg_receipt_fetch_ms, 6))),
        (
            "avg_queue_backpressure_ms",
            json!(round_to(avg_queue_backpressure_ms, 6)),
        ),
        ("avg_queue_latency_ms", json!(round_to(avg_queue_latency_ms, 6))),
        ("avg_processing_ms", json!(round_to(avg_processing_ms, 6))),
        ("logs_total", json!(producer.logs_fetched_total)),
        ("logs_per_block", json!(round_to(logs_per_block, 6))),
        ("log_hit_ratio", json!(round_to(log_hit_ratio, 6))),
        ("features_per_block", json!(round_to(features_per_block, 6))),
        ("feature_hit_ratio", json!(round_to(feature_hit_ratio, 6))),
        ("rpc_errors", json!(producer.rpc_errors)),
        ("max_block_lag", json!(consumer.max_block_lag)),
    ];
    emit_event(output, "runtime_metrics", fields.clone());
    emit_metric(output, "runtime_metrics", fields);
}

fn event_label(kind: &LiquidityEventKind) -> &'static str {
    match kind {
        LiquidityEventKind::Swap => "swap",
        LiquidityEventKind::AddLiquidity => "add_liquidity",
        LiquidityEventKind::RemoveLiquidity => "remove_liquidity",
    }
}

fn print_topic0_summary(producer: &ProducerStats, top_n: usize, output: &OutputConfig) {
    if producer.topic0_counts.is_empty() || top_n == 0 {
        return;
    }

    let mut rows: Vec<(&String, &u64)> = producer.topic0_counts.iter().collect();
    rows.sort_by(|a, b| b.1.cmp(a.1));

    for (rank, (topic, count)) in rows.into_iter().take(top_n).enumerate() {
        emit_event(
            output,
            "topic0_top",
            vec![
                ("rank", json!(rank + 1)),
                ("count", json!(count)),
                ("topic", json!(topic)),
            ],
        );
    }
}

async fn run_preflight(rpc: &RpcClient, cli: &Cli, output: &OutputConfig) -> Result<()> {
    if cli.skip_preflight {
        emit_event(
            output,
            "preflight_skipped",
            vec![("reason", json!("--skip-preflight enabled"))],
        );
        return Ok(());
    }

    emit_event(
        output,
        "preflight_started",
        vec![
            ("start_block", json!(cli.start_block)),
            ("end_block", json!(cli.end_block)),
            ("follow", json!(cli.follow)),
        ],
    );

    let latest = rpc
        .get_latest_block_number()
        .await
        .context("preflight failed to fetch latest block")?;

    if !cli.follow && cli.start_block > latest {
        anyhow::bail!(
            "preflight failed: start_block ({}) > latest ({})",
            cli.start_block,
            latest
        );
    }

    if let Some(end_block) = cli.end_block {
        if !cli.follow && end_block > latest {
            emit_event(
                output,
                "preflight_warning",
                vec![
                    ("kind", json!("end_block_above_latest")),
                    ("end_block", json!(end_block)),
                    ("latest", json!(latest)),
                ],
            );
        }
    }

    emit_event(
        output,
        "preflight_ok",
        vec![
            ("latest_block", json!(latest)),
            ("queue_capacity", json!(cli.pipeline_queue_capacity.max(1))),
            ("extract_features", json!(cli.extract_features)),
            ("enable_memory", json!(cli.enable_memory)),
            ("enable_sequences", json!(cli.enable_sequences)),
            ("enable_alerts", json!(cli.enable_alerts)),
            ("fetch_logs", json!(cli.fetch_logs)),
            ("receipt_limit", json!(cli.receipt_limit)),
        ],
    );

    Ok(())
}

fn emit_event(output: &OutputConfig, event: &str, fields: Vec<(&str, Value)>) {
    match output.log_format {
        LogFormat::Text => {
            let mut parts = Vec::with_capacity(fields.len() + 1);
            parts.push(event.to_string());
            for (key, value) in fields {
                parts.push(format!("{}={}", key, value_to_text(&value)));
            }
            println!("{}", parts.join(" "));
        }
        LogFormat::Json => {
            let mut obj = Map::new();
            obj.insert("type".to_string(), json!("event"));
            obj.insert("event".to_string(), json!(event));
            for (key, value) in fields {
                obj.insert(key.to_string(), value);
            }
            println!("{}", Value::Object(obj));
        }
    }
}

fn emit_metric(output: &OutputConfig, metric: &str, fields: Vec<(&str, Value)>) {
    if !output.emit_metrics_json {
        return;
    }

    let mut obj = Map::new();
    obj.insert("type".to_string(), json!("metric"));
    obj.insert("metric".to_string(), json!(metric));
    for (key, value) in fields {
        obj.insert(key.to_string(), value);
    }
    println!("{}", Value::Object(obj));
}

fn value_to_text(value: &Value) -> String {
    match value {
        Value::String(v) => v.clone(),
        _ => value.to_string(),
    }
}

fn round_to(value: f64, digits: usize) -> f64 {
    let multiplier = 10_f64.powi(digits as i32);
    (value * multiplier).round() / multiplier
}

fn classify_alert_severity(
    volume_ln: f64,
    abs_imbalance: f64,
    gas_used: f64,
    config: &AlertConfig,
) -> AlertSeverity {
    let critical = volume_ln >= config.min_volume_ln * 2.0
        && abs_imbalance >= config.min_abs_imbalance * 2.0
        && gas_used >= config.min_gas_used * 1.5;
    if critical {
        AlertSeverity::Critical
    } else {
        AlertSeverity::Warning
    }
}

fn classify_swap_gas_spike_severity(gas_ln: f64, min_gas_ln_swap_spike: f64) -> AlertSeverity {
    if gas_ln >= min_gas_ln_swap_spike + 1.0 {
        AlertSeverity::Critical
    } else {
        AlertSeverity::Warning
    }
}

fn should_emit_alert(
    last_alert_block_by_rule_pool: &HashMap<String, u64>,
    kind: &str,
    pool: &str,
    block_number: u64,
    cooldown_blocks: u64,
) -> bool {
    let key = format!("{kind}:{pool}");
    match last_alert_block_by_rule_pool.get(&key) {
        Some(last_block) => block_number.saturating_sub(*last_block) >= cooldown_blocks,
        None => true,
    }
}

fn remember_alert(
    last_alert_block_by_rule_pool: &mut HashMap<String, u64>,
    kind: &str,
    pool: &str,
    block_number: u64,
) {
    let key = format!("{kind}:{pool}");
    last_alert_block_by_rule_pool.insert(key, block_number);
}

fn open_features_output_writer(
    base_path: &str,
    rotate_records: u64,
    part: u64,
) -> Result<(BufWriter<File>, String)> {
    let path = resolve_features_output_path(base_path, rotate_records, part);
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open features output file: {path}"))?;
    Ok((BufWriter::new(file), path))
}

fn resolve_features_output_path(base_path: &str, rotate_records: u64, part: u64) -> String {
    if rotate_records == 0 {
        return base_path.to_string();
    }

    let path = Path::new(base_path);
    let parent = path.parent();
    let stem = path
        .file_stem()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_else(|| "features".to_string());
    let extension = path
        .extension()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_default();

    let filename = if extension.is_empty() {
        format!("{stem}.part{part:06}")
    } else {
        format!("{stem}.part{part:06}.{extension}")
    };

    if let Some(parent) = parent {
        parent.join(filename).to_string_lossy().to_string()
    } else {
        filename
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
