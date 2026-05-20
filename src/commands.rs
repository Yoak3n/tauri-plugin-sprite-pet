use crate::audio::{AzureTts, ElevenLabsTts};
use crate::bubble::{BubbleContent, BubblePriority};
use crate::desktop::PetPluginState;
use crate::models::{
    ActionDef, ActionSequence, AudioFormat, BehaviorConfig, BubbleKind, LoadPetResult,
    MoodConfig, PetConfig, PetEvent, PetListResponse, PetMeta, PetSnapshot, PetStats,
    PetState, PositionInfo, SequenceStep,
};
use crate::resource::{ResourceClient, ResourceConfig, ResourceProvider};
use crate::runtime::{start_pet, PetRuntimeConfig};
use tauri::{command, AppHandle, Emitter, Manager, Runtime};

/// Detect the resource provider from a URL.
fn detect_provider(url: &str) -> ResourceProvider {
    if url.contains("codexpet.xyz") {
        ResourceProvider::codexpet_xyz()
    } else {
        ResourceProvider::custom(url)
    }
}

// ─── Lifecycle ───────────────────────────────────────────────────────

#[command]
pub(crate) async fn load_pet<R: Runtime>(
    app: AppHandle<R>,
    pet_id: String,
    api_base_url: Option<String>,
) -> crate::Result<LoadPetResult> {
    let state = app.state::<PetPluginState>();

    // Shut down current pet runtime before loading new one
    {
        let mut handle_guard = state.handle.write().await;
        if let Some(ref handle) = *handle_guard {
            handle.shutdown();
        }
        *handle_guard = None;
    }

    // If a custom API URL is provided, create a temporary client for this request
    let resource = if let Some(ref url) = api_base_url {
        let provider = detect_provider(url);
        let config = ResourceConfig {
            provider,
            cache_dir: state.resource.cache_dir(),
            ..ResourceConfig::default()
        };
        ResourceClient::new(config)?
    } else {
        ResourceClient::new(state.resource.config().clone())?
    };

    // ── Try cache-first ───────────────────────────────────────────
    let cached_config_path = resource.cache_dir().join(&pet_id).join("sprite-pet.json");
    if let Ok(json) = tokio::fs::read_to_string(&cached_config_path).await {
        if let Ok(cached) = serde_json::from_str::<PetConfig>(&json) {
            let spritesheet = std::path::PathBuf::from(&cached.spritesheet_path);
            if spritesheet.exists() {
                if let Ok(hash) = crate::resource::file_crc32(&spritesheet) {
                    if hash == cached.spritesheet_hash {
                        // Cache hit — skip API, validation, frame detection
                        let spritesheet_bytes = tokio::fs::read(&spritesheet).await?;
                        let layout = cached.layout.clone();
                        let action_registry =
                            crate::action::ActionRegistry::new(cached.actions.clone());
                        let (_, sheet) =
                            crate::sprite::load_spritesheet(&spritesheet, layout)?;

                        let store = crate::mood::PetStore::default_store();
                        let saved = store.load(&pet_id).unwrap_or(None);
                        let initial_stats = saved.as_ref().map(|s| s.stats.clone());

                        let sound_registry = {
                            let mut reg = state.sound_registry.write().await;
                            std::mem::take(&mut *reg)
                        };

                        let runtime_config = PetRuntimeConfig {
                            action_registry,
                            event_map: crate::event::EventActionMap::default_map(),
                            behavior_config: Some(BehaviorConfig::default()),
                            mood_config: Some(crate::models::MoodConfig::default()),
                            initial_stats,
                            sound_registry: Some(sound_registry),
                        };

                        let handle = start_pet(pet_id.clone(), sheet, runtime_config);

                        let mut command_rx = handle.subscribe_commands();
                        let app_handle = app.clone();
                        tokio::spawn(async move {
                            while let Ok(cmd) = command_rx.recv().await {
                                let _ = app_handle.emit("pet://command", &cmd);
                            }
                        });

                        *state.handle.write().await = Some(handle);

                        let _ = app.emit("pet://loaded", &cached);

                        return Ok(LoadPetResult {
                            config: cached,
                            spritesheet_bytes,
                        });
                    }
                }
                // Hash mismatch — delete corrupted file
                let _ = tokio::fs::remove_file(&spritesheet).await;
            }
        }
    }

    // ── Full flow ─────────────────────────────────────────────────
    let pet_meta = resource.get_pet(&pet_id).await?;
    let path = resource.fetch_spritesheet(&pet_id).await?;

    let layout = crate::models::FrameLayout::default();
    let (img, sheet) = crate::sprite::load_spritesheet(&path, layout.clone())?;

    let validation_config = crate::validation::ValidationConfig::default();
    let outcome = crate::validation::validate_spritesheet(&img, &validation_config)?;

    if !outcome.valid {
        return Err(crate::error::Error::Validation(
            outcome.issues.into_iter().map(|i| i.message).collect(),
        ));
    }

    let row_frame_counts = crate::sprite::detect_frame_counts(&img, &layout);
    let action_registry = crate::action::ActionRegistry::with_detected_frames(&row_frame_counts);

    let spritesheet_abs = resource.spritesheet_abs_path(&pet_id);
    let spritesheet_bytes = tokio::fs::read(&path).await?;
    let spritesheet_hash = crate::resource::file_crc32(&path)?;
    let actions = action_registry.action_defs();
    let pet_config = PetConfig {
        id: pet_meta.id.clone(),
        display_name: pet_meta.display_name.clone(),
        spritesheet_path: spritesheet_abs,
        spritesheet_hash,
        layout: layout.clone(),
        actions,
    };
    resource.save_config(&pet_id, &pet_config).await?;

    let store = crate::mood::PetStore::default_store();
    let saved = store.load(&pet_id).unwrap_or(None);
    let initial_stats = saved.as_ref().map(|s| s.stats.clone());

    let sound_registry = {
        let mut reg = state.sound_registry.write().await;
        std::mem::take(&mut *reg)
    };

    let runtime_config = PetRuntimeConfig {
        action_registry,
        event_map: crate::event::EventActionMap::default_map(),
        behavior_config: Some(BehaviorConfig::default()),
        mood_config: Some(crate::models::MoodConfig::default()),
        initial_stats,
        sound_registry: Some(sound_registry),
    };

    let handle = start_pet(pet_id.clone(), sheet, runtime_config);

    let mut command_rx = handle.subscribe_commands();
    let app_handle = app.clone();
    tokio::spawn(async move {
        while let Ok(cmd) = command_rx.recv().await {
            let _ = app_handle.emit("pet://command", &cmd);
        }
    });

    *state.handle.write().await = Some(handle);
    *state.current_pet.write().await = Some(pet_meta);

    let _ = app.emit("pet://loaded", &pet_config);

    Ok(LoadPetResult {
        config: pet_config,
        spritesheet_bytes,
    })
}

