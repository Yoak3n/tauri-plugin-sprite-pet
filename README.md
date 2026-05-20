# tauri-plugin-sprite-pet

A Tauri v2 plugin for animated sprite pet desktop companions. Renders sprite sheet animations, handles user interactions (drag, click), manages mood/stats, and supports autonomous ambient behavior.

## Installation

### Tauri (Setup quickly)
```bash
cargo tauri add sprite-pet
```

### Rust (Cargo.toml)

```toml
[dependencies]
# Should use latest version
tauri-plugin-sprite-pet = "0.1"
```

### JavaScript (package.json)

```bash
# pnpm
pnpm add tauri-plugin-sprite-pet-api

# npm
npm install tauri-plugin-sprite-pet-api

# yarn
yarn add tauri-plugin-sprite-pet-api
```

### Register the plugin (lib.rs / main.rs)

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_sprite_pet::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### Permissions (capabilities/*.json)

Add `sprite-pet:default` to your app's capabilities:

```json
{
  "permissions": ["sprite-pet:default"]
}
```

## Quick Start

### PetRenderer (Recommended)

`PetRenderer` is a high-level class that manages the full lifecycle: loading the pet, processing the spritesheet, listening for render commands, and drawing automatically.

```typescript
import { PetRenderer, playAction, say, showBubble } from 'tauri-plugin-sprite-pet-api'

const canvas = document.getElementById('pet-canvas') as HTMLCanvasElement
const renderer = new PetRenderer(canvas)

// Load a pet — this handles everything:
// - Calls loadPet on the backend
// - Loads the spritesheet image from bytes
// - Subscribes to pet://command events
// - Draws frames automatically on render events
// - Polls mood stats periodically
await renderer.load('her-os1')

// Optional: subscribe to lifecycle callbacks
renderer.onRender = (action, frame, facing) => {
  console.log(`${action} frame ${frame} facing ${facing}`)
}
renderer.onBubble = (text, kind) => {
  console.log(`${kind}: ${text}`)
}
renderer.onStats = (stats, moodLabel) => {
  console.log(`Mood: ${moodLabel}`, stats)
}

// Trigger animations
await playAction('waving')
await playAction('jumping')

// Show speech bubbles
await say('Hello!')
await showBubble('Thinking...', 'thought', 3000)

// Cleanup when done
renderer.dispose()
```

### Lower-Level API

For more control, use the individual functions directly:

```typescript
import {
  loadPet, unloadPet, playAction, say, showBubble, dismissBubble,
  triggerEvent, setAmbientEnabled, getStats, playSequence,
  drawFrame, createSpriteRenderer,
} from 'tauri-plugin-sprite-pet-api'
import { listen } from '@tauri-apps/api/event'

// Load a pet
const result = await loadPet('her-os1')

// Option A: Use drawFrame directly
const img = new Image()
const blob = new Blob([new Uint8Array(result.spritesheetBytes)])
img.src = URL.createObjectURL(blob)
img.onload = () => {
  const canvas = document.getElementById('pet-canvas') as HTMLCanvasElement
  drawFrame(canvas, img, result.config, 'idle', 0, 'right')
}

// Option B: Use createSpriteRenderer for managed rendering
const canvas = document.getElementById('pet-canvas') as HTMLCanvasElement
const renderer = createSpriteRenderer(canvas, result)
await listen('pet://command', (event) => {
  const cmd = event.payload
  if (cmd.type === 'render') {
    renderer.handleCommand(cmd)
    renderer.draw(cmd.action, cmd.frame_index, cmd.facing)
  }
})
```

## Rust Backend Usage

The crate can be used directly from Rust code as a library, without going through Tauri commands. This is useful for custom backends, headless operation, or integrating the pet into non-Tauri Rust applications.

### Pet (Recommended)

The `Pet` struct wraps the entire workflow — resource download, validation, runtime start — into a single call.

```rust
use tauri_plugin_sprite_pet::{Pet, PetStats};

// Simplest: load from default provider (codex-pets.net)
let pet = Pet::start("her-os1").await?;

// Interact
pet.say("Hello!");
pet.play("waving");
pet.think("What should I do?");
pet.send_event(PetEvent::Click);
pet.set_position(100.0, 200.0);

// Query
let state = pet.state().await;
println!("{} is {} (mood: {})", state.pet_id, state.action, state.mood_label);

let (x, y, facing) = pet.position().await;
let actions = pet.actions().await;

// Configure
pet.set_ambient_enabled(true);
pet.set_stats(PetStats { happiness: 100, energy: 80, ..Default::default() });

// Save & shutdown
pet.save();
pet.shutdown();
```

