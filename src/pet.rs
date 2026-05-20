use crate::models::{
    ActionSequence, BehaviorConfig, Facing, FrameLayout, MoodConfig,
    PetEvent, PetMeta, PetState, PetStats,
};
use crate::resource::{ResourceClient, ResourceConfig, ResourceProvider};
use crate::runtime::{start_pet, PetHandle, PetRuntimeConfig};
use crate::sprite::load_spritesheet;
use crate::{ActionRegistry, BubbleContent, EventActionMap};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Runtime};

/// High-level pet API. Wraps resource loading, validation, and runtime into a single call.
///
/// # Quick Start
///
/// ```rust,no_run
/// use tauri_plugin_sprite_pet::Pet;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Simplest: load from default provider (codex-pets.net)
/// let pet = Pet::start("her-os1").await?;
///
/// // Interact
/// pet.say("Hello!");
/// pet.play("waving");
///
/// // Query
/// let state = pet.state().await;
/// println!("{} is {}", state.pet_id, state.action);
///
/// // Shutdown
/// pet.shutdown();
/// # Ok(())
/// # }
/// ```
pub struct Pet {
    handle: Arc<PetHandle>,
    meta: PetMeta,
    config: crate::models::PetConfig,
}

impl Pet {
    /// Start a pet from the default provider (codex-pets.net).
    pub async fn start(pet_id: &str) -> crate::Result<Self> {
        Self::builder(pet_id).start().await
    }

    /// Create a builder for customizing the pet before starting.
    pub fn builder(pet_id: &str) -> PetBuilder {
        PetBuilder {
            pet_id: pet_id.to_string(),
            provider: None,
            local_dir: None,
            display_name: None,
            layout: None,
            action_registry: None,
            behavior_config: None,
            mood_config: None,
            initial_stats: None,
        }
    }

    /// The pet's metadata (name, description, tags, etc.).
    pub fn meta(&self) -> &PetMeta {
        &self.meta
    }

    /// The pet's config (actions, layout, etc.).
    pub fn config(&self) -> &crate::models::PetConfig {
        &self.config
    }

    /// The underlying handle for advanced control.
    pub fn handle(&self) -> &PetHandle {
        &self.handle
    }

    // ─── Interaction ────────────────────────────────────────────

    /// Play an action animation.
    pub fn play(&self, action: &str) {
        self.play_n(action, 1);
    }

    /// Play an action animation with a loop count.
    pub fn play_n(&self, action: &str, loops: u32) {
        let seq = ActionSequence::once(vec![
            crate::models::SequenceStep::action(action).with_loops(loops),
        ]);
        self.handle.play_sequence(seq);
    }

    /// Play a choreographed sequence of actions.
    pub fn play_sequence(&self, sequence: ActionSequence) {
        self.handle.play_sequence(sequence);
    }

    /// Stop the currently playing sequence.
    pub fn stop_sequence(&self) {
        self.handle.stop_sequence();
    }

    /// Show a speech bubble.
    pub fn say(&self, text: &str) {
        self.handle.show_bubble(BubbleContent::speech(text));
    }

    /// Show a thought bubble.
    pub fn think(&self, text: &str) {
        self.handle.show_bubble(BubbleContent::thought(text));
    }

    /// Show a bubble with full control.
    pub fn show_bubble(&self, content: BubbleContent) {
        self.handle.show_bubble(content);
    }

    /// Dismiss the current bubble.
    pub fn dismiss_bubble(&self) {
        self.handle.dismiss_bubble();
    }

    /// Send a user interaction event.
    pub fn send_event(&self, event: PetEvent) {
        self.handle.send_event(event);
    }

    /// Notify the pet of a user interaction (resets idle timer, boosts mood).
    pub fn notify_interaction(&self) {
        self.handle.notify_interaction();
    }

    /// Set the pet's screen position.
    pub fn set_position(&self, x: f64, y: f64) {
        self.handle.set_position(x, y);
    }

    // ─── Query ──────────────────────────────────────────────────

    /// Get the full current pet state.
    pub async fn state(&self) -> PetState {
        self.handle.current_state().await
    }

    /// Get the current position and facing.
    pub async fn position(&self) -> (f64, f64, Facing) {
        let pos = self.handle.get_position().await;
        (pos.x, pos.y, pos.facing)
    }

    /// Get the available actions.
    pub async fn actions(&self) -> Vec<crate::models::ActionDef> {
        self.handle.get_actions().await
    }

    // ─── Configuration ──────────────────────────────────────────

    /// Enable or disable autonomous ambient behavior.
    pub fn set_ambient_enabled(&self, enabled: bool) {
        self.handle.set_ambient_enabled(enabled);
    }

    /// Override the pet's mood stats.
    pub fn set_stats(&self, stats: PetStats) {
        self.handle.set_stats(stats);
    }

