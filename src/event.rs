use crate::models::PetEvent;
use std::collections::HashMap;

/// Maps events to action names. Configurable per pet.
#[derive(Debug, Clone)]
pub struct EventActionMap {
    mappings: HashMap<String, String>,
}

impl EventActionMap {
    pub fn new() -> Self {
        Self {
            mappings: HashMap::new(),
        }
    }

    pub fn default_map() -> Self {
        let mut map = Self::new();
        map.bind("idle", "idle");
        map.bind("walk", "walk");
        map.bind("drag_start", "drag");
        map.bind("drag_drop", "drop");
        map.bind("click", "click");
        map.bind("double_click", "double_click");
        map.bind("sleep", "sleep");
        map.bind("wake", "wake");
        map
    }

    pub fn bind(&mut self, event_key: &str, action: &str) {
        self.mappings.insert(event_key.to_string(), action.to_string());
    }

    pub fn resolve(&self, event_key: &str) -> Option<&str> {
        self.mappings.get(event_key).map(|s| s.as_str())
    }

    pub fn resolve_event(&self, event: &PetEvent) -> Option<&str> {
        let key = match event {
            PetEvent::Idle => "idle",
            PetEvent::Walk { .. } => "walk",
            PetEvent::DragStart => "drag_start",
            PetEvent::DragMove { .. } => return None,
            PetEvent::DragDrop => "drag_drop",
            PetEvent::Click => "click",
            PetEvent::DoubleClick => "double_click",
            PetEvent::Sleep => "sleep",
            PetEvent::Wake => "wake",
            PetEvent::Custom(s) => s.as_str(),
        };
        self.resolve(key)
    }
}

impl Default for EventActionMap {
    fn default() -> Self {
        Self::default_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Facing;

    #[test]
    fn test_default_mapping() {
        let map = EventActionMap::default_map();
        assert_eq!(map.resolve_event(&PetEvent::Idle), Some("idle"));
        assert_eq!(map.resolve_event(&PetEvent::Click), Some("click"));
        assert_eq!(
            map.resolve_event(&PetEvent::Walk {
                direction: Facing::Left
            }),
            Some("walk")
        );
    }

    #[test]
    fn test_custom_mapping() {
        let mut map = EventActionMap::new();
        map.bind("click", "special");
        assert_eq!(map.resolve("click"), Some("special"));
    }
}
