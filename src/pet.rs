use crate::models::{
    ActionSequence, BehaviorConfig, Facing, FrameLayout, MoodConfig,
    PetEvent, PetMeta, PetState, PetStats,
};
use crate::resource::{ResourceClient, ResourceConfig, ResourceProvider};
use crate::runtime::{start_pet, PetHandle, PetRuntimeConfig};
use crate::sprite::load_spritesheet;
use crate::{ActionRegistry, BubbleContent, EventActionMap};
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
            api_url: None,
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
/// ```rust,no_run
/// use tauri_plugin_sprite_pet::{Pet, FrameLayout};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let pet = Pet::builder("endminguga")
///     .api_url("https://codexpet.xyz")
///     .start()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct PetBuilder {
    pet_id: String,
    api_url: Option<String>,
    layout: Option<FrameLayout>,
    action_registry: Option<ActionRegistry>,
    behavior_config: Option<BehaviorConfig>,
    mood_config: Option<MoodConfig>,
    initial_stats: Option<PetStats>,
}

impl PetBuilder {
    /// Set the API base URL. Default: `https://codex-pets.net`.
    pub fn api_url(mut self, url: &str) -> Self {
        self.api_url = Some(url.to_string());
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
        let api_url = self
            .api_url
            .unwrap_or_else(|| "https://codex-pets.net".to_string());
        let provider = if api_url.contains("codexpet.xyz") {
            ResourceProvider::CodexpetXyz
        } else {
            ResourceProvider::CodexPets
        };

        let resource_config = ResourceConfig {
            api_base_url: api_url,
            provider,
            ..ResourceConfig::default()
        };
        let client = ResourceClient::new(resource_config)?;

        // Fetch metadata
        let meta = client.get_pet(&self.pet_id).await?;

        // Download spritesheet
        let path = client
            .fetch_spritesheet(&self.pet_id, &meta.spritesheet_url)
            .await?;

        // Load image for validation and frame detection
        let layout = self.layout.unwrap_or_default();
        let (img, sheet) = load_spritesheet(&path, layout.clone())?;

        // Validate
        let validation_config = crate::validation::ValidationConfig::default();
        let outcome = crate::validation::validate_spritesheet(&img, &validation_config)?;
        if !outcome.valid {
            return Err(crate::error::Error::Validation(
                outcome.issues.into_iter().map(|i| i.message).collect(),
            ));
        }

        // Detect frame counts and build action registry
        let row_frame_counts = crate::sprite::detect_frame_counts(&img, &layout);
        let action_registry = self
            .action_registry
            .unwrap_or_else(|| ActionRegistry::with_detected_frames(&row_frame_counts));

        // Build pet config
        let actions = action_registry.action_defs();
        let pet_config = crate::models::PetConfig {
            id: meta.id.clone(),
            display_name: meta.display_name.clone(),
            spritesheet_path: client.spritesheet_abs_path(&self.pet_id),
            layout,
            actions,
        };
        client.save_config(&self.pet_id, &pet_config).await?;

        // Try to load saved state
        let store = crate::mood::PetStore::default_store();
        let saved = store.load(&self.pet_id).unwrap_or(None);
        let initial_stats = self
            .initial_stats
            .or_else(|| saved.as_ref().map(|s| s.stats.clone()));

        // Build runtime config
        let runtime_config = PetRuntimeConfig {
            action_registry,
            event_map: EventActionMap::default_map(),
            behavior_config: Some(self.behavior_config.unwrap_or_default()),
            mood_config: Some(self.mood_config.unwrap_or_default()),
            initial_stats,
            sound_registry: None,
        };

        // Start runtime
        let handle = start_pet(self.pet_id.clone(), sheet, runtime_config);

        Ok(Pet {
            handle: Arc::new(handle),
            meta,
            config: pet_config,
        })
    }
}
