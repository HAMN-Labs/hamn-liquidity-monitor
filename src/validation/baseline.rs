use crate::features::extractor::{LiquidityEventKind, extract_features_with_stats};
use crate::ingestion::rpc::TransactionReceipt;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Deserialize)]
pub struct ValidationSet {
    pub cases: Vec<ValidationCase>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ValidationCase {
    pub name: String,
    pub receipt_file: String,
    pub expected: ExpectedCounts,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExpectedCounts {
    #[serde(default)]
    pub swap: u64,
    #[serde(default)]
    pub add_liquidity: u64,
    #[serde(default)]
    pub remove_liquidity: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct ValidationReport {
    pub cases: usize,
    pub tp: u64,
    pub fp: u64,
    pub fn_: u64,
    pub precision_proxy: f64,
    pub recall_proxy: f64,
    pub recognized_logs: u64,
    pub unknown_topic_logs: u64,
    pub malformed_logs: u64,
}

pub fn run_validation_set(path: &str, min_precision: f64, min_recall: f64) -> Result<ValidationReport> {
    let raw = fs::read_to_string(path).with_context(|| format!("failed to read validation set: {path}"))?;
    let set: ValidationSet =
        serde_json::from_str(&raw).with_context(|| format!("failed to parse validation set: {path}"))?;

    if set.cases.is_empty() {
        bail!("validation set has no cases");
    }

    let mut tp = 0u64;
    let mut fp = 0u64;
    let mut fn_ = 0u64;
    let mut recognized_logs = 0u64;
    let mut unknown_topic_logs = 0u64;
    let mut malformed_logs = 0u64;

    for case in &set.cases {
        let receipt_raw = fs::read_to_string(&case.receipt_file)
            .with_context(|| format!("failed to read receipt fixture: {}", case.receipt_file))?;
        let receipt: TransactionReceipt = serde_json::from_str(&receipt_raw)
            .with_context(|| format!("failed to parse receipt fixture: {}", case.receipt_file))?;

        let (features, stats) = extract_features_with_stats(&receipt);
        recognized_logs = recognized_logs.saturating_add(stats.recognized_logs);
        unknown_topic_logs = unknown_topic_logs.saturating_add(stats.unknown_topic_logs);
        malformed_logs = malformed_logs.saturating_add(stats.malformed_logs);

        let predicted = count_predicted(&features);
        let expected = &case.expected;

        let (case_tp, case_fp, case_fn) = compare_counts(&predicted, expected);
        tp = tp.saturating_add(case_tp);
        fp = fp.saturating_add(case_fp);
        fn_ = fn_.saturating_add(case_fn);

        println!(
            "validation_case name={} predicted_swap={} predicted_add={} predicted_remove={} expected_swap={} expected_add={} expected_remove={}",
            case.name,
            predicted.get("swap").copied().unwrap_or(0),
            predicted.get("add_liquidity").copied().unwrap_or(0),
            predicted.get("remove_liquidity").copied().unwrap_or(0),
            expected.swap,
            expected.add_liquidity,
            expected.remove_liquidity,
        );
    }

    let precision_proxy = if tp + fp == 0 {
        0.0
    } else {
        tp as f64 / (tp + fp) as f64
    };
    let recall_proxy = if tp + fn_ == 0 {
        0.0
    } else {
        tp as f64 / (tp + fn_) as f64
    };

    println!(
        "validation_metrics cases={} tp={} fp={} fn={} precision_proxy={:.4} recall_proxy={:.4} recognized_logs={} unknown_topic_logs={} malformed_logs={}",
        set.cases.len(),
        tp,
        fp,
        fn_,
        precision_proxy,
        recall_proxy,
        recognized_logs,
        unknown_topic_logs,
        malformed_logs,
    );

    if precision_proxy < min_precision || recall_proxy < min_recall {
        bail!(
            "validation failed: precision_proxy={:.4} (min {:.4}), recall_proxy={:.4} (min {:.4})",
            precision_proxy,
            min_precision,
            recall_proxy,
            min_recall
        );
    }

    Ok(ValidationReport {
        cases: set.cases.len(),
        tp,
        fp,
        fn_,
        precision_proxy,
        recall_proxy,
        recognized_logs,
        unknown_topic_logs,
        malformed_logs,
    })
}

fn count_predicted(features: &[crate::features::extractor::LiquidityFeature]) -> HashMap<&'static str, u64> {
    let mut out: HashMap<&'static str, u64> = HashMap::new();
    for feature in features {
        let key = match feature.event_kind {
            LiquidityEventKind::Swap => "swap",
            LiquidityEventKind::AddLiquidity => "add_liquidity",
            LiquidityEventKind::RemoveLiquidity => "remove_liquidity",
        };
        let count = out.entry(key).or_insert(0u64);
        *count = count.saturating_add(1);
    }
    out
}

fn compare_counts(predicted: &HashMap<&'static str, u64>, expected: &ExpectedCounts) -> (u64, u64, u64) {
    let kinds = [
        ("swap", expected.swap),
        ("add_liquidity", expected.add_liquidity),
        ("remove_liquidity", expected.remove_liquidity),
    ];

    let mut tp = 0u64;
    let mut fp = 0u64;
    let mut fn_ = 0u64;

    for (kind, exp) in kinds {
        let pred = predicted.get(kind).copied().unwrap_or(0);
        tp = tp.saturating_add(pred.min(exp));
        if pred > exp {
            fp = fp.saturating_add(pred - exp);
        } else if exp > pred {
            fn_ = fn_.saturating_add(exp - pred);
        }
    }
    (tp, fp, fn_)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_set_passes_for_current_fixtures() {
        let report = run_validation_set("tests/fixtures/validation_set.json", 0.5, 0.5)
            .expect("validation should pass");
        assert!(report.precision_proxy >= 0.5);
        assert!(report.recall_proxy >= 0.5);
        assert!(report.cases >= 2);
    }
}
