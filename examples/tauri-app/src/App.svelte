<script>
  import { tick } from 'svelte'
  import {
    PetRenderer, loadPet, triggerEvent,
    playAction, say, showBubble, dismissBubble,
    setAmbientEnabled, playSequence,
    listDownloadedPets,
  } from 'tauri-plugin-sprite-pet-api'

  // ─── State ────────────────────────────────────────────────────

  let petIdInput = $state('')
  let apiProvider = $state('https://codex-pets.net')
  let loading = $state(false)
  let loaded = $state(false)
  let petConfig = $state(null)
  let error = $state('')

  let currentAction = $state('idle')
  let currentFrame = $state(0)
  let facingLeft = $state(false)

  let bubble = $state(null)

  let stats = $state({ happiness: 70, energy: 80, social: 50, boredom: 30 })
  let moodLabel = $state('neutral')
  let ambientEnabled = $state(true)

  let bubbleText = $state('Hello!')
  let logs = $state([])

  let downloadedPets = $state([])

  // ─── Canvas & Renderer ────────────────────────────────────────

  let canvasEl = $state(null)
  let renderer = null

  // ─── Init ─────────────────────────────────────────────────────

  async function refreshDownloadedPets() {
    try { downloadedPets = await listDownloadedPets() } catch (_) {}
  }
  refreshDownloadedPets()

  // ─── Logging Helper ───────────────────────────────────────────

  function logEvent(type, detail) {
    const time = new Date().toLocaleTimeString()
    logs = [{ time, type, detail }, ...logs].slice(0, 50)
  }

  // ─── Actions ──────────────────────────────────────────────────

  async function handleLoad(id) {
    const petId = (id || petIdInput).trim()
    if (!petId) return
    loading = true
    error = ''
    try {
      // 1) Backend: download spritesheet, validate, detect frames, start runtime
      const result = await loadPet(petId, apiProvider)
      petConfig = result.config
      loaded = true

      // 2) Wait for Svelte to render the canvas element
      await tick()

      // 3) Frontend: create renderer — loads image, subscribes to pet://command events
      renderer = new PetRenderer(canvasEl)

      // 4) Bind callbacks: renderer receives backend events and updates UI state
      renderer.onRender = (action, frame, facing) => {
        currentAction = action
        currentFrame = frame
        facingLeft = facing === 'left'
        logEvent('render', `${action} frame ${frame} ${facing}`)
      }
      renderer.onBubble = (text, kind) => {
        bubble = { text, kind }
        logEvent('bubble', `${kind}: "${text}"`)
      }
      renderer.onBubbleDismiss = () => {
        bubble = null
        logEvent('dismiss_bubble', '')
      }
      renderer.onStats = (s, mood) => {
        stats = s
        moodLabel = mood
      }

      // 5) Apply the loaded result: image → events → polling
      await renderer.load(result)
      refreshDownloadedPets()
    } catch (e) {
      error = typeof e === 'string' ? e : e?.message || JSON.stringify(e)
    } finally {
      loading = false
    }
  }

  async function handleUnload() {
    renderer?.dispose()
    renderer = null
    loaded = false
    petConfig = null
    error = ''
    currentAction = 'idle'
    currentFrame = 0
    facingLeft = false
    bubble = null
  }

  async function handlePlayAction(action) {
    try { await playAction(action, 1) } catch (_) {}
  }

  async function handleSay() {
    if (!bubbleText.trim()) return
    try { await say(bubbleText.trim()) } catch (_) {}
  }

  async function handleShowBubble(kind) {
    if (!bubbleText.trim()) return
    try { await showBubble(bubbleText.trim(), kind, 3000, true) } catch (_) {}
  }

  async function handleDismissBubble() {
    try { await dismissBubble() } catch (_) {}
  }

  async function handleToggleAmbient() {
    ambientEnabled = !ambientEnabled
    try { await setAmbientEnabled(ambientEnabled) } catch (_) {}
  }

  async function handlePlaySequence() {
    try {
      await playSequence({
        steps: [
          { action: 'waving', waitForComplete: true, delayMs: 0 },
          { action: 'jumping', waitForComplete: true, delayMs: 200 },
          { action: 'idle', waitForComplete: false, delayMs: 0 },
        ],
        repeat: { type: 'once' },
        onComplete: 'idle',
      })
    } catch (_) {}
  }

  // ─── Drag ─────────────────────────────────────────────────────

  let dragging = $state(false)

  function onPointerDown(e) {
    dragging = true
    try { triggerEvent({ type: 'drag_start' }) } catch (_) {}
    logEvent('event', 'drag_start')
    e.target.setPointerCapture(e.pointerId)
  }

  function onPointerMove(e) {
    if (!dragging) return
    try { triggerEvent({ type: 'drag_move', data: { x: e.clientX, y: e.clientY } }) } catch (_) {}
  }

  function onPointerUp() {
    if (!dragging) return
    dragging = false
    try { triggerEvent({ type: 'drag_drop' }) } catch (_) {}
    logEvent('event', 'drag_drop')
  }

  // ─── Keyboard Shortcuts ───────────────────────────────────────

  function onKeydown(e) {
    if (e.target.tagName === 'INPUT') return
    if (!loaded) return
    if (e.key === '1') handlePlayAction('idle')
    else if (e.key === '2') handlePlayAction('running_right')
    else if (e.key === '3') handlePlayAction('waving')
    else if (e.key === '4') handlePlayAction('jumping')
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="app">
  <!-- Header -->
  <div class="header">
    <select bind:value={apiProvider} disabled={loading || loaded}>
      <option value="https://codex-pets.net">codex-pets.net</option>
      <option value="https://codexpet.xyz">codexpet.xyz</option>
    </select>
    <input
      type="text"
      placeholder="Pet ID (e.g. her-os1, endminguga)"
      bind:value={petIdInput}
      onkeydown={(e) => e.key === 'Enter' && handleLoad()}
      disabled={loading || loaded}
    />
    {#if !loaded}
      <button class="btn primary" onclick={() => handleLoad()} disabled={loading || !petIdInput.trim()}>
        {loading ? 'Loading...' : 'Load Pet'}
      </button>
    {:else}
      <button class="btn danger" onclick={handleUnload}>Unload</button>
    {/if}
    <span class="status">
      {#if loaded}{petConfig?.displayName || petConfig?.id}{:else if loading}Loading...{:else}No pet loaded{/if}
    </span>
  </div>

  {#if error}
    <div class="error-bar">{error}</div>
  {/if}

  <!-- Downloaded Pets Picker -->
  {#if !loaded && downloadedPets.length > 0}
    <div class="pet-picker">
      <span class="picker-label">Downloaded:</span>
      {#each downloadedPets as pet}
        <button class="btn pet-chip" onclick={() => handleLoad(pet.id)}>
          {pet.displayName || pet.id}
        </button>
      {/each}
    </div>
  {/if}

  <!-- Pet Display -->
  <div class="pet-area" class:empty={!loaded}>
    {#if loaded}
      {#if bubble}
        <div class="bubble {bubble.kind}">{bubble.text}</div>
      {/if}
      <!-- svelte-ignore a11y_no_interactive_element_to_noninteractive_role -->
      <canvas
        bind:this={canvasEl}
        class="pet-canvas"
        width={petConfig?.layout.cellWidth ?? 192}
        height={petConfig?.layout.cellHeight ?? 208}
        onpointerdown={onPointerDown}
        onpointermove={onPointerMove}
        onpointerup={onPointerUp}
        role="img"
        aria-label="pet sprite"
      ></canvas>
    {:else}
      <span>Load a pet to begin</span>
    {/if}
  </div>

  <!-- Controls -->
  {#if loaded}
    <div class="controls">
      <div class="control-group">
        <button class="btn" onclick={() => handlePlayAction('idle')}>Idle</button>
        <button class="btn" onclick={() => handlePlayAction('running_right')}>Run</button>
        <button class="btn" onclick={() => handlePlayAction('waving')}>Wave</button>
        <button class="btn" onclick={() => handlePlayAction('jumping')}>Jump</button>
        <button class="btn" onclick={handlePlaySequence}>Sequence</button>
      </div>
      <div class="divider"></div>
      <div class="bubble-input">
        <input type="text" placeholder="Say something..." bind:value={bubbleText} onkeydown={(e) => e.key === 'Enter' && handleSay()} />
        <button class="btn" onclick={handleSay}>Say</button>
        <button class="btn" onclick={() => handleShowBubble('thought')}>Think</button>
        <button class="btn" onclick={handleDismissBubble}>Dismiss</button>
      </div>
      <div class="divider"></div>
      <button class="btn" class:active={ambientEnabled} onclick={handleToggleAmbient}>
        Ambient: {ambientEnabled ? 'ON' : 'OFF'}
      </button>
    </div>
  {/if}

  <!-- Mood Panel -->
  {#if loaded}
    <div class="mood-panel">
      <div class="mood-stat">
        <span class="mood-stat-label">Happiness</span>
        <div class="mood-bar"><div class="mood-bar-fill happiness" style="width: {stats.happiness}%"></div></div>
      </div>
      <div class="mood-stat">
        <span class="mood-stat-label">Energy</span>
        <div class="mood-bar"><div class="mood-bar-fill energy" style="width: {stats.energy}%"></div></div>
      </div>
      <div class="mood-stat">
        <span class="mood-stat-label">Social</span>
        <div class="mood-bar"><div class="mood-bar-fill social" style="width: {stats.social}%"></div></div>
      </div>
      <div class="mood-stat">
        <span class="mood-stat-label">Boredom</span>
        <div class="mood-bar"><div class="mood-bar-fill boredom" style="width: {stats.boredom}%"></div></div>
      </div>
      <div class="mood-label">{moodLabel}</div>
    </div>
  {/if}

  <!-- Event Log -->
  <div class="event-log">
    {#if logs.length === 0}
      <div class="log-entry"><span class="time">--:--:--</span> <span class="cmd-detail">Waiting for events...</span></div>
    {/if}
    {#each logs as entry}
      <div class="log-entry">
        <span class="time">{entry.time}</span>
        <span class="cmd-type">{entry.type}</span>
        <span class="cmd-detail">{entry.detail}</span>
      </div>
    {/each}
  </div>
</div>
