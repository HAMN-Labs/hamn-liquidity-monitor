use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct RpcClient {
    http: Client,
    url: String,
}

impl RpcClient {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            http: Client::new(),
            url: url.into(),
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

    async fn call<T>(&self, method: &'static str, params: serde_json::Value) -> Result<Option<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let response: JsonRpcResponse<T> = self
            .http
            .post(&self.url)
            .json(&JsonRpcRequest::new(method, params))
            .send()
            .await
            .context("failed to send JSON-RPC request")?
            .error_for_status()
            .context("JSON-RPC endpoint returned non-success status")?
            .json()
            .await
            .context("failed to deserialize JSON-RPC response")?;

        if let Some(err) = response.error {
            bail!("rpc error {}: {}", err.code, err.message);
        }

        Ok(response.result)
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
}
