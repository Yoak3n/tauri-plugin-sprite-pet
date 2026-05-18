use crate::action::{ActionPlayer, ActionRegistry};
use crate::audio::SoundRegistry;
use crate::behavior::{BehaviorEngine, BehaviorTick};
use crate::bubble::{BubbleContent, BubbleManager};
use crate::choreography::{SequenceCommand, SequenceExecutor};
use crate::event::EventActionMap;
use crate::models::{
    BubbleSnapshot, Facing, FrameRect, MoodConfig, PetCommand, PetEvent, PetSnapshot, PetStats,
    PetState, Position, SpriteSheet,
};
use crate::mood::{MoodTracker, PetStore};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, watch, RwLock};

/// Shared pet state readable from any thread.
pub type SharedPetState = Arc<RwLock<PetState>>;

/// Configuration for creating a pet runtime.
pub struct PetRuntimeConfig {
    pub action_registry: ActionRegistry,
    pub event_map: EventActionMap,
    pub behavior_config: Option<crate::models::BehaviorConfig>,
    pub mood_config: Option<MoodConfig>,
    pub initial_stats: Option<PetStats>,
    pub sound_registry: Option<SoundRegistry>,
}

impl Default for PetRuntimeConfig {
    fn default() -> Self {
        Self {
            action_registry: ActionRegistry::default_registry(),
            event_map: EventActionMap::default_map(),
            behavior_config: None,
            mood_config: None,
            initial_stats: None,
            sound_registry: None,
        }
    }
}

/// Handle for sending events to a running pet. Can be cloned and shared.
#[derive(Clone)]
pub struct PetHandle {
    event_tx: mpsc::UnboundedSender<PetEvent>,
    bubble_tx: mpsc::UnboundedSender<BubbleContent>,
    seq_tx: mpsc::UnboundedSender<crate::models::ActionSequence>,
    cmd_tx: mpsc::UnboundedSender<HandleCommand>,
    state: SharedPetState,
    state_tx: broadcast::Sender<PetState>,
    command_tx: broadcast::Sender<PetCommand>,
    shutdown_tx: watch::Sender<bool>,
}

/// Internal commands from PetHandle to run_loop.
enum HandleCommand {
    SetPosition(f64, f64),
    DismissBubble,
    StopSequence,
    SetBehaviorEnabled(bool),
    SetBehaviorConfig(crate::models::BehaviorConfig),
    SetStats(PetStats),
    Interaction,
    SaveState,
}

