import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

// ─── Types ──────────────────────────────────────────────────────────

export interface PetConfig {
  id: string
  displayName: string
  spritesheetPath: string
  spritesheetHash: string
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

export interface PositionInfo {
  x: number
  y: number
  facing: 'left' | 'right'
}

export interface PetState {
  petId: string
  action: string
  frameIndex: number
  frameRect: { x: number; y: number; width: number; height: number }
  position: { x: number; y: number }
  facing: 'left' | 'right'
  bubble: { text: string; kind: string; typingAnimation: boolean } | null
  stats: PetStats
  moodLabel: string
}

export interface ActionDef {
  name: string
  row: number
  frameCount: number
  frameDurationMs: number
  looping: boolean
  interruptible: boolean
  loopRestMs?: number
  lastFrameHoldMs?: number
}

export interface PetListResponse {
  pets: PetMeta[]
  page: number
  pageSize: number
  total: number
  totalPages: number
}

export interface MoodConfig {
  decayIntervalMs: number
  happinessDecay: number
  energyDecay: number
  socialDecay: number
  boredomIncrease: number
  interactionBoost: number
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

// ─── Query ────────────────────────────────────────────────────────────

/** Get the full current pet state (action, frame, position, facing, bubble, stats, mood). */
export async function getState(): Promise<PetState> {
  return await invoke<PetState>('plugin:sprite-pet|get_state')
}

/** Get the current pet's metadata. */
export async function getPetMeta(): Promise<PetMeta> {
  return await invoke<PetMeta>('plugin:sprite-pet|get_pet_meta')
}

/** Get the list of available actions for the current pet. */
export async function getActions(): Promise<ActionDef[]> {
  return await invoke<ActionDef[]>('plugin:sprite-pet|get_actions')
}

/** Get the current position and facing direction. */
export async function getPosition(): Promise<PositionInfo> {
  return await invoke<PositionInfo>('plugin:sprite-pet|get_position')
}

/** List pets from the remote API (paginated). */
export async function listRemotePets(page?: number, pageSize?: number): Promise<PetListResponse> {
  return await invoke<PetListResponse>('plugin:sprite-pet|list_remote_pets', { page, pageSize })
}

/** Search pets on the remote API by query string. */
export async function searchRemotePets(query: string, page?: number, pageSize?: number): Promise<PetListResponse> {
  return await invoke<PetListResponse>('plugin:sprite-pet|search_remote_pets', { query, page, pageSize })
}

// ─── Mutation ─────────────────────────────────────────────────────────

/** Delete a previously saved pet state from disk. */
export async function deleteSavedState(petId: string): Promise<void> {
  return await invoke('plugin:sprite-pet|delete_saved_state', { petId })
}

/** Clear the local cache for a specific pet or all pets. */
export async function clearCache(petId?: string): Promise<void> {
  return await invoke('plugin:sprite-pet|clear_cache', { petId })
}

/** Update the mood decay configuration at runtime. */
export async function setMoodConfig(config: MoodConfig): Promise<void> {
  return await invoke('plugin:sprite-pet|set_mood_config', { config })
}

/** Customize an event-to-action binding at runtime. */
export async function setEventBinding(eventKey: string, action: string): Promise<void> {
  return await invoke('plugin:sprite-pet|set_event_binding', { eventKey, action })
}

// ─── Rendering Utilities ─────────────────────────────────────────────

/**
 * Draw a single sprite frame onto a canvas.
 *
 * @param canvas - The target canvas element
 * @param image - The loaded spritesheet Image (from spritesheetBytes)
 * @param config - The PetConfig (from loadPet result)
 * @param action - Action name (e.g. "idle", "running_right")
 * @param frame - Frame index within the action
 * @param facing - "left" or "right"
 */
export function drawFrame(
  canvas: HTMLCanvasElement,
  image: HTMLImageElement,
  config: PetConfig,
  action: string,
  frame: number,
  facing: 'left' | 'right'
): void {
  if (!canvas || !image || !image.complete || !config) return

  const actionDef = config.actions.find(a => a.name === action)
  if (!actionDef) return

  const ctx = canvas.getContext('2d')
  if (!ctx) return

  const cellW = config.layout.cellWidth
  const cellH = config.layout.cellHeight
  const col = Math.min(frame, actionDef.frameCount - 1)
  const sx = col * cellW
  const sy = actionDef.row * cellH

  ctx.clearRect(0, 0, cellW, cellH)
  ctx.save()
  if (facing === 'left') {
    ctx.translate(cellW, 0)
    ctx.scale(-1, 1)
  }
  ctx.drawImage(image, sx, sy, cellW, cellH, 0, 0, cellW, cellH)
  ctx.restore()
}

/**
 * Create a sprite renderer that manages the spritesheet image and render loop.
 * This is a convenience wrapper around drawFrame + requestAnimationFrame.
 *
 * Usage:
 * ```ts
 * const result = await loadPet('her-os1')
 * const renderer = createSpriteRenderer(canvas, result)
 *
 * // Auto-render: listen to pet://command events
 * listen('pet://command', (e) => renderer.handleCommand(e.payload))
 *
 * // Or manually draw
 * renderer.draw('idle', 0, 'right')
 *
 * // Cleanup
 * renderer.dispose()
 * ```
 */
export function createSpriteRenderer(
  canvas: HTMLCanvasElement,
  loadResult: LoadPetResult
): SpriteRenderer {
  const config = loadResult.config
  const bytes = new Uint8Array(loadResult.spritesheetBytes)
  const blob = new Blob([bytes], { type: 'image/webp' })
  const url = URL.createObjectURL(blob)

  const image = new Image()
  let ready = false

  image.onload = () => {
    ready = true
    canvas.width = config.layout.cellWidth
    canvas.height = config.layout.cellHeight
  }
  image.src = url

  let currentAction = 'idle'
  let currentFrame = 0
  let currentFacing: 'left' | 'right' = 'right'
  let animFrameId: number | null = null

  function tick() {
    if (!ready) return
    drawFrame(canvas, image, config, currentAction, currentFrame, currentFacing)
  }

  function startLoop() {
    if (animFrameId !== null) return
    function loop() {
      tick()
      animFrameId = requestAnimationFrame(loop)
    }
    animFrameId = requestAnimationFrame(loop)
  }

  function stopLoop() {
    if (animFrameId !== null) {
      cancelAnimationFrame(animFrameId)
      animFrameId = null
    }
  }

  return {
    get config() { return config },
    get image() { return image },
    get ready() { return ready },

    draw(action: string, frame: number, facing: 'left' | 'right') {
      currentAction = action
      currentFrame = frame
      currentFacing = facing
      tick()
    },

    handleCommand(cmd: PetCommand) {
      if (cmd.type === 'render') {
        currentAction = cmd.action as string
        currentFrame = cmd.frame_index as number
        currentFacing = cmd.facing as 'left' | 'right'
      }
    },

    startLoop,
    stopLoop,

    dispose() {
      stopLoop()
      URL.revokeObjectURL(url)
    }
  }
}

export interface SpriteRenderer {
  readonly config: PetConfig
  readonly image: HTMLImageElement
  readonly ready: boolean
  draw(action: string, frame: number, facing: 'left' | 'right'): void
  handleCommand(cmd: PetCommand): void
  startLoop(): void
  stopLoop(): void
  dispose(): void
}

/**
 * High-level renderer that manages the full pet lifecycle:
 * load pet → process spritesheet → listen for render commands → draw automatically.
 *
 * Usage:
 * ```ts
 * const renderer = new PetRenderer(canvas)
 * await renderer.load('her-os1')
 *
 * // The renderer now automatically draws on pet://command events
 * // Manual control is also available:
 * await renderer.playAction('waving')
 * await renderer.say('Hello!')
 *
 * // Cleanup
 * renderer.dispose()
 * ```
 */
export class PetRenderer {
  private canvas: HTMLCanvasElement
  private image: HTMLImageElement | null = null
  private config: PetConfig | null = null
  private imageUrl: string | null = null
  private unlisten: (() => void) | null = null
  private statsInterval: ReturnType<typeof setInterval> | null = null
  private bubbleTimeout: ReturnType<typeof setTimeout> | null = null

