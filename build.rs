const COMMANDS: &[&str] = &[
    "load_pet",
    "unload_pet",
    "trigger_event",
    "set_position",
    "register_sound",
    "register_sound_bytes",
    "set_tts",
    "say",
    "play_action",
    "show_bubble",
    "dismiss_bubble",
    "play_sequence",
    "stop_sequence",
    "set_behavior_config",
    "set_ambient_enabled",
    "get_stats",
    "set_stats",
    "save_state",
    "load_saved_state",
    "list_downloaded_pets",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
