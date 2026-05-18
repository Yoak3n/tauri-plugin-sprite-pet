use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Manager, Runtime};

use crate::audio::SoundRegistry;
use crate::models::PetMeta;
use crate::resource::{ResourceClient, ResourceConfig};
use crate::runtime::PetHandle;
use tokio::sync::RwLock;

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<PetPluginState> {
    // Use the app's local data directory for caching pet resources
    let cache_dir = app
        .path()
        .app_local_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("sprite-pet");

    let config = ResourceConfig { cache_dir, ..ResourceConfig::default() };
    let resource = ResourceClient::new(config)?;
    Ok(PetPluginState {
        resource,
        handle: RwLock::new(None),
        current_pet: RwLock::new(None),
        sound_registry: RwLock::new(SoundRegistry::new()),
    })
}

/// Plugin state stored in Tauri's managed state.
pub struct PetPluginState {
    pub resource: ResourceClient,
    pub handle: RwLock<Option<PetHandle>>,
    pub current_pet: RwLock<Option<PetMeta>>,
    pub sound_registry: RwLock<SoundRegistry>,
}
