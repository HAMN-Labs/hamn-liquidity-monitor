mod ingestion;

use anyhow::Result;
use clap::Parser;
use ingestion::rpc::{RpcClient, extract_tx_hash};

#[derive(Debug, Parser)]
#[command(author, version, about = "HAMN liquidity monitor (Stage 1 ingestion)")]
struct Cli {
    #[arg(long, env = "HAMN_RPC_URL")]
    rpc_url: String,

    #[arg(long)]
    start_block: u64,

    #[arg(long)]
    end_block: u64,

    #[arg(long, default_value_t = false)]
    full_tx: bool,

    #[arg(long, default_value_t = false)]
    fetch_logs: bool,

    #[arg(long, default_value_t = 0)]
    receipt_limit: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.start_block > cli.end_block {
        anyhow::bail!("start_block must be <= end_block");
    }

    let rpc = RpcClient::new(cli.rpc_url);
    let mut transaction_hashes = Vec::new();
    for block_number in cli.start_block..=cli.end_block {
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
    }

    if cli.fetch_logs {
        let logs = rpc.get_logs(cli.start_block, cli.end_block).await?;
        println!(
            "logs_range={}..={} total_logs={}",
            cli.start_block,
            cli.end_block,
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
                fetched += 1;
            }
        }
        println!("receipts_fetched={fetched}");
    }

    Ok(())
}
