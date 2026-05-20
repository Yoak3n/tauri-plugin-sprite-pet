use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
pub mod desktop;
#[cfg(mobile)]
mod mobile;

pub mod action;
pub mod audio;
pub mod behavior;
pub mod bubble;
pub mod choreography;
mod commands;
pub mod error;
pub mod event;
mod models;
pub mod mood;
pub mod pet;
pub mod resource;
pub mod runtime;
pub mod sprite;
pub mod validation;

pub use error::{Error, Result};

// Re-export key types for convenience
pub use action::{ActionPlayer, ActionRegistry};
pub use audio::SoundRegistry;
pub use behavior::BehaviorEngine;
pub use bubble::{BubbleContent, BubbleManager, BubblePriority};
pub use choreography::SequenceExecutor;
pub use event::EventActionMap;
pub use mood::{MoodTracker, PetStore};
pub use pet::{Pet, PetBuilder};
pub use resource::{ResourceClient, ResourceConfig, ResourceProvider, ResponseFormat};
pub use runtime::{start_pet, PetHandle, PetRuntimeConfig, SharedPetState};

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
            // Query
            commands::get_state,
            commands::get_pet_meta,
            commands::get_actions,
            commands::get_position,
            commands::list_remote_pets,
            commands::search_remote_pets,
            // Mutation
            commands::delete_saved_state,
            commands::clear_cache,
            commands::set_mood_config,
            commands::set_event_binding,
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
