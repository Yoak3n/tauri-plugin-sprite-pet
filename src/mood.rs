use crate::error::Result;
use crate::models::{MoodConfig, PetSnapshot, PetStats};
use std::path::PathBuf;
use std::time::Instant;

/// Tracks mood stats over time. Integrates into the runtime loop.
pub struct MoodTracker {
    pub stats: PetStats,
    pub config: MoodConfig,
    last_update: Instant,
    is_idle: bool,
}

impl MoodTracker {
    pub fn new(stats: PetStats, config: MoodConfig) -> Self {
        Self {
            stats,
            config,
            last_update: Instant::now(),
            is_idle: false,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(PetStats::default(), MoodConfig::default())
    }

    pub fn set_idle(&mut self, idle: bool) {
        self.is_idle = idle;
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        let delta_secs = now.duration_since(self.last_update).as_secs_f64();
        self.last_update = now;

        if delta_secs <= 0.0 {
            return;
        }

        let interval_secs = self.config.decay_interval_ms as f64 / 1000.0;
        if interval_secs <= 0.0 {
            return;
        }
        let intervals = delta_secs / interval_secs;

        self.stats.happiness -= self.config.happiness_decay * intervals;
        self.stats.energy -= self.config.energy_decay * intervals;
        self.stats.social -= self.config.social_decay * intervals;

        if self.is_idle {
            self.stats.boredom += self.config.boredom_increase * intervals;
            if self.stats.boredom > 60.0 {
                self.stats.happiness -= self.config.happiness_decay * intervals * 0.3;
            }
        } else {
            self.stats.boredom -= self.config.boredom_increase * intervals * 0.5;
        }

        self.stats.clamp();
    }

    pub fn on_interaction(&mut self) {
        self.stats.happiness += self.config.interaction_boost;
        self.stats.social += self.config.interaction_boost * 0.5;
        self.stats.boredom -= self.config.interaction_boost * 2.0;
        self.stats.clamp();
    }
}

/// Persists pet state to disk.
pub struct PetStore {
    cache_dir: PathBuf,
}

impl PetStore {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    pub fn default_store() -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("sprite-pet");
        Self { cache_dir }
    }

    fn snapshot_path(&self, pet_id: &str) -> PathBuf {
        self.cache_dir.join(pet_id).join("state.json")
    }

    pub fn save(&self, snapshot: &PetSnapshot) -> Result<()> {
        let path = self.snapshot_path(&snapshot.pet_id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(snapshot)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    pub fn load(&self, pet_id: &str) -> Result<Option<PetSnapshot>> {
        let path = self.snapshot_path(pet_id);
        if !path.exists() {
            return Ok(None);
        }
        let json = std::fs::read_to_string(&path)?;
        let snapshot: PetSnapshot = serde_json::from_str(&json)?;
        Ok(Some(snapshot))
    }

    pub fn delete(&self, pet_id: &str) -> Result<()> {
        let path = self.snapshot_path(pet_id);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Facing, Position};

    #[test]
    fn test_stats_clamp() {
        let mut stats = PetStats::default();
        stats.happiness = 150.0;
        stats.energy = -10.0;
        stats.clamp();
        assert_eq!(stats.happiness, 100.0);
        assert_eq!(stats.energy, 0.0);
    }

    #[test]
    fn test_mood_score() {
        let stats = PetStats {
            happiness: 100.0,
            energy: 100.0,
            social: 100.0,
            boredom: 0.0,
        };
        // Formula: 100*0.4 + 100*0.2 + 100*0.3 - 0*0.1 = 90.0
        assert_eq!(stats.mood_score(), 90.0);
        assert_eq!(stats.mood_label(), "ecstatic");
    }

    #[test]
    fn test_mood_label_sad() {
        let stats = PetStats {
            happiness: 10.0,
            energy: 10.0,
            social: 10.0,
            boredom: 90.0,
        };
        assert_eq!(stats.mood_label(), "depressed");
    }

    #[test]
    fn test_interaction_boost() {
        let mut tracker = MoodTracker::with_defaults();
        let before = tracker.stats.happiness;
        let before_boredom = tracker.stats.boredom;
        tracker.on_interaction();
        assert!(tracker.stats.happiness > before);
        assert!(tracker.stats.boredom < before_boredom);
    }

    #[test]
    fn test_pet_store_save_load() {
        let store = PetStore::new(PathBuf::from("test_store"));
        let snapshot = PetSnapshot {
            pet_id: "test-pet".into(),
            position: Position { x: 100.0, y: 200.0 },
            facing: Facing::Left,
            stats: PetStats::default(),
            current_action: "idle".into(),
            saved_at: "2026-01-01T00:00:00Z".into(),
        };
        store.save(&snapshot).unwrap();
        let loaded = store.load("test-pet").unwrap().unwrap();
        assert_eq!(loaded.pet_id, "test-pet");
        assert_eq!(loaded.position.x, 100.0);
        store.delete("test-pet").unwrap();
        assert!(store.load("test-pet").unwrap().is_none());
        std::fs::remove_dir_all("test_store").ok();
    }
}
