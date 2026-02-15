use crate::ingestion::rpc::TransactionReceipt;

const SWAP_TOPIC_PREFIX: &str = "0xd78ad95f";
const MINT_TOPIC_PREFIX: &str = "0x4c209b5f";
const BURN_TOPIC_PREFIX: &str = "0xdccd412f";
const UNISWAP_V3_SWAP_TOPIC_PREFIX: &str = "0xc42079f9";
const ALGEBRA_SWAP_TOPIC_PREFIX: &str = "0x19b47279";

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

#[derive(Debug, Clone, Copy, Default)]
pub struct ExtractionStats {
    pub logs_total: u64,
    pub recognized_logs: u64,
    pub unknown_topic_logs: u64,
    pub malformed_logs: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct NormalizationProfile {
    pub token0_decimals: u8,
    pub token1_decimals: u8,
}

impl Default for NormalizationProfile {
    fn default() -> Self {
        Self {
            token0_decimals: 18,
            token1_decimals: 18,
        }
    }
}

#[allow(dead_code)]
pub fn extract_features_from_receipt(receipt: &TransactionReceipt) -> Vec<LiquidityFeature> {
    extract_features_with_stats(receipt).0
}

pub fn extract_features_with_stats(
    receipt: &TransactionReceipt,
) -> (Vec<LiquidityFeature>, ExtractionStats) {
    let mut output = Vec::new();
    let mut stats = ExtractionStats::default();

    for log in &receipt.logs {
        stats.logs_total = stats.logs_total.saturating_add(1);
        let Some(topic0) = log.topics.first().map(String::as_str) else {
            stats.malformed_logs = stats.malformed_logs.saturating_add(1);
            continue;
        };

        let parsed = if has_topic_prefix(topic0, SWAP_TOPIC_PREFIX) {
            let data_words = parse_u256_words(&log.data);
            if data_words.len() < 4 {
                stats.malformed_logs = stats.malformed_logs.saturating_add(1);
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
        } else if has_topic_prefix(topic0, UNISWAP_V3_SWAP_TOPIC_PREFIX)
            || has_topic_prefix(topic0, ALGEBRA_SWAP_TOPIC_PREFIX)
        {
            let signed_words = parse_i256_words(&log.data);
            if signed_words.len() < 2 {
                stats.malformed_logs = stats.malformed_logs.saturating_add(1);
                None
            } else {
                let (amount0_in, amount0_out) = split_signed_flow(signed_words[0]);
                let (amount1_in, amount1_out) = split_signed_flow(signed_words[1]);
                Some((
                    LiquidityEventKind::Swap,
                    LiquidityAmounts::Swap {
                        amount0_in,
                        amount1_in,
                        amount0_out,
                        amount1_out,
                    },
                ))
            }
        } else if has_topic_prefix(topic0, MINT_TOPIC_PREFIX) {
            let data_words = parse_u256_words(&log.data);
            if data_words.len() < 2 {
                stats.malformed_logs = stats.malformed_logs.saturating_add(1);
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
            let data_words = parse_u256_words(&log.data);
            if data_words.len() < 2 {
                stats.malformed_logs = stats.malformed_logs.saturating_add(1);
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
            stats.unknown_topic_logs = stats.unknown_topic_logs.saturating_add(1);
            None
        };

        if let Some((event_kind, amounts)) = parsed {
            stats.recognized_logs = stats.recognized_logs.saturating_add(1);
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

    (output, stats)
}

#[allow(dead_code)]
pub fn normalize_feature(feature: LiquidityFeature) -> NormalizedLiquidityFeature {
    normalize_feature_with_profile(feature, NormalizationProfile::default())
}

pub fn normalize_feature_with_profile(
    feature: LiquidityFeature,
    profile: NormalizationProfile,
) -> NormalizedLiquidityFeature {
    let (total_volume, imbalance) = match &feature.amounts {
        LiquidityAmounts::Swap {
            amount0_in,
            amount1_in,
            amount0_out,
            amount1_out,
        } => {
            let in_total = scale_amount(*amount0_in, profile.token0_decimals)
                + scale_amount(*amount1_in, profile.token1_decimals);
            let out_total = scale_amount(*amount0_out, profile.token0_decimals)
                + scale_amount(*amount1_out, profile.token1_decimals);
            let total = in_total + out_total;
            (total, abs_diff_f64(in_total, out_total))
        }
        LiquidityAmounts::AddLiquidity { amount0, amount1 }
        | LiquidityAmounts::RemoveLiquidity { amount0, amount1 } => {
            let amount0f = scale_amount(*amount0, profile.token0_decimals);
            let amount1f = scale_amount(*amount1, profile.token1_decimals);
            let total = amount0f + amount1f;
            (total, abs_diff_f64(amount0f, amount1f))
        }
    };

    let gas_used = feature.gas_used as f64;

    NormalizedLiquidityFeature {
        feature,
        normalized_total_volume: (1.0 + total_volume).ln(),
        normalized_imbalance: imbalance / (total_volume + 1.0),
        normalized_gas: (1.0 + gas_used).ln(),
    }
}

fn abs_diff_f64(a: f64, b: f64) -> f64 {
    (a - b).abs()
}

fn scale_amount(amount: u128, decimals: u8) -> f64 {
    let scale = 10f64.powi(decimals as i32);
    amount as f64 / scale
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

fn parse_i256_words(data: &str) -> Vec<i128> {
    let hex = data.strip_prefix("0x").unwrap_or(data);
    if hex.is_empty() || hex.len() % 64 != 0 {
        return Vec::new();
    }

    let mut words = Vec::with_capacity(hex.len() / 64);
    let mut i = 0;
    while i + 64 <= hex.len() {
        let word = &hex[i..i + 64];
        let low_16_bytes = &word[32..64];
        let low = match u128::from_str_radix(low_16_bytes, 16) {
            Ok(value) => value,
            Err(_) => return Vec::new(),
        };
        let sign_nibble = word.chars().next().unwrap_or('0');
        let negative = matches!(sign_nibble, '8'..='9' | 'a'..='f' | 'A'..='F');
        if negative {
            let magnitude = low.wrapping_neg();
            if magnitude > i128::MAX as u128 {
                return Vec::new();
            }
            words.push(-(magnitude as i128));
        } else {
            if low > i128::MAX as u128 {
                return Vec::new();
            }
            words.push(low as i128);
        }
        i += 64;
    }
    words
}

fn split_signed_flow(value: i128) -> (u128, u128) {
    if value < 0 {
        (0, value.unsigned_abs())
    } else {
        (value as u128, 0)
    }
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
    use crate::ingestion::rpc::TransactionReceipt;

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
    fn normalize_feature_respects_decimals_profile() {
        let feature = LiquidityFeature {
            tx_hash: "0xtx".to_string(),
            block_number: 1,
            pool_address: "0xpool".to_string(),
            log_index: None,
            event_kind: LiquidityEventKind::AddLiquidity,
            amounts: LiquidityAmounts::AddLiquidity {
                amount0: 1_000_000,
                amount1: 1_000_000,
            },
            gas_used: 100_000,
            tx_status: 1,
        };
        let n6 = normalize_feature_with_profile(
            feature.clone(),
            NormalizationProfile {
                token0_decimals: 6,
                token1_decimals: 6,
            },
        );
        let n18 = normalize_feature_with_profile(
            feature,
            NormalizationProfile {
                token0_decimals: 18,
                token1_decimals: 18,
            },
        );
        assert!(n6.normalized_total_volume > n18.normalized_total_volume);
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

    #[test]
    fn extraction_stats_track_unknown_and_malformed() {
        let receipt = TransactionReceipt {
            transaction_hash: "0xtx".to_string(),
            block_number: 1,
            gas_used: 21_000,
            status: 1,
            logs: vec![
                ReceiptLog {
                    address: "0xpool".to_string(),
                    topics: vec!["0xdeadbeef".to_string()],
                    data: "0x".to_string(),
                    log_index: Some(0),
                },
                ReceiptLog {
                    address: "0xpool".to_string(),
                    topics: vec![],
                    data: "0x1234".to_string(),
                    log_index: Some(1),
                },
            ],
        };
        let (_features, stats) = extract_features_with_stats(&receipt);
        assert_eq!(stats.logs_total, 2);
        assert_eq!(stats.unknown_topic_logs, 1);
        assert_eq!(stats.malformed_logs, 1);
    }

    #[test]
    fn extract_uniswap_v3_swap_like_topic() {
        let log = ReceiptLog {
            address: "0xpool".to_string(),
            topics: vec![
                "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67".to_string(),
            ],
            data: format!(
                "0x{}{}",
                hex_word(25),
                "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffec"
            ),
            log_index: Some(1),
        };
        let receipt = TransactionReceipt {
            transaction_hash: "0xtx".to_string(),
            block_number: 99,
            gas_used: 100_000,
            status: 1,
            logs: vec![log],
        };
        let features = extract_features_from_receipt(&receipt);
        assert_eq!(features.len(), 1);
        match &features[0].amounts {
            LiquidityAmounts::Swap {
                amount0_in,
                amount1_out,
                ..
            } => {
                assert_eq!(*amount0_in, 25);
                assert_eq!(*amount1_out, 20);
            }
            _ => panic!("expected swap"),
        }
    }

    #[test]
    fn fixture_swap_like_receipt_produces_features() {
        let raw = include_str!("../../tests/fixtures/receipt_swap_like_359066952.json");
        let receipt: TransactionReceipt = serde_json::from_str(raw).expect("valid fixture json");
        let (features, stats) = extract_features_with_stats(&receipt);
        assert_eq!(stats.logs_total, 1);
        assert_eq!(stats.recognized_logs, 1);
        assert_eq!(stats.unknown_topic_logs, 0);
        assert_eq!(stats.malformed_logs, 0);
        assert_eq!(features.len(), 1);
        assert_eq!(features[0].event_kind, LiquidityEventKind::Swap);
    }

    #[test]
    fn fixture_unknown_receipt_tracks_unknown_topics() {
        let raw = include_str!("../../tests/fixtures/receipt_unknown_topics_359066951.json");
        let receipt: TransactionReceipt = serde_json::from_str(raw).expect("valid fixture json");
        let (features, stats) = extract_features_with_stats(&receipt);
        assert!(features.is_empty());
        assert_eq!(stats.logs_total, 2);
        assert_eq!(stats.recognized_logs, 0);
        assert_eq!(stats.unknown_topic_logs, 2);
        assert_eq!(stats.malformed_logs, 0);
    }
}