#[command]
pub(crate) async fn unload_pet<R: Runtime>(
    app: AppHandle<R>,
) -> crate::Result<()> {
    let state = app.state::<PetPluginState>();

    let pet_id = {
        let mut handle_guard = state.handle.write().await;
        if let Some(handle) = handle_guard.take() {
            handle.shutdown();
            state
                .current_pet
                .read()
                .await
                .as_ref()
                .map(|m| m.id.clone())
                .unwrap_or_default()
        } else {
            return Ok(());
        }
    };

    let _ = state.current_pet.write().await.take();
    let _ = app.emit("pet://unloaded", serde_json::json!({ "petId": pet_id }));

    Ok(())
}

// ─── User Interaction ────────────────────────────────────────────────

#[command]
pub(crate) async fn trigger_event(
    app: AppHandle<impl Runtime>,
    event: PetEvent,
) -> crate::Result<()> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        handle.notify_interaction();
        handle.send_event(event);
        Ok(())
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}

#[command]
pub(crate) async fn set_position(
    app: AppHandle<impl Runtime>,
    x: f64,
    y: f64,
) -> crate::Result<()> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        handle.set_position(x, y);
        Ok(())
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}

// ─── Sound & TTS ─────────────────────────────────────────────────────

#[command]
pub(crate) async fn register_sound(
    app: AppHandle<impl Runtime>,
    action: String,
    path: String,
    volume: Option<f64>,
) -> crate::Result<()> {
    let state = app.state::<PetPluginState>();
    let mut reg = state.sound_registry.write().await;
    reg.register_file(&action, std::path::Path::new(&path), volume.unwrap_or(1.0))
}

