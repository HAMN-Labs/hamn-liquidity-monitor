use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub struct RpcClient {
    http: Client,
    url: String,
    policy: RpcPolicy,
}

#[derive(Debug, Clone, Copy)]
pub struct RpcPolicy {
    pub timeout_ms: u64,
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
}

impl Default for RpcPolicy {
    fn default() -> Self {
        Self {
            timeout_ms: 10_000,
            max_retries: 3,
            initial_backoff_ms: 250,
            max_backoff_ms: 2_000,
        }
    }
}

impl RpcClient {
    pub fn new_with_policy(url: impl Into<String>, policy: RpcPolicy) -> Self {
        Self {
            http: Client::new(),
            url: url.into(),
            policy,
        }
    }

    pub async fn get_block_by_number(&self, block_number: u64, full_tx: bool) -> Result<Block> {
        let params = serde_json::json!([to_hex_quantity(block_number), full_tx]);
        self.call("eth_getBlockByNumber", params)
            .await?
            .context("block not found in JSON-RPC response")
    }

    pub async fn get_logs(&self, from_block: u64, to_block: u64) -> Result<Vec<LogEntry>> {
        let params = serde_json::json!([{
            "fromBlock": to_hex_quantity(from_block),
            "toBlock": to_hex_quantity(to_block)
        }]);
        self.call("eth_getLogs", params)
            .await?
            .context("logs missing in JSON-RPC response")
    }

    pub async fn get_transaction_receipt(
        &self,
        transaction_hash: &str,
    ) -> Result<Option<TransactionReceipt>> {
        let params = serde_json::json!([transaction_hash]);
        self.call("eth_getTransactionReceipt", params).await
    }

    pub async fn get_latest_block_number(&self) -> Result<u64> {
        let result: String = self
            .call("eth_blockNumber", serde_json::json!([]))
            .await?
            .context("latest block number missing in JSON-RPC response")?;
        parse_hex_quantity(&result).context("failed to parse latest block number")
    }

    async fn call<T>(&self, method: &'static str, params: serde_json::Value) -> Result<Option<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        for attempt in 0..=self.policy.max_retries {
            let send_result = self
                .http
                .post(&self.url)
                .timeout(Duration::from_millis(self.policy.timeout_ms))
                .json(&JsonRpcRequest::new(method, params.clone()))
                .send()
                .await;

            match send_result {
                Ok(response) => {
                    let status = response.status();
                    if status.is_server_error() || status.as_u16() == 429 {
                        if attempt < self.policy.max_retries {
                            sleep(Duration::from_millis(backoff_delay_ms(&self.policy, attempt))).await;
                            continue;
                        }
                        bail!("JSON-RPC endpoint returned retryable status {status} after retries");
                    }

                    if !status.is_success() {
                        bail!("JSON-RPC endpoint returned status {status}");
                    }

                    let response: JsonRpcResponse<T> = response
                        .json()
                        .await
                        .context("failed to deserialize JSON-RPC response")?;

                    if let Some(err) = response.error {
                        bail!("rpc error {}: {}", err.code, err.message);
                    }

                    return Ok(response.result);
                }
                Err(err) => {
                    let retryable = err.is_timeout() || err.is_connect() || err.is_request();
                    if retryable && attempt < self.policy.max_retries {
                        sleep(Duration::from_millis(backoff_delay_ms(&self.policy, attempt))).await;
                        continue;
                    }
                    return Err(err).context("failed to send JSON-RPC request");
                }
            }
        }

        bail!("exhausted retries without response")
    }
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    method: &'static str,
    params: serde_json::Value,
    id: u64,
}

impl JsonRpcRequest {
    fn new(method: &'static str, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0",
            method,
            params,
            id: 1,
        }
    }
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: u64,
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
pub struct Block {
    #[serde(deserialize_with = "de_hex_u64")]
    pub number: u64,
    pub hash: Option<String>,
    #[serde(rename = "parentHash")]
    pub parent_hash: String,
    #[serde(rename = "timestamp", deserialize_with = "de_hex_u64")]
    pub timestamp: u64,
    pub transactions: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct LogEntry {
    #[serde(rename = "blockNumber", deserialize_with = "de_hex_u64")]
    pub block_number: u64,
    #[serde(rename = "transactionHash")]
    pub transaction_hash: String,
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
}

#[derive(Debug, Deserialize)]
pub struct TransactionReceipt {
    #[serde(rename = "transactionHash")]
    pub transaction_hash: String,
    #[serde(rename = "blockNumber", deserialize_with = "de_hex_u64")]
    pub block_number: u64,
    #[serde(rename = "gasUsed", deserialize_with = "de_hex_u64")]
    pub gas_used: u64,
    #[serde(rename = "status", deserialize_with = "de_hex_u64")]
    pub status: u64,
    pub logs: Vec<serde_json::Value>,
}

pub fn extract_tx_hash(tx: &serde_json::Value) -> Option<&str> {
    match tx {
        serde_json::Value::String(hash) => Some(hash.as_str()),
        serde_json::Value::Object(map) => map.get("hash").and_then(serde_json::Value::as_str),
        _ => None,
    }
}

fn to_hex_quantity(value: u64) -> String {
    format!("0x{value:x}")
}

fn parse_hex_quantity(value: &str) -> Result<u64> {
    let s = value.trim_start_matches("0x");
    u64::from_str_radix(s, 16).context("invalid hex quantity")
}

fn backoff_delay_ms(policy: &RpcPolicy, attempt: u32) -> u64 {
    let factor = 1u64 << attempt.min(20);
    policy
        .initial_backoff_ms
        .saturating_mul(factor)
        .min(policy.max_backoff_ms)
}

fn de_hex_u64<'de, D>(deserializer: D) -> std::result::Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let s = s.trim_start_matches("0x");
    u64::from_str_radix(s, 16).map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantity_hex_encoding() {
        assert_eq!(to_hex_quantity(0), "0x0");
        assert_eq!(to_hex_quantity(16), "0x10");
        assert_eq!(to_hex_quantity(255), "0xff");
    }

    #[test]
    fn tx_hash_extraction_for_hash_and_full_tx() {
        let from_hash = serde_json::json!("0xabc");
        let from_full = serde_json::json!({"hash":"0xdef","nonce":"0x1"});

        assert_eq!(extract_tx_hash(&from_hash), Some("0xabc"));
        assert_eq!(extract_tx_hash(&from_full), Some("0xdef"));
    }

    #[test]
    fn backoff_increases_and_caps() {
        let policy = RpcPolicy {
            timeout_ms: 1_000,
            max_retries: 3,
            initial_backoff_ms: 100,
            max_backoff_ms: 250,
        };
        assert_eq!(backoff_delay_ms(&policy, 0), 100);
        assert_eq!(backoff_delay_ms(&policy, 1), 200);
        assert_eq!(backoff_delay_ms(&policy, 2), 250);
        assert_eq!(backoff_delay_ms(&policy, 3), 250);
    }
}
