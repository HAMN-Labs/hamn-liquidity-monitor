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

#[derive(Debug, Clone, Copy)]
pub struct StabilizationConfig {
    pub confidence_decay_per_block: f64,
    pub min_confidence: f64,
    pub max_inactive_blocks: u64,
    pub min_occurrences_for_retention: u64,
    pub noise_inactive_blocks: u64,
    pub max_patterns: usize,
}

impl Default for StabilizationConfig {
    fn default() -> Self {
        Self {
            confidence_decay_per_block: 0.999,
            min_confidence: 0.08,
            max_inactive_blocks: 50_000,
            min_occurrences_for_retention: 2,
            noise_inactive_blocks: 2_000,
            max_patterns: 50_000,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryMetrics {
    pub active_patterns: usize,
    pub match_ratio: f64,
    pub churn_rate: f64,
    pub total_observations: u64,
    pub matched_observations: u64,
    pub created_observations: u64,
    pub pruned_patterns_total: u64,
}

#[derive(Debug, Clone)]
pub struct AdaptiveMemory {
    next_id: u64,
    patterns: Vec<Pattern>,
    pub distance_threshold: f64,
    pub stabilization: StabilizationConfig,
    total_observations: u64,
    matched_observations: u64,
    created_observations: u64,
    pruned_patterns_total: u64,
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
    pub fn new_with_config(distance_threshold: f64, stabilization: StabilizationConfig) -> Self {
        Self {
            next_id: 1,
            patterns: Vec::new(),
            distance_threshold,
            stabilization,
            total_observations: 0,
            matched_observations: 0,
            created_observations: 0,
            pruned_patterns_total: 0,
        }
    }

    pub fn from_snapshot_with_config(
        snapshot: MemorySnapshot,
        distance_threshold: f64,
        stabilization: StabilizationConfig,
    ) -> Self {
        Self {
            next_id: snapshot.next_id,
            patterns: snapshot.patterns,
            distance_threshold,
            stabilization,
            total_observations: 0,
            matched_observations: 0,
            created_observations: 0,
            pruned_patterns_total: 0,
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

    pub fn load_snapshot_with_config<P: AsRef<Path>>(
        path: P,
        distance_threshold: f64,
        stabilization: StabilizationConfig,
    ) -> Result<Self> {
        let raw = fs::read_to_string(path).context("failed to read memory snapshot")?;
        let snapshot: MemorySnapshot =
            serde_json::from_str(&raw).context("failed to deserialize memory snapshot")?;
        Ok(Self::from_snapshot_with_config(
            snapshot,
            distance_threshold,
            stabilization,
        ))
    }

    pub fn observe(&mut self, feature: &NormalizedLiquidityFeature) -> MatchOutcome {
        self.total_observations = self.total_observations.saturating_add(1);

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
                self.matched_observations = self.matched_observations.saturating_add(1);
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
                self.created_observations = self.created_observations.saturating_add(1);
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

    pub fn stabilize(&mut self, current_block: u64) -> usize {
        self.apply_decay(current_block);

        let before = self.patterns.len();
        let cfg = self.stabilization;

        self.patterns.retain(|pattern| {
            let inactivity = current_block.saturating_sub(pattern.last_seen_block);
            let noisy = pattern.occurrences < cfg.min_occurrences_for_retention
                && inactivity > cfg.noise_inactive_blocks;
            let stale = inactivity > cfg.max_inactive_blocks;
            let weak = pattern.confidence < cfg.min_confidence;
            !(noisy || stale || weak)
        });

        if self.patterns.len() > cfg.max_patterns {
            self.patterns.sort_by(|a, b| {
                let sa = retention_score(a, current_block);
                let sb = retention_score(b, current_block);
                sb.total_cmp(&sa)
            });
            self.patterns.truncate(cfg.max_patterns);
        }

        let pruned = before.saturating_sub(self.patterns.len());
        self.pruned_patterns_total = self.pruned_patterns_total.saturating_add(pruned as u64);
        pruned
    }

    pub fn metrics(&self) -> MemoryMetrics {
        let match_ratio = if self.total_observations == 0 {
            0.0
        } else {
            self.matched_observations as f64 / self.total_observations as f64
        };
        let churn_rate = if self.created_observations == 0 {
            0.0
        } else {
            self.pruned_patterns_total as f64 / self.created_observations as f64
        };

        MemoryMetrics {
            active_patterns: self.patterns.len(),
            match_ratio,
            churn_rate,
            total_observations: self.total_observations,
            matched_observations: self.matched_observations,
            created_observations: self.created_observations,
            pruned_patterns_total: self.pruned_patterns_total,
        }
    }

    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    fn apply_decay(&mut self, current_block: u64) {
        let decay = self.stabilization.confidence_decay_per_block.clamp(0.0, 1.0);
        for pattern in &mut self.patterns {
            let inactivity = current_block.saturating_sub(pattern.last_seen_block);
            if inactivity == 0 {
                continue;
            }
            let factor = decay.powf(inactivity as f64);
            pattern.confidence *= factor;
        }
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

fn retention_score(pattern: &Pattern, current_block: u64) -> f64 {
    let recency = 1.0 / (1.0 + current_block.saturating_sub(pattern.last_seen_block) as f64);
    pattern.confidence * 0.7 + recency * 0.2 + (pattern.occurrences as f64).ln_1p() * 0.1
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
        let mut memory = AdaptiveMemory::new_with_config(0.5, StabilizationConfig::default());
        let outcome = memory.observe(&sample_feature(1, 100, 80));
        match outcome {
            MatchOutcome::Created { pattern_id } => assert_eq!(pattern_id, 1),
            _ => panic!("expected Created"),
        }
        assert_eq!(memory.pattern_count(), 1);
    }

    #[test]
    fn matches_existing_pattern_for_close_vectors() {
        let mut memory = AdaptiveMemory::new_with_config(0.5, StabilizationConfig::default());
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
        let mut memory = AdaptiveMemory::new_with_config(0.5, StabilizationConfig::default());
        let _ = memory.observe(&sample_feature(1, 100, 80));
        let snapshot = memory.snapshot();

        let restored =
            AdaptiveMemory::from_snapshot_with_config(snapshot, 0.5, StabilizationConfig::default());
        assert_eq!(restored.pattern_count(), 1);
    }

    #[test]
    fn prunes_low_confidence_patterns_after_decay() {
        let cfg = StabilizationConfig {
            confidence_decay_per_block: 0.5,
            min_confidence: 0.2,
            max_inactive_blocks: 1_000,
            min_occurrences_for_retention: 1,
            noise_inactive_blocks: 1_000,
            max_patterns: 100,
        };
        let mut memory = AdaptiveMemory::new_with_config(0.5, cfg);
        let _ = memory.observe(&sample_feature(1, 100, 80));

        let pruned = memory.stabilize(10);
        assert_eq!(pruned, 1);
        assert_eq!(memory.pattern_count(), 0);
    }

    #[test]
    fn enforces_max_patterns_limit() {
        let cfg = StabilizationConfig {
            max_patterns: 2,
            min_occurrences_for_retention: 0,
            ..StabilizationConfig::default()
        };
        let mut memory = AdaptiveMemory::new_with_config(0.01, cfg);
        let _ = memory.observe(&sample_feature(1, 10, 10));
        let _ = memory.observe(&sample_feature(2, 1_000, 1_000));
        let _ = memory.observe(&sample_feature(3, 2_000, 2_000));

        let pruned = memory.stabilize(3);
        assert_eq!(pruned, 1);
        assert_eq!(memory.pattern_count(), 2);
    }

    #[test]
    fn metrics_include_match_ratio_and_churn() {
        let mut memory = AdaptiveMemory::new_with_config(0.5, StabilizationConfig::default());
        let _ = memory.observe(&sample_feature(1, 100, 80));
        let _ = memory.observe(&sample_feature(2, 101, 81));
        let _ = memory.stabilize(10_000);
        let m = memory.metrics();

        assert!(m.match_ratio > 0.0);
        assert!(m.active_patterns <= memory.pattern_count());
    }
}