#### Builder Pattern

Use `Pet::builder()` for custom options:

```rust
use tauri_plugin_sprite_pet::{Pet, FrameLayout, BehaviorConfig, MoodConfig};

// Custom API provider
let pet = Pet::builder("endminguga")
    .api_url("https://codexpet.xyz")
    .start()
    .await?;

// Custom sprite layout
let pet = Pet::builder("my-pet")
    .layout(FrameLayout { columns: 6, rows: 4, cell_width: 128, cell_height: 128 })
    .start()
    .await?;

// Full customization
let pet = Pet::builder("her-os1")
    .api_url("https://codex-pets.net")
    .behavior_config(BehaviorConfig {
        idle_timeout_ms: 5000,
        ..Default::default()
    })
    .mood_config(MoodConfig {
        decay_interval_ms: 60000,
        ..Default::default()
    })
    .initial_stats(PetStats { happiness: 100, ..Default::default() })
    .start()
    .await?;
```

### Resource Client

For direct resource access without starting a runtime:

```rust
use tauri_plugin_sprite_pet::{ResourceClient, ResourceConfig, ResourceProvider};

let config = ResourceConfig {
    api_base_url: "https://codex-pets.net".into(),
    provider: ResourceProvider::CodexPets,
    ..ResourceConfig::default()
};
let client = ResourceClient::new(config)?;

// Fetch pet metadata
let meta = client.get_pet("her-os1").await?;
println!("Pet: {} ({})", meta.display_name, meta.id);

// Download spritesheet
let path = client.fetch_spritesheet(&meta.id, &meta.spritesheet_url).await?;

// Search for pets
let results = client.search_pets("anime", 1, 10).await?;
println!("Found {} pets", results.total);
```

### Define Custom Animations

```rust
use tauri_plugin_sprite_pet::{ActionRegistry, ActionPlayer, models::ActionDef};

// Create from default actions
let registry = ActionRegistry::default_registry();

// Or define custom actions
let registry = ActionRegistry::new(vec![
    ActionDef {
        name: "dance".into(),
        row: 0,
        frame_count: 8,
        frame_duration_ms: 100,
        looping: true,
        interruptible: true,
        loop_rest_ms: Some(200),
        last_frame_hold_ms: None,
    },
    ActionDef {
        name: "attack".into(),
        row: 1,
        frame_count: 6,
        frame_duration_ms: 80,
        looping: false,
        interruptible: false,
        loop_rest_ms: None,
        last_frame_hold_ms: Some(500),
    },
]);

// Simulate animation frames
let mut player = ActionPlayer::new("dance");
let frame_changed = player.tick(100, &registry); // advance by 100ms
println!("Action: {}, Frame: {}, Finished: {}",
    player.current_action, player.current_frame, player.finished);
```

### Control the Runtime Directly (Low-Level)