impl PetHandle {
    /// Signal the runtime to stop.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    /// Send an event to the pet.
    pub fn send_event(&self, event: PetEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Show a dialogue bubble.
    pub fn show_bubble(&self, content: BubbleContent) {
        let _ = self.bubble_tx.send(content);
    }

    /// Dismiss the current bubble.
    pub fn dismiss_bubble(&self) {
        let _ = self.cmd_tx.send(HandleCommand::DismissBubble);
    }

    /// Play an action sequence.
    pub fn play_sequence(&self, seq: crate::models::ActionSequence) {
        let _ = self.seq_tx.send(seq);
    }

    /// Stop the current sequence.
    pub fn stop_sequence(&self) {
        let _ = self.cmd_tx.send(HandleCommand::StopSequence);
    }

    /// Enable/disable autonomous ambient behavior.
    pub fn set_ambient_enabled(&self, enabled: bool) {
        let _ = self.cmd_tx.send(HandleCommand::SetBehaviorEnabled(enabled));
    }

    /// Replace the behavior config at runtime.
    pub fn set_behavior_config(&self, config: crate::models::BehaviorConfig) {
        let _ = self.cmd_tx.send(HandleCommand::SetBehaviorConfig(config));
    }

    /// Override pet stats at runtime.
    pub fn set_stats(&self, stats: PetStats) {
        let _ = self.cmd_tx.send(HandleCommand::SetStats(stats));
    }

    /// Notify the runtime of a user interaction (resets idle timer, boosts mood).
    pub fn notify_interaction(&self) {
        let _ = self.cmd_tx.send(HandleCommand::Interaction);
    }

    /// Request a state save to disk.
    pub fn save_state(&self) {
        let _ = self.cmd_tx.send(HandleCommand::SaveState);
    }

    /// Get a snapshot of the current pet state.
    pub async fn current_state(&self) -> PetState {
        self.state.read().await.clone()
    }

    /// Subscribe to internal state changes (for persistence/debugging).
    pub fn subscribe(&self) -> broadcast::Receiver<PetState> {
        self.state_tx.subscribe()
    }

    /// Subscribe to PetCommand stream. The frontend listens to this.
    pub fn subscribe_commands(&self) -> broadcast::Receiver<PetCommand> {
        self.command_tx.subscribe()
    }

    /// Update the pet's position.
    pub fn set_position(&self, x: f64, y: f64) {
        let _ = self.cmd_tx.send(HandleCommand::SetPosition(x, y));
    }
}

/// Create and start a pet runtime. Returns a handle for interacting with it.
pub fn start_pet(
    pet_id: String,
    sprite_sheet: SpriteSheet,
    config: PetRuntimeConfig,
) -> PetHandle {
    let (state_tx, _state_rx) = broadcast::channel(64);
    let (command_tx, _cmd_rx) = broadcast::channel::<PetCommand>(256);
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (bubble_tx, bubble_rx) = mpsc::unbounded_channel();
    let (seq_tx, seq_rx) = mpsc::unbounded_channel();
    let (internal_cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let initial_action = "idle";
    let frame_rect = sprite_sheet.frames[0][0];
    let stats = config.initial_stats.unwrap_or_default();
    let mood_label = stats.mood_label().to_string();

    let initial_state = PetState {
        pet_id: pet_id.clone(),
        action: initial_action.to_string(),
        frame_index: 0,
        frame_rect,
        position: Position { x: 0.0, y: 0.0 },
        facing: Facing::Right,
        bubble: None,
        stats,
        mood_label,
    };

    let state = Arc::new(RwLock::new(initial_state.clone()));
    let _ = state_tx.send(initial_state);

    let behavior_engine = config
        .behavior_config
        .map(BehaviorEngine::new)
        .unwrap_or_else(BehaviorEngine::with_defaults);

    let mood_tracker = config
        .mood_config
        .map(|c| MoodTracker::new(PetStats::default(), c))
        .unwrap_or_else(MoodTracker::with_defaults);

    let sound_registry = config.sound_registry.unwrap_or_default();

    let handle = PetHandle {
        event_tx,
        bubble_tx,
        seq_tx,
        cmd_tx: internal_cmd_tx,
        state: state.clone(),
        state_tx: state_tx.clone(),
        command_tx: command_tx.clone(),
        shutdown_tx,
    };

    let pet_store = PetStore::default_store();

    tokio::spawn(run_loop(
        pet_id,
        Arc::new(sprite_sheet),
        config.action_registry,
        config.event_map,
        ActionPlayer::new(initial_action),
        state,
        state_tx,
        command_tx,
        event_rx,
        bubble_rx,
        seq_rx,
        cmd_rx,
        shutdown_rx,
        Position { x: 0.0, y: 0.0 },
        Facing::Right,
        behavior_engine,
        mood_tracker,
        BubbleManager::new(),
        SequenceExecutor::new(),
        pet_store,
        sound_registry,
    ));

    handle
}

/// The main animation/event/behavior loop.
#[allow(clippy::too_many_arguments)]
async fn run_loop(
    pet_id: String,
    sprite_sheet: Arc<SpriteSheet>,
    action_registry: ActionRegistry,
    event_map: EventActionMap,
    mut player: ActionPlayer,
    state: SharedPetState,
    state_tx: broadcast::Sender<PetState>,
    cmd_tx: broadcast::Sender<PetCommand>,
    mut event_rx: mpsc::UnboundedReceiver<PetEvent>,
    mut bubble_rx: mpsc::UnboundedReceiver<BubbleContent>,
    mut seq_rx: mpsc::UnboundedReceiver<crate::models::ActionSequence>,
    mut internal_cmd_rx: mpsc::UnboundedReceiver<HandleCommand>,
    mut shutdown_rx: watch::Receiver<bool>,
    mut position: Position,
    mut facing: Facing,
    mut behavior: BehaviorEngine,
    mut mood: MoodTracker,
    mut bubble_mgr: BubbleManager,
    mut seq_exec: SequenceExecutor,
    pet_store: PetStore,
    sound_registry: SoundRegistry,
) {
    let mut last_tick = tokio::time::Instant::now();
    let mut last_save = tokio::time::Instant::now();
    let save_interval = tokio::time::Duration::from_secs(60);
    let sound_reg = Arc::new(sound_registry);
    let mut pending_idle_switch: bool = false;
    let mut last_frame_hold_remaining_ms: u64 = 0;

    loop {
        let frame_duration = action_registry
            .get(&player.current_action)
            .map(|a| a.frame_duration_ms)
            .unwrap_or(100);

        let tick_interval = tokio::time::Duration::from_millis(frame_duration.max(16));

        tokio::select! {
            // Shutdown signal
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    // Save state before exiting
                    save_pet_state(&pet_id, &state, &pet_store).await;
                    break;
                }
            }

            // User events
            Some(event) = event_rx.recv() => {
                handle_event(
                    &event, &event_map, &action_registry, &mut player,
                    &mut position, &mut facing, &mut behavior, &mood,
                    &state, &state_tx, &cmd_tx, &pet_id, &sprite_sheet,
                    &mut last_frame_hold_remaining_ms, &mut pending_idle_switch,
                ).await;
            }

            // Bubble commands
            Some(bubble) = bubble_rx.recv() => {
                // Emit bubble command to frontend
                let _ = cmd_tx.send(PetCommand::Bubble {
                    text: bubble.text.clone(),
                    kind: format!("{:?}", bubble.kind).to_lowercase(),
                    duration_ms: bubble.duration_ms,
                });

                // TTS: synthesize speech for speech bubbles
                if bubble.kind == crate::models::BubbleKind::Speech {
                    let text = bubble.text.clone();
                    let reg = sound_reg.clone();
                    let tts_cmd_tx = cmd_tx.clone();
                    tokio::spawn(async move {
                        if let Ok(Some((audio_bytes, format))) = reg.speak(&text).await {
                            if !audio_bytes.is_empty() {
                                let _ = tts_cmd_tx.send(PetCommand::Audio {
                                    audio_bytes,
                                    format,
                                    volume: 1.0,
                                });
                            }
                        }
                    });
                }

                bubble_mgr.show(bubble);
            }

            // Sequence commands
            Some(seq) = seq_rx.recv() => {
                last_frame_hold_remaining_ms = 0;
                pending_idle_switch = false;
                seq_exec.play(seq);
            }

            // Handle commands
            Some(cmd) = internal_cmd_rx.recv() => {
                match cmd {
                    HandleCommand::SetPosition(x, y) => {
                        position = Position { x, y };
                    }
                    HandleCommand::DismissBubble => {
                        bubble_mgr.dismiss();
                        let _ = cmd_tx.send(PetCommand::DismissBubble);
                    }
                    HandleCommand::StopSequence => {
                        seq_exec.stop();
                    }
                    HandleCommand::SetBehaviorEnabled(enabled) => {
                        behavior.ambient_enabled = enabled;
                    }
                    HandleCommand::SetBehaviorConfig(config) => {
                        behavior.config = config;
                    }
                    HandleCommand::SetStats(new_stats) => {
                        mood.stats = new_stats;
                    }
                    HandleCommand::Interaction => {
                        behavior.on_interaction();
                        mood.on_interaction();
                    }
                    HandleCommand::SaveState => {
                        save_pet_state(&pet_id, &state, &pet_store).await;
                    }
                }
            }

            // Tick
            _ = tokio::time::sleep(tick_interval) => {
                let now = tokio::time::Instant::now();
                let delta = now - last_tick;
                let delta_ms = delta.as_millis() as u64;
                last_tick = now;

                let prev_action = player.current_action.clone();

                // Track hold state - skip animation advance but still process events/behavior/sequences
                let in_hold = last_frame_hold_remaining_ms > 0;
                if in_hold {
                    let consumed = delta_ms.min(last_frame_hold_remaining_ms);
                    last_frame_hold_remaining_ms -= consumed;
                }

                // 1. Tick animation (skip during hold)
                let frame_changed = if in_hold { false } else { player.tick(delta_ms, &action_registry) };

                // 2. Tick mood
                mood.set_idle(behavior.in_ambient_mode);
                mood.tick();

                // 3. Tick behavior engine (autonomous actions) - still runs during hold
                if let Some(BehaviorTick { action, bubble }) = behavior.tick(&mood.stats) {
                    if player.switch_to(&action, &action_registry) {
                        last_frame_hold_remaining_ms = 0;
                        pending_idle_switch = false;
                        if let Some(b) = bubble {
                            let _ = cmd_tx.send(PetCommand::Bubble {
                                text: b.text.clone(),
                                kind: format!("{:?}", b.kind).to_lowercase(),
                                duration_ms: b.duration_ms,
                            });
                            bubble_mgr.show(b);
                        }
                    }
                }

                // 4. Tick sequence executor - still runs during hold
                if let Some(seq_cmd) = seq_exec.tick(delta_ms) {
                    last_frame_hold_remaining_ms = 0;
                    pending_idle_switch = false;
                    apply_sequence_command(
                        seq_cmd, &action_registry, &mut player,
                        &mut bubble_mgr, &state, &state_tx, &cmd_tx, &pet_id, &sprite_sheet,
                        position, facing, &mood,
                    ).await;
                }

                // 5. Tick bubble manager
                let bubble_snapshot = bubble_mgr.tick().map(|b| BubbleSnapshot {
                    text: b.text.clone(),
                    kind: b.kind,
                    typing_animation: b.typing_animation,
                });

                // 6. Notify sequence if action finished
                if player.finished && seq_exec.active {
                    if seq_exec.on_action_finished() {
                        // Sequence will advance on next tick
                    }
                }

                // 7. Handle non-looping action finish with hold, or pending idle switch
                if !in_hold {
                    if player.finished && !seq_exec.active {
                        // Emit ActionFinished notification to frontend
                        let _ = cmd_tx.send(PetCommand::ActionFinished {
                            action: player.current_action.clone(),
                        });

                        // Start hold: use action's configured hold or default 200ms
                        let hold_ms = action_registry
                            .get(&player.current_action)
                            .and_then(|a| a.last_frame_hold_ms)
                            .unwrap_or(200);

                        if hold_ms > 0 {
                            last_frame_hold_remaining_ms = hold_ms;
                            player.finished = false;
                        } else {
                            pending_idle_switch = true;
                        }
                    }

                    if pending_idle_switch {
                        pending_idle_switch = false;
                        player.switch_to("idle", &action_registry);
                    }
                } else if last_frame_hold_remaining_ms == 0 {
                    // Hold just expired
                    if !seq_exec.active {
                        pending_idle_switch = true;
                    }
                }

                // 8. Emit commands to frontend

                // Action changed → emit sound if registered
                if player.current_action != prev_action {
                    if let Some(sound) = sound_reg.get(&player.current_action) {
                        let _ = cmd_tx.send(PetCommand::Audio {
                            audio_bytes: sound.data.clone(),
                            format: sound.format,
                            volume: sound.volume,
                        });
                    }
                }

                // Emit render: during hold re-send current frame; otherwise on change
                if in_hold {
                    let _ = cmd_tx.send(PetCommand::Render {
                        action: player.current_action.clone(),
                        frame_index: player.current_frame,
                        facing: format!("{:?}", facing).to_lowercase(),
                        x: position.x,
                        y: position.y,
                        scale: 1.0,
                    });
                } else if frame_changed || player.current_action != prev_action {
                    let _ = cmd_tx.send(PetCommand::Render {
                        action: player.current_action.clone(),
                        frame_index: player.current_frame,
                        facing: format!("{:?}", facing).to_lowercase(),
                        x: position.x,
                        y: position.y,
                        scale: 1.0,
                    });
                }

                // 9. Update internal state
                let action = action_registry.get(&player.current_action);
                let row = action.map(|a| a.row).unwrap_or(0);
                let frame_rect = sprite_sheet.frames
                    .get(row as usize)
                    .and_then(|r| r.get(player.current_frame as usize))
                    .copied()
                    .unwrap_or(FrameRect { x: 0, y: 0, width: 0, height: 0 });

                let new_state = PetState {
                    pet_id: pet_id.clone(),
                    action: player.current_action.clone(),
                    frame_index: player.current_frame,
                    frame_rect,
                    position,
                    facing,
                    bubble: bubble_snapshot,
                    stats: mood.stats.clone(),
                    mood_label: mood.stats.mood_label().to_string(),
                };

                *state.write().await = new_state.clone();
                let _ = state_tx.send(new_state);

                // 10. Periodic auto-save
                if last_save.elapsed() >= save_interval {
                    save_pet_state(&pet_id, &state, &pet_store).await;
                    last_save = now;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_event(
    event: &PetEvent,
    event_map: &EventActionMap,
    action_registry: &ActionRegistry,
    player: &mut ActionPlayer,
    position: &mut Position,
    facing: &mut Facing,
    _behavior: &mut BehaviorEngine,
    mood: &MoodTracker,
    state: &SharedPetState,
    state_tx: &broadcast::Sender<PetState>,
    cmd_tx: &broadcast::Sender<PetCommand>,
    pet_id: &str,
    sprite_sheet: &SpriteSheet,
    last_frame_hold_remaining_ms: &mut u64,
    pending_idle_switch: &mut bool,
) {
    // Handle position/facing updates
    match event {
        PetEvent::DragMove { x, y } => {
            *position = Position { x: *x, y: *y };
            return; // DragMove doesn't trigger action change
        }
        PetEvent::Walk { direction } => {
            *facing = *direction;
        }
        _ => {}
    }

    // Resolve event to action
    if let Some(action_name) = event_map.resolve_event(event) {
        if player.switch_to(action_name, action_registry) {
            // Reset hold timers since the action changed
            *last_frame_hold_remaining_ms = 0;
            *pending_idle_switch = false;
            // Emit lightweight render for the new action
            let _ = cmd_tx.send(PetCommand::Render {
                action: player.current_action.clone(),
                frame_index: 0,
                facing: format!("{:?}", facing).to_lowercase(),
                x: position.x,
                y: position.y,
                scale: 1.0,
            });

            // Update internal state
            let action = action_registry.get(&player.current_action);
            let row = action.map(|a| a.row).unwrap_or(0);
            let frame_rect = sprite_sheet.frames
                .get(row as usize)
                .and_then(|r| r.get(0))
                .copied()
                .unwrap_or(FrameRect { x: 0, y: 0, width: 0, height: 0 });

            let new_state = PetState {
                pet_id: pet_id.to_string(),
                action: player.current_action.clone(),
                frame_index: 0,
                frame_rect,
                position: *position,
                facing: *facing,
                bubble: None,
                stats: mood.stats.clone(),
                mood_label: mood.stats.mood_label().to_string(),
            };
            *state.write().await = new_state.clone();
            let _ = state_tx.send(new_state);
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn apply_sequence_command(
    cmd: SequenceCommand,
    action_registry: &ActionRegistry,
    player: &mut ActionPlayer,
    bubble_mgr: &mut BubbleManager,
    state: &SharedPetState,
    state_tx: &broadcast::Sender<PetState>,
    cmd_tx: &broadcast::Sender<PetCommand>,
    pet_id: &str,
    sprite_sheet: &SpriteSheet,
    position: Position,
    facing: Facing,
    mood: &MoodTracker,
) {
    if let Some(ref action_name) = cmd.action {
        player.switch_to(action_name, action_registry);
    }
    if let Some(bubble) = cmd.bubble {
        let _ = cmd_tx.send(PetCommand::Bubble {
            text: bubble.text.clone(),
            kind: format!("{:?}", bubble.kind).to_lowercase(),
            duration_ms: bubble.duration_ms,
        });
        bubble_mgr.show(bubble);
    }
    // Sound triggers from sequence
    if let Some(ref sound) = cmd.sound {
        // Frontend can map sound_id to audio — for now emit as a placeholder
        tracing::info!("Sound trigger: {} (volume: {})", sound.sound_id, sound.volume);
    }

    // Emit lightweight render for the new action
    let _ = cmd_tx.send(PetCommand::Render {
        action: player.current_action.clone(),
        frame_index: 0,
        facing: format!("{:?}", facing).to_lowercase(),
        x: position.x,
        y: position.y,
        scale: 1.0,
    });

    let action = action_registry.get(&player.current_action);
    let row = action.map(|a| a.row).unwrap_or(0);
    let frame_rect = sprite_sheet.frames
        .get(row as usize)
        .and_then(|r| r.get(0))
        .copied()
        .unwrap_or(FrameRect { x: 0, y: 0, width: 0, height: 0 });

    let new_state = PetState {
        pet_id: pet_id.to_string(),
        action: player.current_action.clone(),
        frame_index: 0,
        frame_rect,
        position,
        facing,
        bubble: None,
        stats: mood.stats.clone(),
        mood_label: mood.stats.mood_label().to_string(),
    };
    *state.write().await = new_state.clone();
    let _ = state_tx.send(new_state);
}

async fn save_pet_state(pet_id: &str, state: &SharedPetState, store: &PetStore) {
    let s = state.read().await;
    let snapshot = PetSnapshot {
        pet_id: pet_id.to_string(),
        position: s.position,
        facing: s.facing,
        stats: s.stats.clone(),
        current_action: s.action.clone(),
        saved_at: chrono::Utc::now().to_rfc3339(),
    };
    if let Err(e) = store.save(&snapshot) {
        tracing::warn!("Failed to save pet state: {}", e);
    }
}
