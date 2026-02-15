use crate::features::extractor::LiquidityEventKind;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SequenceEventKind {
    Swap,
    AddLiquidity,
    RemoveLiquidity,
}

#[derive(Debug, Clone, Copy)]
pub struct SequenceMetrics {
    pub tracked_entities: usize,
    pub total_transitions: u64,
    pub unique_transitions: usize,
}

#[derive(Debug, Clone)]
pub struct TransitionModel {
    last_event_by_entity: HashMap<String, SequenceEventKind>,
    transition_counts: HashMap<(SequenceEventKind, SequenceEventKind), u64>,
    outgoing_counts: HashMap<SequenceEventKind, u64>,
    total_transitions: u64,
}

impl TransitionModel {
    pub fn new() -> Self {
        Self {
            last_event_by_entity: HashMap::new(),
            transition_counts: HashMap::new(),
            outgoing_counts: HashMap::new(),
            total_transitions: 0,
        }
    }

    pub fn observe_entity_event(&mut self, entity_key: String, next: SequenceEventKind) {
        if let Some(prev) = self.last_event_by_entity.get(&entity_key).copied() {
            let key = (prev, next);
            let count = self.transition_counts.entry(key).or_insert(0);
            *count = count.saturating_add(1);

            let out_count = self.outgoing_counts.entry(prev).or_insert(0);
            *out_count = out_count.saturating_add(1);
            self.total_transitions = self.total_transitions.saturating_add(1);
        }

        self.last_event_by_entity.insert(entity_key, next);
    }

    pub fn transition_probability(
        &self,
        from: SequenceEventKind,
        to: SequenceEventKind,
    ) -> f64 {
        let total_outgoing = self.outgoing_counts.get(&from).copied().unwrap_or(0) as f64;
        if total_outgoing == 0.0 {
            return 0.0;
        }
        let transition = self
            .transition_counts
            .get(&(from, to))
            .copied()
            .unwrap_or(0) as f64;
        transition / total_outgoing
    }

    pub fn smoothed_probability(
        &self,
        from: SequenceEventKind,
        to: SequenceEventKind,
        alpha: f64,
    ) -> f64 {
        let k = 3.0;
        let alpha = alpha.max(0.0);
        let total_outgoing = self.outgoing_counts.get(&from).copied().unwrap_or(0) as f64;
        let transition = self
            .transition_counts
            .get(&(from, to))
            .copied()
            .unwrap_or(0) as f64;
        (transition + alpha) / (total_outgoing + alpha * k)
    }

    pub fn metrics(&self) -> SequenceMetrics {
        SequenceMetrics {
            tracked_entities: self.last_event_by_entity.len(),
            total_transitions: self.total_transitions,
            unique_transitions: self.transition_counts.len(),
        }
    }
}

impl SequenceEventKind {
    pub fn from_liquidity_event(value: &LiquidityEventKind) -> Self {
        match value {
            LiquidityEventKind::Swap => Self::Swap,
            LiquidityEventKind::AddLiquidity => Self::AddLiquidity,
            LiquidityEventKind::RemoveLiquidity => Self::RemoveLiquidity,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Swap => "swap",
            Self::AddLiquidity => "add_liquidity",
            Self::RemoveLiquidity => "remove_liquidity",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_transitions_by_entity() {
        let mut model = TransitionModel::new();
        model.observe_entity_event("pool-a".to_string(), SequenceEventKind::Swap);
        model.observe_entity_event("pool-a".to_string(), SequenceEventKind::AddLiquidity);
        model.observe_entity_event("pool-a".to_string(), SequenceEventKind::RemoveLiquidity);

        assert_eq!(model.metrics().total_transitions, 2);
        assert_eq!(
            model.transition_probability(
                SequenceEventKind::Swap,
                SequenceEventKind::AddLiquidity
            ),
            1.0
        );
    }

    #[test]
    fn calculates_smoothed_probability_for_rare_edges() {
        let mut model = TransitionModel::new();
        model.observe_entity_event("pool-a".to_string(), SequenceEventKind::Swap);
        model.observe_entity_event("pool-a".to_string(), SequenceEventKind::AddLiquidity);

        let raw = model.transition_probability(
            SequenceEventKind::Swap,
            SequenceEventKind::RemoveLiquidity,
        );
        let smooth = model.smoothed_probability(
            SequenceEventKind::Swap,
            SequenceEventKind::RemoveLiquidity,
            0.5,
        );

        assert_eq!(raw, 0.0);
        assert!(smooth > 0.0);
    }

    #[test]
    fn keeps_sequences_isolated_per_entity() {
        let mut model = TransitionModel::new();
        model.observe_entity_event("pool-a".to_string(), SequenceEventKind::Swap);
        model.observe_entity_event("pool-b".to_string(), SequenceEventKind::Swap);
        model.observe_entity_event("pool-a".to_string(), SequenceEventKind::AddLiquidity);

        let metrics = model.metrics();
        assert_eq!(metrics.tracked_entities, 2);
        assert_eq!(metrics.total_transitions, 1);
    }
}
