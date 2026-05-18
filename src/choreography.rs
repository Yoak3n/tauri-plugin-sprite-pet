use crate::bubble::{BubbleContent, BubblePriority};
use crate::models::{ActionSequence, BubbleDef, BubbleKind, SequenceRepeat, SequenceStep, SoundTrigger};

impl BubbleDef {
    pub fn to_content(&self) -> BubbleContent {
        BubbleContent {
            text: self.text.clone(),
            kind: self.kind,
            duration_ms: self.duration_ms,
            typing_animation: self.typing_animation,
            priority: BubblePriority::Normal,
        }
    }
}

impl SequenceStep {
    pub fn action(name: impl Into<String>) -> Self {
        Self {
            action: name.into(),
            loops: None,
            sound: None,
            bubble: None,
            wait_for_complete: true,
            delay_ms: 0,
        }
    }

    pub fn with_loops(mut self, n: u32) -> Self {
        self.loops = Some(n);
        self
    }

    pub fn with_sound(mut self, sound_id: impl Into<String>, volume: f64) -> Self {
        self.sound = Some(SoundTrigger {
            sound_id: sound_id.into(),
            volume,
            delay_ms: 0,
        });
        self
    }

    pub fn with_bubble(mut self, text: impl Into<String>, duration_ms: u64) -> Self {
        self.bubble = Some(BubbleDef {
            text: text.into(),
            kind: BubbleKind::Speech,
            duration_ms,
            typing_animation: true,
        });
        self
    }

    pub fn fire_and_forget(mut self) -> Self {
        self.wait_for_complete = false;
        self
    }

    pub fn with_delay(mut self, ms: u64) -> Self {
        self.delay_ms = ms;
        self
    }
}

impl ActionSequence {
    pub fn once(steps: Vec<SequenceStep>) -> Self {
        Self {
            steps,
            repeat: SequenceRepeat::Once,
            on_complete: Some("idle".into()),
        }
    }

    pub fn looping(steps: Vec<SequenceStep>) -> Self {
        Self {
            steps,
            repeat: SequenceRepeat::Loop,
            on_complete: None,
        }
    }

    pub fn loop_n(steps: Vec<SequenceStep>, n: u32) -> Self {
        Self {
            steps,
            repeat: SequenceRepeat::LoopN(n),
            on_complete: Some("idle".into()),
        }
    }

    pub fn with_completion(mut self, action: impl Into<String>) -> Self {
        self.on_complete = Some(action.into());
        self
    }
}

/// Tracks playback state of a sequence.
pub struct SequenceExecutor {
    pub sequence: Option<ActionSequence>,
    pub current_step: usize,
    pub loop_count: u32,
    pub delay_remaining_ms: u64,
    pub waiting_for_action: bool,
    pub active: bool,
}

impl SequenceExecutor {
    pub fn new() -> Self {
        Self {
            sequence: None,
            current_step: 0,
            loop_count: 0,
            delay_remaining_ms: 0,
            waiting_for_action: false,
            active: false,
        }
    }

    /// Start playing a sequence.
    pub fn play(&mut self, sequence: ActionSequence) {
        self.current_step = 0;
        self.loop_count = 0;
        self.delay_remaining_ms = sequence.steps.first().map(|s| s.delay_ms).unwrap_or(0);
        self.waiting_for_action = false;
        self.active = true;
        self.sequence = Some(sequence);
    }

    /// Stop the current sequence.
    pub fn stop(&mut self) {
        self.sequence = None;
        self.active = false;
        self.current_step = 0;
    }

    /// Notify that the current action has finished.
    /// Returns true if the sequence should advance to the next step.
    pub fn on_action_finished(&mut self) -> bool {
        if self.waiting_for_action {
            self.waiting_for_action = false;
            true
        } else {
            false
        }
    }

    /// Tick the executor. Returns a SequenceCommand if something should happen.
    pub fn tick(&mut self, delta_ms: u64) -> Option<SequenceCommand> {
        let seq = self.sequence.as_ref()?;

        if !self.active {
            return None;
        }

        // Handle delay
        if self.delay_remaining_ms > 0 {
            if delta_ms >= self.delay_remaining_ms {
                self.delay_remaining_ms = 0;
            } else {
                self.delay_remaining_ms -= delta_ms;
                return None;
            }
        }

        // Waiting for current action to complete
        if self.waiting_for_action {
            return None;
        }

        // Get current step
        let step = match seq.steps.get(self.current_step) {
            Some(s) => s,
            None => {
                // Sequence complete
                return self.handle_sequence_end();
            }
        };

        // Execute the step
        let mut cmd = SequenceCommand::default();

        cmd.action = Some(step.action.clone());
        cmd.loops = step.loops;

        if let Some(ref sound) = step.sound {
            cmd.sound = Some(sound.clone());
        }

        if let Some(ref bubble) = step.bubble {
            cmd.bubble = Some(bubble.to_content());
        }

        if step.wait_for_complete {
            self.waiting_for_action = true;
        }

        // Advance to next step
        self.current_step += 1;
        if self.current_step < seq.steps.len() {
            let next_delay = seq.steps[self.current_step].delay_ms;
            self.delay_remaining_ms = next_delay;
        }

        Some(cmd)
    }