#[command]
pub(crate) async fn register_sound_bytes(
    app: AppHandle<impl Runtime>,
    action: String,
    data: Vec<u8>,
    format: String,
    volume: Option<f64>,
) -> crate::Result<()> {
    let audio_format = match format.as_str() {
        "wav" => AudioFormat::Wav,
        "ogg" => AudioFormat::Ogg,
        "mp3" => AudioFormat::Mp3,
        _ => return Err(crate::error::Error::InvalidAction(format!("Unsupported format: {format}"))),
    };
    let state = app.state::<PetPluginState>();
    let mut reg = state.sound_registry.write().await;
    reg.register_bytes(&action, data, audio_format, volume.unwrap_or(1.0));
    Ok(())
}

#[command]
pub(crate) async fn set_tts(
    app: AppHandle<impl Runtime>,
    provider: String,
    api_key: String,
    voice: Option<String>,
    region: Option<String>,
) -> crate::Result<()> {
    let tts: Box<dyn crate::audio::TtsProvider> = match provider.as_str() {
        "azure" => {
            let region = region.ok_or_else(|| {
                crate::error::Error::InvalidAction("Azure TTS requires 'region' parameter".into())
            })?;
            let voice = voice.unwrap_or_else(|| "en-US-JennyNeural".into());
            Box::new(AzureTts::new(api_key, region, voice))
        }
        "elevenlabs" => {
            let voice_id = voice.ok_or_else(|| {
                crate::error::Error::InvalidAction(
                    "ElevenLabs TTS requires 'voice' parameter".into(),
                )
            })?;
            Box::new(ElevenLabsTts::new(api_key, voice_id))
        }
        _ => {
            return Err(crate::error::Error::InvalidAction(format!(
                "Unknown TTS provider: {provider}"
            )))
        }
    };

    let state = app.state::<PetPluginState>();
    let mut reg = state.sound_registry.write().await;
    reg.set_tts(tts);
    Ok(())
}

#[command]
pub(crate) async fn say(
    app: AppHandle<impl Runtime>,
    text: String,
    kind: Option<String>,
) -> crate::Result<()> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        let bubble_kind = match kind.as_deref() {
            Some("thought") => BubbleKind::Thought,
            Some("action") => BubbleKind::Action,
            Some("system") => BubbleKind::System,
            _ => BubbleKind::Speech,
        };
        let bubble = BubbleContent {
            text,
            kind: bubble_kind,
            duration_ms: 3000,
            typing_animation: true,
            priority: BubblePriority::Normal,
        };
        handle.show_bubble(bubble);
        Ok(())
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}

// ─── Direct Control ──────────────────────────────────────────────────

#[command]
pub(crate) async fn play_action(
    app: AppHandle<impl Runtime>,
    action: String,
    loops: Option<u32>,
) -> crate::Result<()> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        let seq = ActionSequence::once(vec![
            SequenceStep::action(&action).with_loops(loops.unwrap_or(1)),
        ]);
        handle.play_sequence(seq);
        Ok(())
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}

#[command]
pub(crate) async fn show_bubble(
    app: AppHandle<impl Runtime>,
    text: String,
    kind: Option<String>,
    duration_ms: Option<u64>,
    typing: Option<bool>,
) -> crate::Result<()> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        let bubble_kind = match kind.as_deref() {
            Some("thought") => BubbleKind::Thought,
            Some("action") => BubbleKind::Action,
            Some("system") => BubbleKind::System,
            _ => BubbleKind::Speech,
        };
        let mut bubble = BubbleContent {
            text,
            kind: bubble_kind,
            duration_ms: duration_ms.unwrap_or(3000),
            typing_animation: typing.unwrap_or(true),
            priority: BubblePriority::Normal,
        };
        if bubble_kind == BubbleKind::System {
            bubble.priority = BubblePriority::High;
        }
        handle.show_bubble(bubble);
        Ok(())
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}

#[command]
pub(crate) async fn dismiss_bubble(
    app: AppHandle<impl Runtime>,
) -> crate::Result<()> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        handle.dismiss_bubble();
        Ok(())
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}

#[command]
pub(crate) async fn play_sequence(
    app: AppHandle<impl Runtime>,
    sequence: ActionSequence,
) -> crate::Result<()> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        handle.play_sequence(sequence);
        Ok(())
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}

#[command]
pub(crate) async fn stop_sequence(
    app: AppHandle<impl Runtime>,
) -> crate::Result<()> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        handle.stop_sequence();
        Ok(())
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}

// ─── Behavior ────────────────────────────────────────────────────────

#[command]
pub(crate) async fn set_behavior_config(
    app: AppHandle<impl Runtime>,
    config: BehaviorConfig,
) -> crate::Result<()> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        handle.set_behavior_config(config);
        Ok(())
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}

