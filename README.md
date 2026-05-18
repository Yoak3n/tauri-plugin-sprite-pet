# tauri-plugin-sprite-pet

A Tauri v2 plugin for animated sprite pet desktop companions. Renders sprite sheet animations, handles user interactions (drag, click), manages mood/stats, and supports autonomous ambient behavior.

## Installation

### Rust (Cargo.toml)

```toml
[dependencies]
tauri-plugin-sprite-pet = { path = "../tauri-plugin-sprite-pet" }
```

### JavaScript (package.json)

```json
{
  "dependencies": {
    "tauri-plugin-sprite-pet-api": "file:../tauri-plugin-sprite-pet/guest-js"
  }
}
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

```typescript
import {
  loadPet, unloadPet, playAction, say, showBubble, dismissBubble,
  triggerEvent, setAmbientEnabled, getStats, playSequence,
} from 'tauri-plugin-sprite-pet-api'
import { listen } from '@tauri-apps/api/event'

// Load a pet from codex-pets.net (default)
const result = await loadPet('her-os1')

// Or load from codexpet.xyz
const result = await loadPet('endminguga', 'https://codexpet.xyz')

// Listen for render commands and draw to canvas
await listen('pet://command', (event) => {
  const cmd = event.payload
  if (cmd.type === 'render') {
    // Draw frame from spritesheet
    drawSpriteFrame(cmd.action, cmd.frame_index, cmd.facing)
  }
})

// Trigger animations
await playAction('click')
await playAction('walk')
await playAction('special')

// Show speech bubbles
await say('Hello!')
await showBubble('Thinking...', 'thought', 3000)
await dismissBubble()
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
| 0 | idle | 8 | yes | 120ms | 500ms rest at last frame |
| 1 | walk | 8 | yes | 100ms | - |
| 2 | drag | 8 | yes | 80ms | Not interruptible |
| 3 | drop | 8 | no | 100ms | 200ms hold on last frame |
| 4 | click | 8 | no | 100ms | 300ms hold on last frame |
| 5 | double_click | 8 | no | 100ms | 200ms hold on last frame |
| 6 | sleep | 8 | yes | 200ms | - |
| 7 | wake | 8 | no | 120ms | 400ms hold on last frame |
| 8 | special | 8 | no | 100ms | 300ms hold on last frame |

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

### Animation

#### `playAction(action: string, loops?: number): Promise<void>`

Play a specific action animation.

```typescript
await playAction('click')      // Play once
await playAction('walk', 3)    // Play 3 loops
```

#### `playSequence(sequence: ActionSequence): Promise<void>`

Play a choreographed sequence of actions.

```typescript
await playSequence({
  steps: [
    { action: 'click', waitForComplete: true, delayMs: 0 },
    { action: 'special', waitForComplete: true, delayMs: 200 },
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
- Canvas-based sprite rendering
- Action buttons and keyboard shortcuts (1=idle, 2=walk, 3=click, 4=special)
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