    fn handle_sequence_end(&mut self) -> Option<SequenceCommand> {
        let seq = self.sequence.as_ref()?;

        match &seq.repeat {
            SequenceRepeat::Once => {
                self.active = false;
                let complete_action = seq.on_complete.clone();
                self.sequence = None;
                Some(SequenceCommand {
                    action: complete_action,
                    ..Default::default()
                })
            }
            SequenceRepeat::Loop => {
                self.current_step = 0;
                self.loop_count += 1;
                self.delay_remaining_ms = seq.steps.first().map(|s| s.delay_ms).unwrap_or(0);
                self.waiting_for_action = false;
                // Return None so the next tick picks up the first step
                None
            }
            SequenceRepeat::LoopN(n) => {
                self.loop_count += 1;
                if self.loop_count >= *n {
                    self.active = false;
                    let complete_action = seq.on_complete.clone();
                    self.sequence = None;
                    Some(SequenceCommand {
                        action: complete_action,
                        ..Default::default()
                    })
                } else {
                    self.current_step = 0;
                    self.delay_remaining_ms = seq.steps.first().map(|s| s.delay_ms).unwrap_or(0);
                    self.waiting_for_action = false;
                    None
                }
            }
        }
    }
}

impl Default for SequenceExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// A command issued by the sequence executor.
#[derive(Debug, Default)]
pub struct SequenceCommand {
    pub action: Option<String>,
    pub loops: Option<u32>,
    pub sound: Option<SoundTrigger>,
    pub bubble: Option<BubbleContent>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_step_sequence() {
        let seq = ActionSequence::once(vec![SequenceStep::action("waving").with_loops(1)]);
        let mut exec = SequenceExecutor::new();
        exec.play(seq);

        let cmd = exec.tick(0).unwrap();
        assert_eq!(cmd.action.as_deref(), Some("waving"));
        assert_eq!(cmd.loops, Some(1));
    }

    #[test]
    fn test_multi_step_sequence() {
        let seq = ActionSequence::once(vec![
            SequenceStep::action("waving").with_loops(1),
            SequenceStep::action("jumping").with_loops(1),
        ]);
        let mut exec = SequenceExecutor::new();
        exec.play(seq);

        // First step
        let cmd = exec.tick(0).unwrap();
        assert_eq!(cmd.action.as_deref(), Some("waving"));

        // Notify action finished
        assert!(exec.on_action_finished());

        // Second step
        let cmd = exec.tick(0).unwrap();
        assert_eq!(cmd.action.as_deref(), Some("jumping"));
    }

    #[test]
    fn test_sequence_loop() {
        let seq = ActionSequence::looping(vec![
            SequenceStep::action("waving"),
            SequenceStep::action("idle"),
        ]);
        let mut exec = SequenceExecutor::new();
        exec.play(seq);

        // Step 0: waving
        let cmd = exec.tick(0).unwrap();
        assert_eq!(cmd.action.as_deref(), Some("waving"));
        exec.on_action_finished();

        // Step 1: idle
        let cmd = exec.tick(0).unwrap();
        assert_eq!(cmd.action.as_deref(), Some("idle"));
        exec.on_action_finished();

        // Loop re-enters (returns None), then next tick picks up step 0
        exec.tick(0); // loop reset
        let cmd = exec.tick(0).unwrap();
        assert_eq!(cmd.action.as_deref(), Some("waving"));
    }

    #[test]
    fn test_sequence_with_delay() {
        let seq = ActionSequence::once(vec![
            SequenceStep::action("idle"),
            SequenceStep::action("waving").with_delay(500),
        ]);
        let mut exec = SequenceExecutor::new();
        exec.play(seq);

        exec.tick(0); // idle
        exec.on_action_finished();

        // Delay not elapsed
        assert!(exec.tick(200).is_none());

        // Delay elapsed
        let cmd = exec.tick(400).unwrap();
        assert_eq!(cmd.action.as_deref(), Some("waving"));
    }

    #[test]
    fn test_stop_sequence() {
        let seq = ActionSequence::looping(vec![SequenceStep::action("waving")]);
        let mut exec = SequenceExecutor::new();
        exec.play(seq);
        exec.stop();
        assert!(!exec.active);
        assert!(exec.tick(0).is_none());
    }
}
