use crate::bubble::BubbleContent;
use crate::models::{BehaviorConfig, PetStats};
use rand::prelude::*;
use std::collections::HashMap;
use std::time::Instant;

/// Result of a behavior engine tick.
pub struct BehaviorTick {
    pub action: String,
    pub bubble: Option<BubbleContent>,
}

/// The autonomous behavior engine.
pub struct BehaviorEngine {
    pub config: BehaviorConfig,
    last_interaction: Instant,
    last_ambient_at: Instant,
    last_ambient_action: HashMap<String, Instant>,
    next_interval_ms: u64,
    rng: StdRng,
    pub ambient_enabled: bool,
    pub in_ambient_mode: bool,
}

impl BehaviorEngine {
    pub fn new(config: BehaviorConfig) -> Self {
        let mut rng = StdRng::from_rng(&mut rand::rng());
        let interval = rng.random_range(config.ambient_interval_min_ms..=config.ambient_interval_max_ms);
        let now = Instant::now();
        Self {
            config,
            last_interaction: now,
            last_ambient_at: now,
            last_ambient_action: HashMap::new(),
            next_interval_ms: interval,
            rng,
            ambient_enabled: true,
            in_ambient_mode: false,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(BehaviorConfig::default())
    }

    /// Call when user interacts. Resets idle timer.
    pub fn on_interaction(&mut self) {
        self.last_interaction = Instant::now();
        self.in_ambient_mode = false;
    }

    /// Tick the behavior engine. Returns an action to play if one was selected.
    pub fn tick(&mut self, stats: &PetStats) -> Option<BehaviorTick> {
        if !self.ambient_enabled {
            return None;
        }

        let idle_elapsed = self.last_interaction.elapsed().as_millis() as u64;

        // Check if we should enter ambient mode
        if !self.in_ambient_mode {
            if idle_elapsed >= self.config.idle_timeout_ms {
                self.in_ambient_mode = true;
                self.last_ambient_at = Instant::now();
                self.next_interval_ms = self.rng.random_range(
                    self.config.ambient_interval_min_ms..=self.config.ambient_interval_max_ms,
                );
            }
            return None;
        }

        // In ambient mode: check if it's time for an ambient action
        let ambient_elapsed = self.last_ambient_at.elapsed().as_millis() as u64;
        if ambient_elapsed < self.next_interval_ms {
            return None;
        }

        // Select an ambient action
        let mood_score = stats.mood_score();
        let result = self.select_action(mood_score);

        self.last_ambient_at = Instant::now();
        self.next_interval_ms = self.rng.random_range(
            self.config.ambient_interval_min_ms..=self.config.ambient_interval_max_ms,
        );

        result
    }

    fn select_action(&mut self, mood_score: f64) -> Option<BehaviorTick> {
        let now = Instant::now();

        // Filter actions by mood and cooldown
        let candidates: Vec<(usize, f64)> = self
            .config
            .ambient_actions
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                // Mood filter
                if let Some(min) = a.min_mood {
                    if mood_score < min {
                        return false;
                    }
                }
                if let Some(max) = a.max_mood {
                    if mood_score > max {
                        return false;
                    }
                }

                // Cooldown filter
                if let Some(last) = self.last_ambient_action.get(&a.action) {
                    if (last.elapsed().as_millis() as u64) < a.cooldown_ms {
                        return false;
                    }
                }

                true
            })
            .map(|(i, a)| {
                // Apply mood influence to weight
                let mood_factor = if self.config.mood_influence > 0.0 {
                    let mood_normalized = mood_score / 100.0;
                    1.0 + (mood_normalized - 0.5) * self.config.mood_influence
                } else {
                    1.0
                };
                (i, a.weight * mood_factor)
            })
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // Weighted random selection
        let total_weight: f64 = candidates.iter().map(|(_, w)| w).sum();
        let mut pick = self.rng.random_range(0.0..total_weight);
        let mut selected_idx = candidates[0].0;

        for (idx, weight) in &candidates {
            pick -= weight;
            if pick <= 0.0 {
                selected_idx = *idx;
                break;
            }
        }

        let action = &self.config.ambient_actions[selected_idx];
        self.last_ambient_action
            .insert(action.action.clone(), now);

        Some(BehaviorTick {
            action: action.action.clone(),
            bubble: action.bubble.as_ref().map(|text| BubbleContent::action(text)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AmbientAction;

    #[test]
    fn test_no_ambient_before_timeout() {
        let mut engine = BehaviorEngine::with_defaults();
        let stats = PetStats::default();
        // Should not trigger ambient before timeout
        assert!(engine.tick(&stats).is_none());
    }

    #[test]
    fn test_interaction_resets_idle() {
        let mut engine = BehaviorEngine::new(BehaviorConfig {
            idle_timeout_ms: 100,
            ambient_interval_min_ms: 50,
            ambient_interval_max_ms: 100,
            ..Default::default()
        });
        let stats = PetStats::default();

        // Wait for idle timeout
        std::thread::sleep(std::time::Duration::from_millis(150));
        engine.tick(&stats); // enters ambient mode

        // Interaction resets
        engine.on_interaction();
        assert!(!engine.in_ambient_mode);
    }

    #[test]
    fn test_mood_filters_actions() {
        let config = BehaviorConfig {
            idle_timeout_ms: 0,
            ambient_interval_min_ms: 0,
            ambient_interval_max_ms: 1,
            mood_influence: 0.0,
            ambient_actions: vec![AmbientAction {
                action: "waving".into(),
                weight: 1.0,
                bubble: None,
                min_mood: Some(80.0),
                max_mood: None,
                cooldown_ms: 0,
            }],
        };
        let mut engine = BehaviorEngine::new(config);
        engine.in_ambient_mode = true;
        engine.last_ambient_at = Instant::now() - std::time::Duration::from_millis(100);

        // Low mood → action filtered out
        let low_stats = PetStats {
            happiness: 10.0,
            energy: 10.0,
            social: 10.0,
            boredom: 90.0,
        };
        assert!(engine.tick(&low_stats).is_none());

        // High mood → action available
        let high_stats = PetStats {
            happiness: 100.0,
            energy: 100.0,
            social: 100.0,
            boredom: 0.0,
        };
        engine.last_ambient_at = Instant::now() - std::time::Duration::from_millis(100);
        assert!(engine.tick(&high_stats).is_some());
    }

    #[test]
    fn test_ambient_disabled() {
        let mut engine = BehaviorEngine::with_defaults();
        engine.ambient_enabled = false;
        engine.in_ambient_mode = true;
        let stats = PetStats::default();
        assert!(engine.tick(&stats).is_none());
    }
}
