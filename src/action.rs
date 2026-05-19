use crate::models::ActionDef;
use std::collections::HashMap;

fn default_actions() -> Vec<ActionDef> {
    vec![
        ActionDef { name: "idle".into(),         row: 0, frame_count: 6, frame_duration_ms: 120, looping: true,  interruptible: true,  loop_rest_ms: Some(500), last_frame_hold_ms: None },
        ActionDef { name: "running_right".into(),         row: 1, frame_count: 9, frame_duration_ms: 100, looping: true,  interruptible: true,  loop_rest_ms: None,      last_frame_hold_ms: None },
        ActionDef { name: "running_left".into(),         row: 2, frame_count: 9, frame_duration_ms: 80,  looping: true,  interruptible: false, loop_rest_ms: None,      last_frame_hold_ms: None },
        ActionDef { name: "waving".into(),         row: 3, frame_count: 4, frame_duration_ms: 100, looping: false, interruptible: true,  loop_rest_ms: None,      last_frame_hold_ms: Some(200) },
        ActionDef { name: "jumping".into(),        row: 4, frame_count: 5, frame_duration_ms: 100, looping: false, interruptible: true,  loop_rest_ms: None,      last_frame_hold_ms: Some(300) },
        ActionDef { name: "failed".into(), row: 5, frame_count: 9, frame_duration_ms: 100, looping: false, interruptible: true,  loop_rest_ms: None,      last_frame_hold_ms: Some(200) },
        ActionDef { name: "waiting".into(),        row: 6, frame_count: 6, frame_duration_ms: 200, looping: true,  interruptible: true,  loop_rest_ms: None,      last_frame_hold_ms: None },
        ActionDef { name: "running".into(),         row: 7, frame_count: 6, frame_duration_ms: 120, looping: false, interruptible: true,  loop_rest_ms: None,      last_frame_hold_ms: Some(400) },
        ActionDef { name: "review".into(),      row: 8, frame_count: 6, frame_duration_ms: 100, looping: false, interruptible: true,  loop_rest_ms: None,      last_frame_hold_ms: Some(300) },
    ]
}

#[derive(Debug)]
pub struct ActionRegistry {
    actions: HashMap<String, ActionDef>,
    #[allow(dead_code)]
    row_index: Vec<Option<String>>,
}

impl ActionRegistry {
    pub fn new(actions: Vec<ActionDef>) -> Self {
        let max_row = actions.iter().map(|a| a.row).max().unwrap_or(0);
        let mut row_index: Vec<Option<String>> = vec![None; (max_row + 1) as usize];
        let mut map = HashMap::new();

        for action in actions {
            row_index[action.row as usize] = Some(action.name.clone());
            map.insert(action.name.clone(), action);
        }

        Self {
            actions: map,
            row_index,
        }
    }

    pub fn default_registry() -> Self {
        Self::new(default_actions())
    }

    /// Create a registry using default actions but with frame counts overridden
    /// from the detected sprite sheet layout. `row_frame_counts[i]` = actual frames in row i.
    pub fn with_detected_frames(row_frame_counts: &[u32]) -> Self {
        let mut actions = default_actions();
        for action in &mut actions {
            if let Some(&detected) = row_frame_counts.get(action.row as usize) {
                if detected > 0 && detected != action.frame_count {
                    action.frame_count = detected;
                }
            }
        }
        Self::new(actions)
    }

    pub fn get(&self, name: &str) -> Option<&ActionDef> {
        self.actions.get(name)
    }

    #[allow(dead_code)]
    pub fn get_by_row(&self, row: u32) -> Option<&ActionDef> {
        self.row_index
            .get(row as usize)
            .and_then(|name| name.as_ref())
            .and_then(|name| self.actions.get(name))
    }

    #[allow(dead_code)]
    pub fn action_names(&self) -> Vec<&str> {
        self.actions.keys().map(|s| s.as_str()).collect()
    }

    pub fn action_defs(&self) -> Vec<ActionDef> {
        self.actions.values().cloned().collect()
    }
}

#[derive(Debug, Clone)]
pub struct ActionPlayer {
    pub current_action: String,
    pub current_frame: u32,
    pub elapsed_ms: u64,
    pub finished: bool,
    pub loop_rest_remaining_ms: u64,
}

impl ActionPlayer {
    pub fn new(action: &str) -> Self {
        Self {
            current_action: action.to_string(),
            current_frame: 0,
            elapsed_ms: 0,
            finished: false,
            loop_rest_remaining_ms: 0,
        }
    }

    pub fn tick(&mut self, delta_ms: u64, registry: &ActionRegistry) -> bool {
        let action = match registry.get(&self.current_action) {
            Some(a) => a,
            None => return false,
        };

        if self.finished {
            return false;
        }

        // Handle loop rest: hold last frame during rest period
        if self.loop_rest_remaining_ms > 0 {
            let consumed = delta_ms.min(self.loop_rest_remaining_ms);
            self.loop_rest_remaining_ms -= consumed;
            if self.loop_rest_remaining_ms == 0 {
                // Rest complete - wrap to frame 0
                self.current_frame = 0;
                self.elapsed_ms = 0;
                return true;
            }
            return false;
        }

        self.elapsed_ms += delta_ms;
        let frame_dur = action.frame_duration_ms;

        if self.elapsed_ms >= frame_dur {
            let frames_advance = (self.elapsed_ms / frame_dur) as u32;
            self.elapsed_ms %= frame_dur;

            let new_frame = self.current_frame + frames_advance;

            if new_frame >= action.frame_count {
                if action.looping {
                    let rest_ms = action.loop_rest_ms.unwrap_or(0);
                    if rest_ms > 0 {
                        // Clamp to last frame and start rest
                        self.current_frame = action.frame_count - 1;
                        self.loop_rest_remaining_ms = rest_ms;
                        return true;
                    } else {
                        self.current_frame = new_frame % action.frame_count;
                    }
                } else {
                    self.current_frame = action.frame_count - 1;
                    self.finished = true;
                }
            } else {
                self.current_frame = new_frame;
            }
            true
        } else {
            false
        }
    }

