use crate::ingestion::rpc::TransactionReceipt;

const SWAP_TOPIC_PREFIX: &str = "0xd78ad95f";
const MINT_TOPIC_PREFIX: &str = "0x4c209b5f";
const BURN_TOPIC_PREFIX: &str = "0xdccd412f";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiquidityEventKind {
    Swap,
    AddLiquidity,
    RemoveLiquidity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiquidityAmounts {
    Swap {
        amount0_in: u128,
        amount1_in: u128,
        amount0_out: u128,
        amount1_out: u128,
    },
    AddLiquidity {
        amount0: u128,
        amount1: u128,
    },
    RemoveLiquidity {
        amount0: u128,
        amount1: u128,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiquidityFeature {
    pub tx_hash: String,
    pub block_number: u64,
    pub pool_address: String,
    pub log_index: Option<u64>,
    pub event_kind: LiquidityEventKind,
    pub amounts: LiquidityAmounts,
    pub gas_used: u64,
    pub tx_status: u64,
}

#[derive(Debug, Clone)]
pub struct NormalizedLiquidityFeature {
    pub feature: LiquidityFeature,
    pub normalized_total_volume: f64,
    pub normalized_imbalance: f64,
    pub normalized_gas: f64,
}

pub fn extract_features_from_receipt(receipt: &TransactionReceipt) -> Vec<LiquidityFeature> {
    let mut output = Vec::new();
    for log in &receipt.logs {
        let Some(topic0) = log.topics.first().map(String::as_str) else {
            continue;
        };

        let data_words = parse_u256_words(&log.data);
        let parsed = if has_topic_prefix(topic0, SWAP_TOPIC_PREFIX) {
            if data_words.len() < 4 {
                None
            } else {
                Some((
                    LiquidityEventKind::Swap,
                    LiquidityAmounts::Swap {
                        amount0_in: data_words[0],
                        amount1_in: data_words[1],
                        amount0_out: data_words[2],
                        amount1_out: data_words[3],
                    },
                ))
            }
        } else if has_topic_prefix(topic0, MINT_TOPIC_PREFIX) {
            if data_words.len() < 2 {
                None
            } else {
                Some((
                    LiquidityEventKind::AddLiquidity,
                    LiquidityAmounts::AddLiquidity {
                        amount0: data_words[0],
                        amount1: data_words[1],
                    },
                ))
            }
        } else if has_topic_prefix(topic0, BURN_TOPIC_PREFIX) {
            if data_words.len() < 2 {
                None
            } else {
                Some((
                    LiquidityEventKind::RemoveLiquidity,
                    LiquidityAmounts::RemoveLiquidity {
                        amount0: data_words[0],
                        amount1: data_words[1],
                    },
                ))
            }
        } else {
            None
        };

        if let Some((event_kind, amounts)) = parsed {
            output.push(LiquidityFeature {
                tx_hash: receipt.transaction_hash.clone(),
                block_number: receipt.block_number,
                pool_address: log.address.clone(),
                log_index: log.log_index,
                event_kind,
                amounts,
                gas_used: receipt.gas_used,
                tx_status: receipt.status,
            });
        }
    }

    output
}

pub fn normalize_feature(feature: LiquidityFeature) -> NormalizedLiquidityFeature {
    let (total_volume, imbalance) = match &feature.amounts {
        LiquidityAmounts::Swap {
            amount0_in,
            amount1_in,
            amount0_out,
            amount1_out,
        } => {
            let in_total = amount0_in.saturating_add(*amount1_in);
            let out_total = amount0_out.saturating_add(*amount1_out);
            let total = in_total.saturating_add(out_total);
            (total, abs_diff(in_total, out_total))
        }
        LiquidityAmounts::AddLiquidity { amount0, amount1 }
        | LiquidityAmounts::RemoveLiquidity { amount0, amount1 } => {
            let total = amount0.saturating_add(*amount1);
            (total, abs_diff(*amount0, *amount1))
        }
    };

    let total_f = total_volume as f64;
    let imbalance_f = imbalance as f64;
    let gas_used = feature.gas_used as f64;

    NormalizedLiquidityFeature {
        feature,
        normalized_total_volume: (1.0 + total_f).ln(),
        normalized_imbalance: imbalance_f / (total_f + 1.0),
        normalized_gas: (1.0 + gas_used).ln(),
    }
}

fn abs_diff(a: u128, b: u128) -> u128 {
    if a >= b { a - b } else { b - a }
}

fn parse_u256_words(data: &str) -> Vec<u128> {
    let hex = data.strip_prefix("0x").unwrap_or(data);
    if hex.is_empty() || hex.len() % 64 != 0 {
        return Vec::new();
    }

    let mut words = Vec::with_capacity(hex.len() / 64);
    let mut i = 0;
    while i + 64 <= hex.len() {
        let word = &hex[i..i + 64];
        let low_16_bytes = &word[32..64];
        match u128::from_str_radix(low_16_bytes, 16) {
            Ok(value) => words.push(value),
            Err(_) => return Vec::new(),
        }
        i += 64;
    }
    words
}

fn has_topic_prefix(topic: &str, prefix: &str) -> bool {
    topic
        .get(0..prefix.len())
        .map(|value| value.eq_ignore_ascii_case(prefix))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingestion::rpc::ReceiptLog;

    fn hex_word(value: u128) -> String {
        format!("{:064x}", value)
    }

    #[test]
    fn extract_swap_feature_from_receipt_log() {
        let log = ReceiptLog {
            address: "0xpool".to_string(),
            topics: vec![
                "0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822".to_string(),
            ],
            data: format!(
                "0x{}{}{}{}",
                hex_word(11),
                hex_word(0),
                hex_word(0),
                hex_word(7)
            ),
            log_index: Some(5),
        };
        let receipt = TransactionReceipt {
            transaction_hash: "0xtx".to_string(),
            block_number: 42,
            gas_used: 21_000,
            status: 1,
            logs: vec![log],
        };

        let features = extract_features_from_receipt(&receipt);
        assert_eq!(features.len(), 1);
        assert_eq!(features[0].event_kind, LiquidityEventKind::Swap);
        assert_eq!(features[0].log_index, Some(5));
    }

    #[test]
    fn normalize_feature_outputs_stable_values() {
        let feature = LiquidityFeature {
            tx_hash: "0xtx".to_string(),
            block_number: 1,
            pool_address: "0xpool".to_string(),
            log_index: None,
            event_kind: LiquidityEventKind::AddLiquidity,
            amounts: LiquidityAmounts::AddLiquidity {
                amount0: 100,
                amount1: 50,
            },
            gas_used: 100_000,
            tx_status: 1,
        };

        let normalized = normalize_feature(feature);
        assert!(normalized.normalized_total_volume > 0.0);
        assert!(normalized.normalized_imbalance > 0.0);
        assert!(normalized.normalized_gas > 0.0);
    }

    #[test]
    fn ignore_unknown_topic() {
        let receipt = TransactionReceipt {
            transaction_hash: "0xtx".to_string(),
            block_number: 1,
            gas_used: 21_000,
            status: 1,
            logs: vec![ReceiptLog {
                address: "0xpool".to_string(),
                topics: vec!["0xdeadbeef".to_string()],
                data: "0x".to_string(),
                log_index: Some(0),
            }],
        };

        let features = extract_features_from_receipt(&receipt);
        assert!(features.is_empty());
    }
}