#[command]
pub(crate) async fn set_ambient_enabled(
    app: AppHandle<impl Runtime>,
    enabled: bool,
) -> crate::Result<()> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        handle.set_ambient_enabled(enabled);
        Ok(())
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}

// ─── Mood & Persistence ─────────────────────────────────────────────

#[command]
pub(crate) async fn get_stats(
    app: AppHandle<impl Runtime>,
) -> crate::Result<PetStats> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        let s = handle.current_state().await;
        Ok(s.stats)
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}

#[command]
pub(crate) async fn set_stats(
    app: AppHandle<impl Runtime>,
    stats: PetStats,
) -> crate::Result<()> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        handle.set_stats(stats);
        Ok(())
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}

#[command]
pub(crate) async fn save_state(
    app: AppHandle<impl Runtime>,
) -> crate::Result<()> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        handle.save_state();
        Ok(())
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}

#[command]
pub(crate) async fn load_saved_state(
    pet_id: String,
) -> crate::Result<PetSnapshot> {
    let store = crate::mood::PetStore::default_store();
    store
        .load(&pet_id)?
        .ok_or_else(|| crate::error::Error::NotFound(format!("No saved state for pet {pet_id}")))
}

#[command]
pub(crate) async fn list_downloaded_pets(
    app: AppHandle<impl Runtime>,
) -> crate::Result<Vec<PetConfig>> {
    let state = app.state::<PetPluginState>();
    state.resource.list_cached_pets().await
}

// ─── Query ────────────────────────────────────────────────────────────

#[command]
pub(crate) async fn get_state(
    app: AppHandle<impl Runtime>,
) -> crate::Result<PetState> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        Ok(handle.current_state().await)
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}

#[command]
pub(crate) async fn get_pet_meta(
    app: AppHandle<impl Runtime>,
) -> crate::Result<PetMeta> {
    let state = app.state::<PetPluginState>();
    let meta = state.current_pet.read().await;
    meta.clone()
        .ok_or_else(|| crate::error::Error::Runtime("No pet loaded".into()))
}

#[command]
pub(crate) async fn get_actions(
    app: AppHandle<impl Runtime>,
) -> crate::Result<Vec<ActionDef>> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        Ok(handle.get_actions().await)
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}

#[command]
pub(crate) async fn get_position(
    app: AppHandle<impl Runtime>,
) -> crate::Result<PositionInfo> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        Ok(handle.get_position().await)
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}

#[command]
pub(crate) async fn list_remote_pets(
    app: AppHandle<impl Runtime>,
    page: Option<u32>,
    page_size: Option<u32>,
) -> crate::Result<PetListResponse> {
    let state = app.state::<PetPluginState>();
    state.resource.list_pets(page.unwrap_or(1), page_size.unwrap_or(20)).await
}

#[command]
pub(crate) async fn search_remote_pets(
    app: AppHandle<impl Runtime>,
    query: String,
    page: Option<u32>,
    page_size: Option<u32>,
) -> crate::Result<PetListResponse> {
    let state = app.state::<PetPluginState>();
    state.resource.search_pets(&query, page.unwrap_or(1), page_size.unwrap_or(20)).await
}

// ─── Mutation ─────────────────────────────────────────────────────────

#[command]
pub(crate) async fn delete_saved_state(
    pet_id: String,
) -> crate::Result<()> {
    let store = crate::mood::PetStore::default_store();
    store.delete(&pet_id)
}

#[command]
pub(crate) async fn clear_cache(
    app: AppHandle<impl Runtime>,
    pet_id: Option<String>,
) -> crate::Result<()> {
    let state = app.state::<PetPluginState>();
    state.resource.clear_cache(pet_id.as_deref()).await
}

#[command]
pub(crate) async fn set_mood_config(
    app: AppHandle<impl Runtime>,
    config: MoodConfig,
) -> crate::Result<()> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        handle.set_mood_config(config);
        Ok(())
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}

#[command]
pub(crate) async fn set_event_binding(
    app: AppHandle<impl Runtime>,
    event_key: String,
    action: String,
) -> crate::Result<()> {
    let state = app.state::<PetPluginState>();
    let handle_guard = state.handle.read().await;
    if let Some(ref handle) = *handle_guard {
        handle.set_event_binding(event_key, action);
        Ok(())
    } else {
        Err(crate::error::Error::Runtime("No pet loaded".into()))
    }
}