For full control over the runtime, use `start_pet` directly. Most users should prefer [`Pet`](#pet-recommended) instead.

```rust
use tauri_plugin_sprite_pet::{
    start_pet, PetRuntimeConfig, PetHandle, SpriteSheet, FrameLayout,
    ActionRegistry, EventActionMap, BehaviorConfig, MoodConfig,
    models::{PetEvent, Facing},
};

// Create a SpriteSheet from the default layout (8x9 grid, 192x208 cells)
let spritesheet = SpriteSheet::new(FrameLayout::default());

// Or with a custom layout
let spritesheet = SpriteSheet::new(FrameLayout {
    columns: 6,
    rows: 4,
    cell_width: 128,
    cell_height: 128,
});

// Or load from a spritesheet image file (for validation / frame detection)
use tauri_plugin_sprite_pet::sprite::load_spritesheet;
let (img, spritesheet) = load_spritesheet(
    std::path::Path::new("path/to/spritesheet.webp"),
    FrameLayout::default(),
)?;

// Build runtime config
let runtime_config = PetRuntimeConfig {
    action_registry: ActionRegistry::default_registry(),
    event_map: EventActionMap::default_map(),
    behavior_config: Some(BehaviorConfig::default()),
    mood_config: Some(MoodConfig::default()),
    initial_stats: None,
    sound_registry: None,
};

// Start the pet runtime (returns a handle for control)
let handle: PetHandle = start_pet("my-pet".into(), spritesheet, runtime_config);

// Interact with the pet
handle.send_event(PetEvent::Click);
handle.send_event(PetEvent::Walk { direction: Facing::Left });
handle.set_position(100.0, 200.0);

// Show bubbles
handle.show_bubble(BubbleContent::speech("Hello from Rust!"));

// Query state
let state = handle.current_state().await;
println!("{} is {} (mood: {})", state.pet_id, state.action, state.mood_label);

// Get position
let pos = handle.get_position().await;
println!("Position: ({}, {}), facing {:?}", pos.x, pos.y, pos.facing);

// Get available actions
let actions = handle.get_actions().await;
for action in &actions {
    println!("  {}: {} frames, {}ms", action.name, action.frame_count, action.frame_duration_ms);
}

// Update behavior at runtime
handle.set_behavior_config(custom_behavior_config);
handle.set_mood_config(custom_mood_config);
handle.set_event_binding("click".into(), "jumping".into());

// Shutdown
handle.shutdown();
```

### Standalone Components

Each subsystem can be used independently:

```rust
use tauri_plugin_sprite_pet::{
    BehaviorEngine, MoodTracker, EventActionMap,
    BubbleManager, BubbleContent,
    SequenceExecutor,
    models::{BehaviorConfig, MoodConfig, PetStats, PetEvent},
};

// Behavior engine - autonomous actions
let mut behavior = BehaviorEngine::new(BehaviorConfig::default());
let stats = PetStats::default();
if let Some(tick) = behavior.tick(&stats) {
    println!("Ambient action: {}", tick.action);
}

// Mood tracking
let mut mood = MoodTracker::new(PetStats::default(), MoodConfig::default());
mood.on_interaction(); // boost mood on user interaction
mood.tick(); // decay stats over time

// Event mapping
let mut event_map = EventActionMap::default_map();
event_map.bind("click", "jumping"); // customize event-to-action mapping

// Bubble queue with priority
let mut bubbles = BubbleManager::new();
bubbles.show(BubbleContent::speech("Hello").with_duration(3000));
bubbles.show(BubbleContent::system("Important!")); // high priority, interrupts
```

## Resource Providers

The plugin supports two sprite resource sites. Developers choose which site to use by passing the API base URL to `loadPet`:

| Provider | Base URL | Pet ID Example |
|----------|----------|----------------|
| codex-pets.net | `https://codex-pets.net` (default) | `her-os1`, `chibi-rem` |
| codexpet.xyz | `https://codexpet.xyz` | `endminguga` |

```typescript
// Default (codex-pets.net)
await loadPet('her-os1')

// Explicit codex-pets.net
await loadPet('her-os1', 'https://codex-pets.net')

// codexpet.xyz
await loadPet('endminguga', 'https://codexpet.xyz')
```

## Sprite Sheet Format

The plugin expects sprite sheets organized as a grid:

- **Default layout**: 8 columns x 9 rows, 192x208 pixels per cell
- **Each row** represents one action/animation
- **Columns** are sequential frames of that animation
- Frame counts per row are auto-detected from non-transparent pixels

### Default Actions

| Row | Action | Frames | Loop | Frame Duration | Notes |
|-----|--------|--------|------|---------------|-------|
| 0 | idle | 6 | yes | 120ms | 500ms rest at last frame |
| 1 | running_right | 9 | yes | 100ms | - |
| 2 | running_left | 9 | yes | 80ms | Not interruptible |
| 3 | waving | 4 | no | 100ms | 200ms hold on last frame |
| 4 | jumping | 5 | no | 100ms | 300ms hold on last frame |
| 5 | failed | 9 | no | 100ms | 200ms hold on last frame |
| 6 | waiting | 6 | yes | 200ms | - |
| 7 | running | 6 | no | 120ms | 400ms hold on last frame |
| 8 | review | 6 | no | 100ms | 300ms hold on last frame |

Frame counts are auto-detected, so sprite sheets with fewer frames per row work correctly.

## API Reference

### Lifecycle

#### `loadPet(petId: string, apiBaseUrl?: string): Promise<LoadPetResult>`

Load a pet by ID. Downloads the spritesheet, validates it, detects frame counts, and starts the animation runtime.

- `petId` - The pet identifier from the resource site
- `apiBaseUrl` - Optional API base URL (default: `https://codex-pets.net`)

Returns `{ config: PetConfig, spritesheetBytes: number[] }`.

#### `unloadPet(): Promise<void>`

Unload the current pet, save its state, and stop the runtime.

### Rendering

#### `PetRenderer` class (Recommended)

High-level renderer that manages the full pet lifecycle: load → process spritesheet → listen for render commands → draw automatically.

```typescript
import { PetRenderer } from 'tauri-plugin-sprite-pet-api'

const renderer = new PetRenderer(canvas)

// Simple: load from pet ID
await renderer.load('her-os1')

// Or two-step: load pet first, then apply to renderer
// (useful when you need the config before the canvas is in the DOM)
const result = await loadPet('her-os1')
petConfig = result.config
loaded = true
await tick() // wait for canvas to render
const renderer = new PetRenderer(canvas)
await renderer.load(result) // pass existing result

// Optional callbacks
renderer.onRender = (action, frame, facing) => { /* ... */ }
renderer.onBubble = (text, kind) => { /* ... */ }
renderer.onBubbleDismiss = () => { /* ... */ }
renderer.onStats = (stats, moodLabel) => { /* ... */ }

// Cleanup
renderer.dispose()
```

`PetRenderer` properties and methods:

| Member | Type | Description |
|--------|------|-------------|
| `action` | `string` | Current action name |
| `frame` | `number` | Current frame index |
| `facing` | `'left' \| 'right'` | Current facing direction |
| `bubble` | `string \| null` | Current bubble text |
| `stats` | `PetStats` | Current mood stats |
| `moodLabel` | `string` | Computed mood label |
| `ready` | `boolean` | Whether a pet is loaded |
| `load(petIdOrResult, apiBaseUrl?)` | `Promise<LoadPetResult>` | Load a pet and start rendering. Accepts a pet ID string or an existing `LoadPetResult`. |
| `draw(action, frame, facing)` | `void` | Manually draw a frame |
| `playAction(action, loops?)` | `Promise<void>` | Play an action animation |
| `say(text, kind?)` | `Promise<void>` | Show a speech bubble |
| `showBubble(text, kind?, durationMs?, typing?)` | `Promise<void>` | Show a bubble with full control |
| `dismissBubble()` | `Promise<void>` | Dismiss the current bubble |
| `setAmbientEnabled(enabled)` | `Promise<void>` | Toggle ambient behavior |
| `playSequence(sequence)` | `Promise<void>` | Play an action sequence |
| `getConfig()` | `PetConfig \| null` | Get the loaded pet config |
| `getImage()` | `HTMLImageElement \| null` | Get the spritesheet image |
| `dispose()` | `void` | Clean up all resources |

#### `drawFrame(canvas, image, config, action, frame, facing): void`

Draw a single sprite frame onto a canvas. This is a standalone utility function.

```typescript
import { drawFrame } from 'tauri-plugin-sprite-pet-api'

drawFrame(canvas, image, config, 'idle', 0, 'right')
```

#### `createSpriteRenderer(canvas, loadResult): SpriteRenderer`

Lower-level renderer that manages the spritesheet image. Requires manual event handling.

```typescript
import { createSpriteRenderer, loadPet } from 'tauri-plugin-sprite-pet-api'
import { listen } from '@tauri-apps/api/event'

const result = await loadPet('her-os1')
const renderer = createSpriteRenderer(canvas, result)

// Auto-draw on render commands
await listen('pet://command', (e) => {
  if (e.payload.type === 'render') {
    renderer.handleCommand(e.payload)
    renderer.draw(e.payload.action, e.payload.frame_index, e.payload.facing)
  }
})

// Cleanup when done
renderer.dispose()
```

`SpriteRenderer` methods:

| Method | Description |
|--------|-------------|
| `draw(action, frame, facing)` | Draw a specific frame |
| `handleCommand(cmd)` | Update state from a `pet://command` render payload |
| `startLoop()` | Start a `requestAnimationFrame` render loop |
| `stopLoop()` | Stop the render loop |
| `dispose()` | Stop loop and revoke object URL |
| `ready` | Whether the image is loaded |
| `config` | The `PetConfig` |
| `image` | The `HTMLImageElement` |
| `image` | The `HTMLImageElement` |

### Animation

#### `playAction(action: string, loops?: number): Promise<void>`

Play a specific action animation.

```typescript
await playAction('idle')      // Play once
await playAction('waving', 3) // Play 3 loops
```

#### `playSequence(sequence: ActionSequence): Promise<void>`

Play a choreographed sequence of actions.

```typescript
await playSequence({
  steps: [
    { action: 'waving', waitForComplete: true, delayMs: 0 },
    { action: 'jumping', waitForComplete: true, delayMs: 200 },
    { action: 'idle', waitForComplete: false, delayMs: 0 },
  ],
  repeat: { type: 'once' },
  onComplete: 'idle',
})
```

#### `stopSequence(): Promise<void>`

Stop the currently playing sequence.

### Bubbles & Speech

#### `say(text: string, kind?: string): Promise<void>`

Show a speech bubble. `kind` can be `'speech'` (default), `'thought'`, `'action'`, or `'system'`.

#### `showBubble(text: string, kind?: string, durationMs?: number, typing?: boolean): Promise<void>`

Show a bubble with full control over duration and typing animation.

#### `dismissBubble(): Promise<void>`

Dismiss the current bubble.

### Interaction

#### `triggerEvent(event: PetEvent): Promise<void>`

Send a user interaction event.

```typescript
await triggerEvent({ type: 'click' })
await triggerEvent({ type: 'drag_start' })
await triggerEvent({ type: 'drag_move', data: { x: 100, y: 200 } })
await triggerEvent({ type: 'drag_drop' })
```

#### `setPosition(x: number, y: number): Promise<void>`

Update the pet's screen position.

### Behavior & Mood

#### `setAmbientEnabled(enabled: boolean): Promise<void>`

Enable or disable autonomous ambient behavior (idle animations, random actions).

#### `getStats(): Promise<PetStats>`

Get the current mood stats: `{ happiness, energy, social, boredom }` (0-100 each).

#### `setStats(stats: PetStats): Promise<void>`

Override the pet's mood stats.

#### `setBehaviorConfig(config: BehaviorConfig): Promise<void>`

Update the behavior engine configuration (idle timeout, ambient intervals, action weights).

#### `setMoodConfig(config: MoodConfig): Promise<void>`

Update the mood decay configuration at runtime.

#### `setEventBinding(eventKey: string, action: string): Promise<void>`

Customize an event-to-action binding. For example, make clicks trigger jumping instead of the default:

```typescript
await setEventBinding('click', 'jumping')
```

### Audio & TTS

#### `registerSound(action: string, path: string, volume?: number): Promise<void>`

Register a sound file to play when an action starts.

#### `registerSoundBytes(action: string, data: number[], format: 'wav' | 'ogg' | 'mp3', volume?: number): Promise<void>`

Register raw audio bytes for an action.

#### `setTts(provider: string, apiKey: string, voice?: string, region?: string): Promise<void>`

Configure TTS. Supports `'azure'` (requires `region`) and `'elevenlabs'` (requires `voice`).

### Persistence

#### `saveState(): Promise<void>`

Manually trigger a state save to disk.

#### `loadSavedState(petId: string): Promise<PetSnapshot>`

Load a previously saved pet state.

#### `listDownloadedPets(): Promise<PetConfig[]>`

List all previously downloaded pets from local cache.

#### `deleteSavedState(petId: string): Promise<void>`

Delete a previously saved pet state from disk.

#### `clearCache(petId?: string): Promise<void>`

Clear the local cache for a specific pet or all pets.

### Query

#### `getState(): Promise<PetState>`

Get the full current pet state (action, frame, position, facing, bubble, stats, mood).

#### `getPetMeta(): Promise<PetMeta>`

Get the current pet's metadata (name, description, owner, stats, tags, etc.).

#### `getActions(): Promise<ActionDef[]>`

Get the list of available animation actions for the current pet.

#### `getPosition(): Promise<PositionInfo>`

Get the current position and facing direction.

#### `listRemotePets(page?, pageSize?): Promise<PetListResponse>`

List pets from the remote API (paginated).

#### `searchRemotePets(query, page?, pageSize?): Promise<PetListResponse>`

Search pets on the remote API by query string.

## Events

Listen to events via `@tauri-apps/api/event`:

### `pet://command`

Main event channel. Payload types:

| type | Fields | Description |
|------|--------|-------------|
| `render` | `action`, `frame_index`, `facing`, `x`, `y`, `scale` | Draw a sprite frame |
| `bubble` | `text`, `kind`, `duration_ms` | Show a bubble |
| `dismiss_bubble` | - | Dismiss the current bubble |
| `audio` | `audio_bytes`, `format`, `volume` | Play audio |
| `action_finished` | `action` | Non-looping animation completed |

### `pet://loaded`

Fired when a pet is successfully loaded. Payload is the `PetConfig`.

### `pet://unloaded`

Fired when a pet is unloaded. Payload: `{ petId: string }`.

## Example

See `examples/tauri-app/` for a complete working demo with:

- Pet loading from both resource providers
- Canvas-based sprite rendering with `createSpriteRenderer`
- Action buttons and keyboard shortcuts
- Drag interaction
- Speech bubbles
- Mood stats display
- Ambient behavior toggle

Run the example:

```bash
cd examples/tauri-app
pnpm install
pnpm tauri dev
```

## License

MIT