  /** Current action name */
  action = 'idle'
  /** Current frame index */
  frame = 0
  /** Current facing direction */
  facing: 'left' | 'right' = 'right'
  /** Current bubble text (null if no bubble) */
  bubble: string | null = null
  /** Current mood stats */
  stats: PetStats = { happiness: 70, energy: 80, social: 50, boredom: 30 }
  /** Computed mood label from stats */
  moodLabel = 'neutral'
  /** Whether a pet is loaded and ready */
  ready = false

  /** Callback fired on every render event, after drawing */
  onRender: ((action: string, frame: number, facing: 'left' | 'right') => void) | null = null
  /** Callback fired when a bubble is shown */
  onBubble: ((text: string, kind: string) => void) | null = null
  /** Callback fired when the bubble is dismissed */
  onBubbleDismiss: (() => void) | null = null
  /** Callback fired when stats are updated */
  onStats: ((stats: PetStats, moodLabel: string) => void) | null = null

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas
  }

  /**
   * Load a pet and start automatic rendering.
   *
   * Can be called in two ways:
   * - `renderer.load('her-os1')` — calls loadPet internally
   * - `renderer.load(loadPetResult)` — uses an existing LoadPetResult (avoids duplicate loadPet call)
   */
  async load(petIdOrResult: string | LoadPetResult, apiBaseUrl?: string): Promise<LoadPetResult> {
    this.dispose()

    const result: LoadPetResult = typeof petIdOrResult === 'string'
      ? await loadPet(petIdOrResult, apiBaseUrl)
      : petIdOrResult

    await this.applyResult(result)
    return result
  }