    /// Update the behavior engine configuration.
    pub fn set_behavior_config(&self, config: BehaviorConfig) {
        self.handle.set_behavior_config(config);
    }

    /// Update the mood decay configuration.
    pub fn set_mood_config(&self, config: MoodConfig) {
        self.handle.set_mood_config(config);
    }

    /// Customize an event-to-action binding.
    pub fn set_event_binding(&self, event_key: &str, action: &str) {
        self.handle
            .set_event_binding(event_key.to_string(), action.to_string());
    }

    // ─── Persistence ────────────────────────────────────────────

    /// Save the current pet state to disk.
    pub fn save(&self) {
        self.handle.save_state();
    }

    // ─── Lifecycle ──────────────────────────────────────────────

    /// Shut down the pet runtime. Saves state before stopping.
    pub fn shutdown(&self) {
        self.handle.shutdown();
    }

    // ─── Tauri Bridge ───────────────────────────────────────────

    /// Bridge pet commands to Tauri events so the frontend can render.
    ///
    /// This spawns a background task that forwards every [`PetCommand`]
    /// as a `"pet://command"` Tauri event. It also emits a `"pet://loaded"`
    /// event with the pet config so the frontend knows a pet is ready.
    ///
    /// ```rust,no_run
    /// # use tauri::AppHandle;
    /// # use tauri_plugin_sprite_pet::Pet;
    /// # async fn example(app: AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    /// let pet = Pet::start("her-os1").await?;
    /// pet.bridge_to_tauri(&app);
    /// // Frontend now receives render commands automatically
    /// # Ok(())
    /// # }
    /// ```
    pub fn bridge_to_tauri<R: Runtime>(&self, app: &AppHandle<R>) {
        let mut cmd_rx = self.handle.subscribe_commands();
        let app_handle = app.clone();
        let config = self.config.clone();
        tokio::spawn(async move {
            while let Ok(cmd) = cmd_rx.recv().await {
                let _ = app_handle.emit("pet://command", &cmd);
            }
        });
        let _ = app.emit("pet://loaded", &config);
    }
}

/// Builder for customizing a [`Pet`] before starting.
///
/// # From a remote provider
///
/// ```rust,no_run
/// use tauri_plugin_sprite_pet::{Pet, ResourceProvider};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let pet = Pet::builder("endminguga")
///     .provider(ResourceProvider::codexpet_xyz())
///     .start()
///     .await?;
/// # Ok(())
/// # }
/// ```
///
/// # From a local directory
///
/// ```rust,no_run
/// use tauri_plugin_sprite_pet::Pet;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Directory should contain a spritesheet image (any webp/png/jpg)
/// let pet = Pet::builder("my-pet")
///     .local("./assets/my-pet")
///     .display_name("My Pet")
///     .start()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct PetBuilder {
    pet_id: String,
    provider: Option<ResourceProvider>,
    local_dir: Option<PathBuf>,
    display_name: Option<String>,
    layout: Option<FrameLayout>,
    action_registry: Option<ActionRegistry>,
    behavior_config: Option<BehaviorConfig>,
    mood_config: Option<MoodConfig>,
    initial_stats: Option<PetStats>,
}

impl PetBuilder {
    /// Set the resource provider. Default: [`ResourceProvider::codex_pets()`].
    pub fn provider(mut self, provider: ResourceProvider) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Set the API base URL. Auto-detects the provider from the URL.
    ///
    /// For full control, use [`provider()`](Self::provider) instead.
    pub fn api_url(mut self, url: &str) -> Self {
        let p = if url.contains("codexpet.xyz") {
            ResourceProvider::codexpet_xyz()
        } else {
            ResourceProvider::custom(url)
        };
        self.provider = Some(p);
        self
    }

    /// Load pet from a local directory instead of downloading.
    ///
    /// The directory should contain a spritesheet image (webp/png/jpg).
    /// If a `pet.json` config exists in the directory, it will be loaded.
    /// After starting, the config will be generated/saved to the directory.
    ///
    /// When set, the pet starts entirely offline — no network requests are made.
    pub fn local(mut self, dir: impl Into<PathBuf>) -> Self {
        self.local_dir = Some(dir.into());
        self
    }

    /// Set the display name (used when loading from a local file).
    ///
    /// When loading from a remote provider, the name is fetched from the API.
    pub fn display_name(mut self, name: &str) -> Self {
        self.display_name = Some(name.to_string());
        self
    }

    /// Set a custom sprite sheet layout. Default: 8x9 grid, 192x208 cells.
    pub fn layout(mut self, layout: FrameLayout) -> Self {
        self.layout = Some(layout);
        self
    }

    /// Set a custom action registry. Default: standard actions (idle, running, waving, etc.).
    pub fn action_registry(mut self, registry: ActionRegistry) -> Self {
        self.action_registry = Some(registry);
        self
    }

