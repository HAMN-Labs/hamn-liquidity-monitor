use crate::features::extractor::{LiquidityEventKind, NormalizedLiquidityFeature};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub id: u64,
    pub event_kind: PatternEventKind,
    pub centroid: [f64; 3],
    pub occurrences: u64,
    pub confidence: f64,
    pub last_seen_block: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternEventKind {
    Swap,
    AddLiquidity,
    RemoveLiquidity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    pub next_id: u64,
    pub patterns: Vec<Pattern>,
}

#[derive(Debug, Clone)]
pub struct AdaptiveMemory {
    next_id: u64,
    patterns: Vec<Pattern>,
    pub distance_threshold: f64,
}

#[derive(Debug, Clone)]
pub enum MatchOutcome {
    Matched {
        pattern_id: u64,
        distance: f64,
        confidence: f64,
    },
    Created {
        pattern_id: u64,
    },
}

impl AdaptiveMemory {
    pub fn new(distance_threshold: f64) -> Self {
        Self {
            next_id: 1,
            patterns: Vec::new(),
            distance_threshold,
        }
    }

    pub fn from_snapshot(snapshot: MemorySnapshot, distance_threshold: f64) -> Self {
        Self {
            next_id: snapshot.next_id,
            patterns: snapshot.patterns,
            distance_threshold,
        }
    }

    pub fn snapshot(&self) -> MemorySnapshot {
        MemorySnapshot {
            next_id: self.next_id,
            patterns: self.patterns.clone(),
        }
    }

    pub fn save_snapshot<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let serialized = serde_json::to_string_pretty(&self.snapshot())
            .context("failed to serialize memory snapshot")?;
        fs::write(path, serialized).context("failed to write memory snapshot")
    }

    pub fn load_snapshot<P: AsRef<Path>>(path: P, distance_threshold: f64) -> Result<Self> {
        let raw = fs::read_to_string(path).context("failed to read memory snapshot")?;
        let snapshot: MemorySnapshot =
            serde_json::from_str(&raw).context("failed to deserialize memory snapshot")?;
        Ok(Self::from_snapshot(snapshot, distance_threshold))
    }

    pub fn observe(&mut self, feature: &NormalizedLiquidityFeature) -> MatchOutcome {
        let event_kind = PatternEventKind::from_feature(feature);
        let vector = vector_from_feature(feature);

        let best = self
            .patterns
            .iter()
            .enumerate()
            .filter(|(_, pattern)| pattern.event_kind == event_kind)
            .map(|(idx, pattern)| (idx, euclidean_distance(pattern.centroid, vector)))
            .min_by(|a, b| a.1.total_cmp(&b.1));

        match best {
            Some((idx, distance)) if distance <= self.distance_threshold => {
                let pattern = &mut self.patterns[idx];
                pattern.occurrences = pattern.occurrences.saturating_add(1);
                pattern.centroid = update_centroid(pattern.centroid, vector, pattern.occurrences);
                pattern.last_seen_block = feature.feature.block_number;
                pattern.confidence = confidence_from_occurrences(pattern.occurrences);

                MatchOutcome::Matched {
                    pattern_id: pattern.id,
                    distance,
                    confidence: pattern.confidence,
                }
            }
            _ => {
                let id = self.next_id;
                self.next_id = self.next_id.saturating_add(1);
                self.patterns.push(Pattern {
                    id,
                    event_kind,
                    centroid: vector,
                    occurrences: 1,
                    confidence: confidence_from_occurrences(1),
                    last_seen_block: feature.feature.block_number,
                });
                MatchOutcome::Created { pattern_id: id }
            }
        }
    }

    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }
}

impl PatternEventKind {
    fn from_feature(feature: &NormalizedLiquidityFeature) -> Self {
        match &feature.feature.event_kind {
            LiquidityEventKind::Swap => Self::Swap,
            LiquidityEventKind::AddLiquidity => Self::AddLiquidity,
            LiquidityEventKind::RemoveLiquidity => Self::RemoveLiquidity,
        }
    }
}

fn vector_from_feature(feature: &NormalizedLiquidityFeature) -> [f64; 3] {
    [
        feature.normalized_total_volume,
        feature.normalized_imbalance,
        feature.normalized_gas,
    ]
}

fn euclidean_distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn update_centroid(current: [f64; 3], sample: [f64; 3], occurrences: u64) -> [f64; 3] {
    let n_prev = (occurrences.saturating_sub(1)) as f64;
    let n_curr = occurrences as f64;
    [
        (current[0] * n_prev + sample[0]) / n_curr,
        (current[1] * n_prev + sample[1]) / n_curr,
        (current[2] * n_prev + sample[2]) / n_curr,
    ]
}

fn confidence_from_occurrences(occurrences: u64) -> f64 {
    (occurrences as f64 / (occurrences as f64 + 5.0)).min(0.99)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::extractor::{
        LiquidityAmounts, LiquidityEventKind, LiquidityFeature, normalize_feature,
    };

    fn sample_feature(block: u64, amount0: u128, amount1: u128) -> NormalizedLiquidityFeature {
        let f = LiquidityFeature {
            tx_hash: format!("0x{block}"),
            block_number: block,
            pool_address: "0xpool".to_string(),
            log_index: Some(0),
            event_kind: LiquidityEventKind::AddLiquidity,
            amounts: LiquidityAmounts::AddLiquidity { amount0, amount1 },
            gas_used: 100_000,
            tx_status: 1,
        };
        normalize_feature(f)
    }

    #[test]
    fn creates_pattern_when_memory_empty() {
        let mut memory = AdaptiveMemory::new(0.5);
        let outcome = memory.observe(&sample_feature(1, 100, 80));
        match outcome {
            MatchOutcome::Created { pattern_id } => assert_eq!(pattern_id, 1),
            _ => panic!("expected Created"),
        }
        assert_eq!(memory.pattern_count(), 1);
    }

    #[test]
    fn matches_existing_pattern_for_close_vectors() {
        let mut memory = AdaptiveMemory::new(0.5);
        let _ = memory.observe(&sample_feature(1, 100, 80));
        let outcome = memory.observe(&sample_feature(2, 101, 81));
        match outcome {
            MatchOutcome::Matched {
                pattern_id,
                confidence,
                ..
            } => {
                assert_eq!(pattern_id, 1);
                assert!(confidence > 0.0);
            }
            _ => panic!("expected Matched"),
        }
        assert_eq!(memory.pattern_count(), 1);
    }

    #[test]
    fn serializes_and_restores_snapshot() {
        let mut memory = AdaptiveMemory::new(0.5);
        let _ = memory.observe(&sample_feature(1, 100, 80));
        let snapshot = memory.snapshot();

        let restored = AdaptiveMemory::from_snapshot(snapshot, 0.5);
        assert_eq!(restored.pattern_count(), 1);
    }
}
