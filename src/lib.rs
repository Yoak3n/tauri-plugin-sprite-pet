use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod action;
mod audio;
mod behavior;
mod bubble;
mod choreography;
mod commands;
mod error;
mod event;
mod models;
mod mood;
mod resource;
mod runtime;
mod sprite;
mod validation;

pub use error::{Error, Result};

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("sprite-pet")
        .invoke_handler(tauri::generate_handler![
            // Lifecycle
            commands::load_pet,
            commands::unload_pet,
            // User interaction
            commands::trigger_event,
            commands::set_position,
            // Sound & TTS
            commands::register_sound,
            commands::register_sound_bytes,
            commands::set_tts,
            commands::say,
            // Direct control
            commands::play_action,
            commands::show_bubble,
            commands::dismiss_bubble,
            commands::play_sequence,
            commands::stop_sequence,
            // Behavior
            commands::set_behavior_config,
            commands::set_ambient_enabled,
            // Mood & persistence
            commands::get_stats,
            commands::set_stats,
            commands::save_state,
            commands::load_saved_state,
            commands::list_downloaded_pets,
        ])
        .setup(|app, api| {
            #[cfg(desktop)]
            {
                let state = desktop::init(app, api)?;
                app.manage(state);
            }
            #[cfg(mobile)]
            {
                let _ = mobile::init(app, api)?;
            }
            Ok(())
        })
        .build()
}
