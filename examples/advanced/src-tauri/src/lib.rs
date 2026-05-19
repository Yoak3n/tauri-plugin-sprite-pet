use serde::Serialize;
use std::sync::Arc;
use tauri::{command, AppHandle, State};
use tokio::sync::Mutex;

// ─── State ──────────────────────────────────────────────────────

struct PetState {
    pet: Mutex<Option<Arc<tauri_plugin_sprite_pet::Pet>>>,
}

#[derive(Serialize)]
struct LoadResult {
    config: tauri_plugin_sprite_pet::PetConfig,
    spritesheet_bytes: Vec<u8>,
}

// ─── Commands ───────────────────────────────────────────────────

/// Load a pet using the high-level Pet API.
/// Downloads spritesheet, validates, starts runtime, and bridges events to frontend.
#[command]
async fn load_pet(
    app: AppHandle,
    state: State<'_, PetState>,
    pet_id: String,
    api_url: Option<String>,
) -> Result<LoadResult, String> {
    // Shutdown existing pet
    {
        let mut guard = state.pet.lock().await;
        if let Some(pet) = guard.take() {
            pet.shutdown();
        }
    }

    // Build and start pet
    let mut builder = tauri_plugin_sprite_pet::Pet::builder(&pet_id);
    if let Some(url) = api_url {
        builder = builder.api_url(&url);
    }
    let pet = Arc::new(builder.start().await.map_err(|e| e.to_string())?);

    // Bridge commands to frontend events (one call — no manual spawn)
    pet.bridge_to_tauri(&app);

    // Get config and spritesheet bytes for frontend rendering
    let config = pet.config().clone();
    let spritesheet_bytes = tokio::fs::read(&config.spritesheet_path)
        .await
        .map_err(|e| format!("Failed to read spritesheet: {e}"))?;

    *state.pet.lock().await = Some(pet);
    Ok(LoadResult {
        config,
        spritesheet_bytes,
    })
}

#[command]
async fn unload_pet(state: State<'_, PetState>) -> Result<(), String> {
    let mut guard = state.pet.lock().await;
    if let Some(pet) = guard.take() {
        pet.shutdown();
    }
    Ok(())
}

#[command]
async fn play_action(
    state: State<'_, PetState>,
    action: String,
    loops: Option<u32>,
) -> Result<(), String> {
    let guard = state.pet.lock().await;
    let pet = guard.as_ref().ok_or("No pet loaded")?;
    pet.play_n(&action, loops.unwrap_or(1));
    Ok(())
}

#[command]
async fn say(state: State<'_, PetState>, text: String) -> Result<(), String> {
    let guard = state.pet.lock().await;
    let pet = guard.as_ref().ok_or("No pet loaded")?;
    pet.say(&text);
    Ok(())
}

#[command]
async fn think(state: State<'_, PetState>, text: String) -> Result<(), String> {
    let guard = state.pet.lock().await;
    let pet = guard.as_ref().ok_or("No pet loaded")?;
    pet.think(&text);
    Ok(())
}

#[command]
async fn dismiss_bubble(state: State<'_, PetState>) -> Result<(), String> {
    let guard = state.pet.lock().await;
    let pet = guard.as_ref().ok_or("No pet loaded")?;
    pet.dismiss_bubble();
    Ok(())
}

#[command]
async fn toggle_ambient(state: State<'_, PetState>, enabled: bool) -> Result<(), String> {
    let guard = state.pet.lock().await;
    let pet = guard.as_ref().ok_or("No pet loaded")?;
    pet.set_ambient_enabled(enabled);
    Ok(())
}

#[command]
async fn trigger_drag(
    state: State<'_, PetState>,
    event_type: String,
    x: Option<f64>,
    y: Option<f64>,
) -> Result<(), String> {
    let guard = state.pet.lock().await;
    let pet = guard.as_ref().ok_or("No pet loaded")?;
    use tauri_plugin_sprite_pet::PetEvent;
    let event = match event_type.as_str() {
        "drag_start" => PetEvent::DragStart,
        "drag_move" => PetEvent::DragMove {
            x: x.unwrap_or(0.0),
            y: y.unwrap_or(0.0),
        },
        "drag_drop" => PetEvent::DragDrop,
        _ => return Err(format!("Unknown event: {event_type}")),
    };
    pet.send_event(event);
    Ok(())
}

// ─── Entry ──────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(PetState {
            pet: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            load_pet,
            unload_pet,
            play_action,
            say,
            think,
            dismiss_bubble,
            toggle_ambient,
            trigger_drag,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
