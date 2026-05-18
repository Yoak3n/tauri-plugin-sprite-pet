import { invoke } from '@tauri-apps/api/core'

// ─── Types ──────────────────────────────────────────────────────────

export interface PetConfig {
  id: string
  displayName: string
  spritesheetPath: string
  layout: {
    columns: number
    rows: number
    cellWidth: number
    cellHeight: number
  }
  actions: Array<{
    name: string
    row: number
    frameCount: number
    frameDurationMs: number
    looping: boolean
    interruptible?: boolean
    loopRestMs?: number
    lastFrameHoldMs?: number
  }>
}

export interface LoadPetResult {
  config: PetConfig
  spritesheetBytes: number[]
}

export interface PetMeta {
  id: string
  displayName: string
  description: string
  spritesheetPath?: string
  kind?: 'person' | 'animal' | 'object' | 'creature'
  ownerId?: string
  ownerHandle?: string
  ownerName?: string
  uploadedAt?: string
  viewCount?: number
  downloadCount?: number
  likeCount?: number
  commentCount?: number
  likedByMe?: boolean
  ownerShadowbanned?: boolean
  tags?: string[]
  spritesheetUrl?: string
  posterUrl?: string
  previewUrl?: string
  shareImageUrl?: string
  downloadUrl?: string
  validationReport?: ValidationReport
}

export interface ValidationReport {
  manifestId: string
  atlasSize: string
  cellSize: string
  statesDetected: number
  manifestBytes: number
  spritesheetBytes: number
}

export type PetEvent =
  | { type: 'idle' }
  | { type: 'walk'; data: { direction: 'left' | 'right' } }
  | { type: 'drag_start' }
  | { type: 'drag_move'; data: { x: number; y: number } }
  | { type: 'drag_drop' }
  | { type: 'click' }
  | { type: 'double_click' }
  | { type: 'sleep' }
  | { type: 'wake' }
  | { type: 'custom'; data: string }

export interface PetStats {
  happiness: number
  energy: number
  social: number
  boredom: number
}

export interface PetSnapshot {
  petId: string
  position: { x: number; y: number }
  facing: 'left' | 'right'
  stats: PetStats
  currentAction: string
  savedAt: string
}

export interface BehaviorConfig {
  idleTimeoutMs: number
  ambientIntervalMinMs: number
  ambientIntervalMaxMs: number
  ambientActions: AmbientAction[]
  moodInfluence: number
}

export interface AmbientAction {
  action: string
  weight: number
  bubble?: string
  minMood?: number
  maxMood?: number
  cooldownMs: number
}

export interface ActionSequence {
  steps: SequenceStep[]
  repeat: { type: 'once' } | { type: 'loop' } | { type: 'loop_n'; data: number }
  onComplete?: string
}

export interface SequenceStep {
  action: string
  loops?: number
  sound?: { soundId: string; volume: number; delayMs: number }
  bubble?: { text: string; kind: string; durationMs: number; typingAnimation: boolean }
  waitForComplete: boolean
  delayMs: number
}

export interface PetCommand {
  type: 'render' | 'audio' | 'bubble' | 'dismiss_bubble' | 'action_finished'
  [key: string]: unknown
}

// ─── Commands ───────────────────────────────────────────────────────

/**
 * Load a pet by ID. Downloads spritesheet, generates config, and starts the runtime.
 * @param petId - The pet identifier (e.g. "her-os1", "endminguga")
 * @param apiBaseUrl - Optional API base URL. Use "https://codexpet.xyz" for codexpet.xyz,
 *                     or "https://codex-pets.net" (default) for codex-pets.net.
 */
export async function loadPet(petId: string, apiBaseUrl?: string): Promise<LoadPetResult> {
  return await invoke<LoadPetResult>('plugin:sprite-pet|load_pet', { petId, apiBaseUrl })
}

/** Unload the current pet and save its state. */
export async function unloadPet(): Promise<void> {
  return await invoke('plugin:sprite-pet|unload_pet')
}

/** Send a user interaction event to the pet. */
export async function triggerEvent(event: PetEvent): Promise<void> {
  return await invoke('plugin:sprite-pet|trigger_event', { event })
}

/** Update the pet's screen position. */
export async function setPosition(x: number, y: number): Promise<void> {
  return await invoke('plugin:sprite-pet|set_position', { x, y })
}

/** Register a sound file for an action. */
export async function registerSound(action: string, path: string, volume?: number): Promise<void> {
  return await invoke('plugin:sprite-pet|register_sound', { action, path, volume })
}

/** Register raw audio bytes for an action. */
export async function registerSoundBytes(
  action: string,
  data: number[],
  format: 'wav' | 'ogg' | 'mp3',
  volume?: number
): Promise<void> {
  return await invoke('plugin:sprite-pet|register_sound_bytes', {
    action, data, format, volume,
  })
}

/** Configure the TTS provider (azure or elevenlabs). */
export async function setTts(
  provider: string,
  apiKey: string,
  voice?: string,
  region?: string
): Promise<void> {
  return await invoke('plugin:sprite-pet|set_tts', {
    provider, apiKey, voice, region,
  })
}

/** Make the pet say something (shows a speech bubble). */
export async function say(text: string, kind?: string): Promise<void> {
  return await invoke('plugin:sprite-pet|say', { text, kind })
}

/** Play a specific action animation. */
export async function playAction(action: string, loops?: number): Promise<void> {
  return await invoke('plugin:sprite-pet|play_action', { action, loops })
}

/** Show a dialogue bubble with full control. */
export async function showBubble(
  text: string,
  kind?: string,
  durationMs?: number,
  typing?: boolean
): Promise<void> {
  return await invoke('plugin:sprite-pet|show_bubble', {
    text, kind, durationMs, typing,
  })
}

/** Dismiss the current bubble. */
export async function dismissBubble(): Promise<void> {
  return await invoke('plugin:sprite-pet|dismiss_bubble')
}

/** Play a choreographed action sequence. */
export async function playSequence(sequence: ActionSequence): Promise<void> {
  return await invoke('plugin:sprite-pet|play_sequence', { sequence })
}

/** Stop the current sequence. */
export async function stopSequence(): Promise<void> {
  return await invoke('plugin:sprite-pet|stop_sequence')
}

/** Update the behavior engine configuration. */
export async function setBehaviorConfig(config: BehaviorConfig): Promise<void> {
  return await invoke('plugin:sprite-pet|set_behavior_config', { config })
}

/** Enable or disable autonomous ambient behavior. */
export async function setAmbientEnabled(enabled: boolean): Promise<void> {
  return await invoke('plugin:sprite-pet|set_ambient_enabled', { enabled })
}

/** Get the current pet mood stats. */
export async function getStats(): Promise<PetStats> {
  return await invoke<PetStats>('plugin:sprite-pet|get_stats')
}

/** Override the pet's mood stats. */
export async function setStats(stats: PetStats): Promise<void> {
  return await invoke('plugin:sprite-pet|set_stats', { stats })
}

/** Manually trigger a state save to disk. */
export async function saveState(): Promise<void> {
  return await invoke('plugin:sprite-pet|save_state')
}

/** Load a previously saved pet state. */
export async function loadSavedState(petId: string): Promise<PetSnapshot> {
  return await invoke<PetSnapshot>('plugin:sprite-pet|load_saved_state', { petId })
}

/** List all previously downloaded pets (from local cache). */
export async function listDownloadedPets(): Promise<PetConfig[]> {
  return await invoke<PetConfig[]>('plugin:sprite-pet|list_downloaded_pets')
}