  /** Apply a LoadPetResult: load image, subscribe events, start polling. */
  private async applyResult(result: LoadPetResult): Promise<void> {
    this.config = result.config

    // Update canvas size to match sprite cell
    this.canvas.width = result.config.layout.cellWidth
    this.canvas.height = result.config.layout.cellHeight

    // Load spritesheet image from bytes
    const bytes = new Uint8Array(result.spritesheetBytes)
    const blob = new Blob([bytes])
    this.imageUrl = URL.createObjectURL(blob)

    await new Promise<void>((resolve, reject) => {
      const img = new Image()
      img.onload = () => {
        this.image = img
        this.ready = true
        this.draw('idle', 0, 'right')
        resolve()
      }
      img.onerror = () => reject(new Error('Failed to load spritesheet image'))
      img.src = this.imageUrl!
    })

    // Subscribe to pet://command events
    this.unlisten = await listen('pet://command', (event) => {
      this.handleCommand(event.payload as PetCommand)
    })

    // Start stats polling
    this.startStatsPolling()
  }

  /** Handle a pet://command event payload. */
  handleCommand(cmd: PetCommand): void {
    switch (cmd.type) {
      case 'render':
        this.action = cmd.action as string
        this.frame = cmd.frame_index as number
        this.facing = cmd.facing as 'left' | 'right'
        this.draw(this.action, this.frame, this.facing)
        this.onRender?.(this.action, this.frame, this.facing)
        break
      case 'bubble':
        this.showBubbleUI(cmd.text as string, cmd.kind as string, cmd.duration_ms as number)
        break
      case 'dismiss_bubble':
        this.clearBubble()
        this.onBubbleDismiss?.()
        break
      case 'audio':
        this.playAudio(cmd.audio_bytes as number[], cmd.format as string)
        break
    }
  }

