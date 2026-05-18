use crate::models::BubbleKind;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BubblePriority {
    Low,
    Normal,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BubbleContent {
    pub text: String,
    pub kind: BubbleKind,
    pub duration_ms: u64,
    pub typing_animation: bool,
    pub priority: BubblePriority,
}

impl BubbleContent {
    pub fn speech(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: BubbleKind::Speech,
            duration_ms: 3000,
            typing_animation: true,
            priority: BubblePriority::Normal,
        }
    }

    pub fn thought(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: BubbleKind::Thought,
            duration_ms: 4000,
            typing_animation: false,
            priority: BubblePriority::Normal,
        }
    }

    pub fn action(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: BubbleKind::Action,
            duration_ms: 2000,
            typing_animation: false,
            priority: BubblePriority::Low,
        }
    }

    pub fn system(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: BubbleKind::System,
            duration_ms: 5000,
            typing_animation: false,
            priority: BubblePriority::High,
        }
    }

    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    pub fn with_priority(mut self, p: BubblePriority) -> Self {
        self.priority = p;
        self
    }

    pub fn with_typing(mut self, typing: bool) -> Self {
        self.typing_animation = typing;
        self
    }
}

pub struct BubbleManager {
    pub current: Option<BubbleState>,
    queue: VecDeque<BubbleContent>,
    max_queue_size: usize,
}

pub struct BubbleState {
    pub content: BubbleContent,
    pub shown_at: Instant,
}

impl BubbleManager {
    pub fn new() -> Self {
        Self {
            current: None,
            queue: VecDeque::new(),
            max_queue_size: 10,
        }
    }

    pub fn show(&mut self, content: BubbleContent) {
        match content.priority {
            BubblePriority::High => {
                self.current = Some(BubbleState {
                    content,
                    shown_at: Instant::now(),
                });
            }
            BubblePriority::Normal => {
                if self.current.is_none() {
                    self.current = Some(BubbleState {
                        content,
                        shown_at: Instant::now(),
                    });
                } else if self.queue.len() < self.max_queue_size {
                    self.queue.push_back(content);
                }
            }
            BubblePriority::Low => {
                if self.current.is_none() {
                    self.current = Some(BubbleState {
                        content,
                        shown_at: Instant::now(),
                    });
                }
            }
        }
    }

    pub fn dismiss(&mut self) {
        self.current = None;
    }

    pub fn tick(&mut self) -> Option<&BubbleContent> {
        if let Some(ref state) = self.current {
            if state.content.duration_ms > 0 {
                let elapsed = state.shown_at.elapsed().as_millis() as u64;
                if elapsed >= state.content.duration_ms {
                    self.current = None;
                }
            }
        }

        if self.current.is_none() {
            if let Some(next) = self.queue.pop_front() {
                self.current = Some(BubbleState {
                    content: next,
                    shown_at: Instant::now(),
                });
            }
        }

        self.current.as_ref().map(|s| &s.content)
    }

    pub fn is_showing(&self) -> bool {
        self.current.is_some()
    }
}

impl Default for BubbleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bubble_priority_interrupt() {
        let mut mgr = BubbleManager::new();
        mgr.show(BubbleContent::speech("hello"));
        assert!(mgr.is_showing());
        mgr.show(BubbleContent::system("interrupted!"));
        assert_eq!(mgr.current.as_ref().unwrap().content.text, "interrupted!");
    }

    #[test]
    fn test_bubble_normal_queues() {
        let mut mgr = BubbleManager::new();
        mgr.show(BubbleContent::speech("first"));
        mgr.show(BubbleContent::speech("second"));
        assert_eq!(mgr.current.as_ref().unwrap().content.text, "first");
        assert_eq!(mgr.queue.len(), 1);
    }

    #[test]
    fn test_bubble_low_drops() {
        let mut mgr = BubbleManager::new();
        mgr.show(BubbleContent::speech("first"));
        mgr.show(BubbleContent::action("dropped"));
        assert_eq!(mgr.current.as_ref().unwrap().content.text, "first");
        assert_eq!(mgr.queue.len(), 0);
    }

    #[test]
    fn test_bubble_dismiss() {
        let mut mgr = BubbleManager::new();
        mgr.show(BubbleContent::speech("hello"));
        mgr.dismiss();
        assert!(!mgr.is_showing());
    }

    #[test]
    fn test_builder_methods() {
        let b = BubbleContent::speech("hi")
            .with_duration(5000)
            .with_priority(BubblePriority::High)
            .with_typing(false);
        assert_eq!(b.duration_ms, 5000);
        assert_eq!(b.priority, BubblePriority::High);
        assert!(!b.typing_animation);
    }
}
