mod features;
mod ingestion;

use anyhow::Result;
use clap::Parser;
use features::extractor::{LiquidityEventKind, extract_features_from_receipt, normalize_feature};
use ingestion::rpc::{RpcClient, RpcPolicy, extract_tx_hash};
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
            process_block(&rpc, &cli, next_block).await?;
            next_block += 1;
        }

        if !cli.follow {
            break;
        }
    }

    Ok(())
}

async fn process_block(rpc: &RpcClient, cli: &Cli, block_number: u64) -> Result<()> {
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

                if cli.extract_features {
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
                    }
                }
                fetched += 1;
            }
        }
        println!("receipts_fetched={fetched}");
    }

    Ok(())
}