  /** Draw a specific frame onto the canvas. */
  draw(action: string, frame: number, facing: 'left' | 'right'): void {
    drawFrame(this.canvas, this.image!, this.config!, action, frame, facing)
  }

  /** Play an action animation. */
  async playAction(action: string, loops?: number): Promise<void> {
    await playAction(action, loops)
  }

  /** Show a speech bubble. */
  async say(text: string, kind?: string): Promise<void> {
    await say(text, kind)
  }

  /** Show a bubble with full control. */
  async showBubble(text: string, kind?: string, durationMs?: number, typing?: boolean): Promise<void> {
    await showBubble(text, kind, durationMs, typing)
  }

  /** Dismiss the current bubble. */
  async dismissBubble(): Promise<void> {
    await dismissBubble()
  }

  /** Enable or disable ambient behavior. */
  async setAmbientEnabled(enabled: boolean): Promise<void> {
    await setAmbientEnabled(enabled)
  }

  /** Play a choreographed sequence. */
  async playSequence(sequence: ActionSequence): Promise<void> {
    await playSequence(sequence)
  }

  /** Get the loaded pet config. Returns null if no pet is loaded. */
  getConfig(): PetConfig | null {
    return this.config
  }

  /** Get the loaded spritesheet Image element. Returns null if no pet is loaded. */
  getImage(): HTMLImageElement | null {
    return this.image
  }

  /** Clean up all resources: unsubscribe events, revoke URLs, stop polling. */
  dispose(): void {
    this.unlisten?.()
    this.unlisten = null
    this.stopStatsPolling()
    this.clearBubble()
    if (this.imageUrl) {
      URL.revokeObjectURL(this.imageUrl)
      this.imageUrl = null
    }
    this.image = null
    this.config = null
    this.ready = false
    this.action = 'idle'
    this.frame = 0
    this.facing = 'right'
    this.bubble = null
  }

  private showBubbleUI(text: string, kind: string, durationMs: number): void {
    this.clearBubble()
    this.bubble = text
    this.onBubble?.(text, kind)
    if (durationMs > 0) {
      this.bubbleTimeout = setTimeout(() => {
        this.bubble = null
        this.onBubbleDismiss?.()
      }, durationMs)
    }
  }

  private clearBubble(): void {
    if (this.bubbleTimeout) {
      clearTimeout(this.bubbleTimeout)
      this.bubbleTimeout = null
    }
    this.bubble = null
  }

  private playAudio(audioBytes: number[], format: string): void {
    try {
      const mime = format === 'mp3' ? 'audio/mpeg' : format === 'ogg' ? 'audio/ogg' : 'audio/wav'
      const bytes = new Uint8Array(audioBytes)
      const blob = new Blob([bytes], { type: mime })
      const url = URL.createObjectURL(blob)
      const audio = new Audio(url)
      audio.play().catch(() => {})
      audio.onended = () => URL.revokeObjectURL(url)
    } catch (_) {}
  }

  private startStatsPolling(): void {
    this.stopStatsPolling()
    this.statsInterval = setInterval(async () => {
      try {
        const s = await getStats()
        this.stats = s
        const score = s.happiness * 0.4 + s.energy * 0.2 + s.social * 0.3 - s.boredom * 0.1
        if (score >= 80) this.moodLabel = 'ecstatic'
        else if (score >= 60) this.moodLabel = 'happy'
        else if (score >= 40) this.moodLabel = 'neutral'
        else if (score >= 20) this.moodLabel = 'sad'
        else this.moodLabel = 'depressed'
        this.onStats?.(this.stats, this.moodLabel)
      } catch (_) {}
    }, 2000)
  }

  private stopStatsPolling(): void {
    if (this.statsInterval) {
      clearInterval(this.statsInterval)
      this.statsInterval = null
    }
  }
}