    pub fn switch_to(&mut self, action: &str, registry: &ActionRegistry) -> bool {
        if let Some(current) = registry.get(&self.current_action) {
            if !current.interruptible && !self.finished {
                return false;
            }
        }

        if registry.get(action).is_none() {
            return false;
        }

        self.current_action = action.to_string();
        self.current_frame = 0;
        self.elapsed_ms = 0;
        self.finished = false;
        self.loop_rest_remaining_ms = 0;
        true
    }

    #[allow(dead_code)]
    pub fn restart(&mut self) {
        self.current_frame = 0;
        self.elapsed_ms = 0;
        self.finished = false;
        self.loop_rest_remaining_ms = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_registry() {
        let registry = ActionRegistry::default_registry();
        assert!(registry.get("idle").is_some());
        assert!(registry.get("running_right").is_some());
        assert!(registry.get("nonexistent").is_none());
        assert_eq!(registry.get_by_row(0).unwrap().name, "idle");
    }

    #[test]
    fn test_action_player_tick() {
        let registry = ActionRegistry::default_registry();
        let mut player = ActionPlayer::new("idle");
        assert!(!player.tick(50, &registry));
        assert_eq!(player.current_frame, 0);
        assert!(player.tick(80, &registry));
        assert_eq!(player.current_frame, 1);
    }

    #[test]
    fn test_action_player_loop() {
        let registry = ActionRegistry::default_registry();
        let mut player = ActionPlayer::new("idle");
        for _ in 0..8 {
            player.tick(120, &registry);
        }
        assert!(player.current_frame < 8);
        assert!(!player.finished);
    }

    #[test]
    fn test_action_player_no_loop() {
        let registry = ActionRegistry::default_registry();
        let mut player = ActionPlayer::new("waving");
        // waving: 4 frames at 100ms, non-looping
        for _ in 0..4 {
            player.tick(100, &registry);
        }
        assert!(player.finished);
        assert_eq!(player.current_frame, 3);
    }

    #[test]
    fn test_action_player_switch() {
        let registry = ActionRegistry::default_registry();
        let mut player = ActionPlayer::new("idle");
        player.tick(120, &registry);
        assert!(player.switch_to("running_right", &registry));
        assert_eq!(player.current_action, "running_right");
        assert_eq!(player.current_frame, 0);
    }

    #[test]
    fn test_action_player_cannot_interrupt() {
        let registry = ActionRegistry::default_registry();
        let mut player = ActionPlayer::new("running_left");
        // running_left is non-interruptible
        player.tick(80, &registry);
        assert!(!player.switch_to("idle", &registry));
        assert_eq!(player.current_action, "running_left");
    }

    #[test]
    fn test_loop_rest_holds_last_frame() {
        let registry = ActionRegistry::default_registry();
        let mut player = ActionPlayer::new("idle");
        // idle: 6 frames at 120ms, last frame is 5
        for _ in 0..5 {
            player.tick(120, &registry);
        }
        assert_eq!(player.current_frame, 5);
        assert_eq!(player.loop_rest_remaining_ms, 0);

        // Next tick should trigger loop rest (idle has 500ms rest)
        player.tick(120, &registry);
        assert_eq!(player.current_frame, 5); // Still at last frame
        assert_eq!(player.loop_rest_remaining_ms, 500);

        // During rest, frame doesn't change
        assert!(!player.tick(200, &registry));
        assert_eq!(player.current_frame, 5);
        assert_eq!(player.loop_rest_remaining_ms, 300);

        // Rest expires, wraps to frame 0
        assert!(player.tick(300, &registry));
        assert_eq!(player.current_frame, 0);
        assert_eq!(player.loop_rest_remaining_ms, 0);
    }

    #[test]
    fn test_loop_rest_reset_on_switch() {
        let registry = ActionRegistry::default_registry();
        let mut player = ActionPlayer::new("idle");
        // Advance to last frame and trigger rest
        for _ in 0..5 {
            player.tick(120, &registry);
        }
        player.tick(120, &registry);
        assert!(player.loop_rest_remaining_ms > 0);

        // Switch to another action should reset rest
        player.switch_to("running_right", &registry);
        assert_eq!(player.loop_rest_remaining_ms, 0);
        assert_eq!(player.current_frame, 0);
    }

    #[test]
    fn test_no_loop_rest_when_none() {
        let registry = ActionRegistry::default_registry();
        let mut player = ActionPlayer::new("running_right");
        // running_right: 9 frames at 100ms, loop_rest_ms: None
        for _ in 0..9 {
            player.tick(100, &registry);
        }
        // Should have wrapped to frame 0 without rest
        assert_eq!(player.current_frame, 0);
        assert_eq!(player.loop_rest_remaining_ms, 0);
        assert!(!player.finished);
    }

    #[test]
    fn test_non_looping_finishes_at_last_frame() {
        let registry = ActionRegistry::default_registry();
        let mut player = ActionPlayer::new("waving");
        // waving is non-looping with 4 frames at 100ms
        for _ in 0..4 {
            player.tick(100, &registry);
        }
        assert_eq!(player.current_frame, 3);
        assert!(player.finished);
    }
}