    /// Set the behavior engine config.
    pub fn behavior_config(mut self, config: BehaviorConfig) -> Self {
        self.behavior_config = Some(config);
        self
    }

    /// Set the mood decay config.
    pub fn mood_config(mut self, config: MoodConfig) -> Self {
        self.mood_config = Some(config);
        self
    }

    /// Set initial mood stats.
    pub fn initial_stats(mut self, stats: PetStats) -> Self {
        self.initial_stats = Some(stats);
        self
    }

    /// Start the pet with the configured options.
    pub async fn start(self) -> crate::Result<Pet> {
        let layout = self.layout.unwrap_or_default();

        // ── Resolve spritesheet path and metadata ──────────────────
        let (path, meta, spritesheet_abs_path, client) =
            if let Some(local_dir) = &self.local_dir {
                // Local mode: scan directory for image file
                let spritesheet_path = find_spritesheet_in_dir(local_dir)?;
                let abs = std::fs::canonicalize(&spritesheet_path)?;
                let display_name = self
                    .display_name
                    .clone()
                    .unwrap_or_else(|| self.pet_id.clone());
                let meta = PetMeta {
                    id: self.pet_id.clone(),
                    display_name,
                    ..PetMeta::empty()
                };
                (abs.clone(), meta, abs.to_string_lossy().into_owned(), None)
            } else {
                // Remote mode: fetch from provider
                let provider = self.provider.unwrap_or_else(ResourceProvider::codex_pets);
                let client = ResourceClient::new(ResourceConfig {
                    provider,
                    ..ResourceConfig::default()
                })?;
                let meta = client.get_pet(&self.pet_id).await?;
                let path = client.fetch_spritesheet(&self.pet_id).await?;
                let abs = client.spritesheet_abs_path(&self.pet_id);
                (path, meta, abs, Some(client))
            };

        // ── Load image for validation and frame detection ──────────
        let (img, sheet) = load_spritesheet(&path, layout.clone())?;

        let validation_config = crate::validation::ValidationConfig::default();
        let outcome = crate::validation::validate_spritesheet(&img, &validation_config)?;
        if !outcome.valid {
            return Err(crate::error::Error::Validation(
                outcome.issues.into_iter().map(|i| i.message).collect(),
            ));
        }

        // ── Detect frame counts and build action registry ──────────
        let row_frame_counts = crate::sprite::detect_frame_counts(&img, &layout);
        let action_registry = self
            .action_registry
            .unwrap_or_else(|| ActionRegistry::with_detected_frames(&row_frame_counts));

        // ── Build pet config ───────────────────────────────────────
        let pet_config = crate::models::PetConfig {
            id: meta.id.clone(),
            display_name: meta.display_name.clone(),
            spritesheet_path: spritesheet_abs_path,
            layout,
            actions: action_registry.action_defs(),
        };

        // Persist config
        if let Some(ref c) = client {
            c.save_config(&self.pet_id, &pet_config).await?;
        } else if let Some(local_dir) = &self.local_dir {
            // Save pet.json alongside the spritesheet
            let config_path = local_dir.join("pet.json");
            let json = serde_json::to_string_pretty(&pet_config)?;
            tokio::fs::write(&config_path, json).await?;
        }

        // ── Try to load saved state ────────────────────────────────
        let store = crate::mood::PetStore::default_store();
        let saved = store.load(&self.pet_id).unwrap_or(None);
        let initial_stats = self
            .initial_stats
            .or_else(|| saved.as_ref().map(|s| s.stats.clone()));

        // ── Start runtime ──────────────────────────────────────────
        let runtime_config = PetRuntimeConfig {
            action_registry,
            event_map: EventActionMap::default_map(),
            behavior_config: Some(self.behavior_config.unwrap_or_default()),
            mood_config: Some(self.mood_config.unwrap_or_default()),
            initial_stats,
            sound_registry: None,
        };
        let handle = start_pet(self.pet_id.clone(), sheet, runtime_config);

        Ok(Pet {
            handle: Arc::new(handle),
            meta,
            config: pet_config,
        })
    }
}

/// Scan a directory for the first image file (webp/png/jpg/jpeg/gif/bmp).
fn find_spritesheet_in_dir(dir: &PathBuf) -> crate::Result<PathBuf> {
    let image_extensions = ["webp", "png", "jpg", "jpeg", "gif", "bmp"];
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .collect();

    // Sort for deterministic order
    entries.sort_by_key(|e| e.file_name());

    for entry in &entries {
        if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
            if image_extensions.contains(&ext.to_lowercase().as_str()) {
                return Ok(entry.path());
            }
        }
    }

    Err(crate::error::Error::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("No image file found in {}", dir.display()),
    )))
}
