<script>
  import { listen } from '@tauri-apps/api/event'
  import {
    loadPet, unloadPet, triggerEvent, setPosition,
    playAction, say, showBubble, dismissBubble,
    setAmbientEnabled, getStats, playSequence,
    listDownloadedPets,
  } from 'tauri-plugin-sprite-pet-api'

  // ─── State ────────────────────────────────────────────────────

  let petIdInput = $state('')
  let apiProvider = $state('https://codex-pets.net')
  let loading = $state(false)
  let loaded = $state(false)
  let petConfig = $state(null)
  let error = $state('')

  let spritesheetUrl = $state('')
  let currentAction = $state('idle')
  let currentFrame = $state(0)
  let facingLeft = $state(false)

  let bubble = $state(null)
  let bubbleTimeout = null

  let stats = $state({ happiness: 70, energy: 80, social: 50, boredom: 30 })
  let moodLabel = $state('neutral')
  let ambientEnabled = $state(true)

  let bubbleText = $state('Hello!')
  let logs = $state([])
  let statsInterval = null

  let downloadedPets = $state([])

  // ─── Canvas Rendering ────────────────────────────────────────

  let canvasEl = $state(null)
  let spriteImage = null
  let configRef = null
  let renderAction = 'idle'
  let renderFrame = 0
  let renderFacing = 'right'
  let rendering = false

  function startRenderLoop() {
    if (rendering) return
    rendering = true
    function loop() {
      if (!rendering) return
      drawFrame(renderAction, renderFrame, renderFacing)
      requestAnimationFrame(loop)
    }
    requestAnimationFrame(loop)
  }

  function stopRenderLoop() {
    rendering = false
  }

  function drawFrame(action, frame, facing) {
    const canvas = canvasEl
    const img = spriteImage
    const config = configRef
    if (!canvas || !img || !img.complete || !config) return

    const actionDef = config.actions.find(a => a.name === action)
    if (!actionDef) return

    const ctx = canvas.getContext('2d')
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
    ctx.drawImage(img, sx, sy, cellW, cellH, 0, 0, cellW, cellH)
    ctx.restore()
  }

  // ─── Init: load downloaded pets list ──────────────────────────

  async function refreshDownloadedPets() {
    try {
      downloadedPets = await listDownloadedPets()
    } catch (_) {}
  }

  refreshDownloadedPets()

  // ─── Event Listener ───────────────────────────────────────────

  let unlistenFn = null
  let lastRenderKey = ''

  async function setupListener() {
    if (unlistenFn) return
    unlistenFn = await listen('pet://command', (event) => {
      const cmd = event.payload
      logEvent(cmd)

      try {
        switch (cmd.type) {
          case 'render': {
            const key = `${cmd.action}:${cmd.frame_index}:${cmd.facing}`
            if (key === lastRenderKey) break
            lastRenderKey = key
            currentAction = cmd.action
            currentFrame = cmd.frame_index
            facingLeft = cmd.facing === 'left'
            renderAction = cmd.action
            renderFrame = cmd.frame_index
            renderFacing = cmd.facing
            break
          }
          case 'bubble':
            showBubbleUI(cmd.text, cmd.kind, cmd.duration_ms)
            break
          case 'dismiss_bubble':
            clearBubble()
            break
          case 'audio':
            playAudio(cmd.audio_bytes, cmd.format)
            break
          case 'action_finished':
            // Non-looping action completed - last frame is being held
            break
        }
      } catch (e) {
        console.error('Error handling pet command:', cmd.type, e)
      }
    })
  }

  setupListener()

  // ─── Bubble ───────────────────────────────────────────────────

  function showBubbleUI(text, kind, durationMs) {
    clearBubble()
    bubble = { text, kind: kind || 'speech' }
    if (durationMs > 0) {
      bubbleTimeout = setTimeout(() => { bubble = null }, durationMs)
    }
  }

  function clearBubble() {
    if (bubbleTimeout) {
      clearTimeout(bubbleTimeout)
      bubbleTimeout = null
    }
    bubble = null
  }

  // ─── Audio ────────────────────────────────────────────────────

  function playAudio(audioBytes, format) {
    try {
      const mime = format === 'mp3' ? 'audio/mpeg' : format === 'ogg' ? 'audio/ogg' : 'audio/wav'
      const bytes = new Uint8Array(audioBytes)
      const blob = new Blob([bytes], { type: mime })
      const url = URL.createObjectURL(blob)
      const audio = new Audio(url)
      audio.play().catch(() => {})
      audio.onended = () => URL.revokeObjectURL(url)
    } catch (e) {
      console.warn('Audio playback failed:', e)
    }
  }

  // ─── Logging ──────────────────────────────────────────────────

  function logEvent(cmd) {
    const time = new Date().toLocaleTimeString()
    let detail = ''
    if (cmd.type === 'render') detail = `${cmd.action} frame ${cmd.frame_index} ${cmd.facing}`
    else if (cmd.type === 'bubble') detail = `${cmd.kind}: "${cmd.text}"`
    else if (cmd.type === 'audio') detail = `${cmd.format} ${cmd.audio_bytes?.length || 0}B`
    else detail = JSON.stringify(cmd).slice(0, 60)

    logs = [{ time, type: cmd.type, detail }, ...logs].slice(0, 50)
  }

  // ─── Actions ──────────────────────────────────────────────────

  async function handleLoad(id) {
    const petId = (id || petIdInput).trim()
    if (!petId) return
    loading = true
    error = ''
    try {
      const result = await loadPet(petId, apiProvider)
      const config = result.config
      petConfig = config
      configRef = config

      const bytes = new Uint8Array(result.spritesheetBytes)
      const blob = new Blob([bytes], { type: 'image/webp' })
      spritesheetUrl = URL.createObjectURL(blob)

      const img = new Image()
      img.onload = () => {
        spriteImage = img
        renderAction = 'idle'
        renderFrame = 0
        renderFacing = 'right'
        startRenderLoop()
      }
      img.src = spritesheetUrl

      loaded = true
      currentAction = 'idle'
      currentFrame = 0
      refreshDownloadedPets()
      startStatsPolling()
    } catch (e) {
      const msg = typeof e === 'string' ? e : e?.message || JSON.stringify(e)
      error = msg
    } finally {
      loading = false
    }
  }

  async function handleUnload() {
    try {
      await unloadPet()
    } catch (_) {}
    loaded = false
    petConfig = null
    configRef = null
    spriteImage = null
    stopRenderLoop()
    if (spritesheetUrl) {
      URL.revokeObjectURL(spritesheetUrl)
    }
    spritesheetUrl = ''
    error = ''
    currentAction = 'idle'
    currentFrame = 0
    facingLeft = false
    clearBubble()
    stopStatsPolling()
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
          { action: 'click', waitForComplete: true, delayMs: 0 },
          { action: 'special', waitForComplete: true, delayMs: 200 },
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
  }

  // ─── Stats Polling ────────────────────────────────────────────

  function startStatsPolling() {
    stopStatsPolling()
    statsInterval = setInterval(async () => {
      try {
        const s = await getStats()
        stats = s
        const score = s.happiness * 0.4 + s.energy * 0.2 + s.social * 0.3 - s.boredom * 0.1
        if (score >= 80) moodLabel = 'ecstatic'
        else if (score >= 60) moodLabel = 'happy'
        else if (score >= 40) moodLabel = 'neutral'
        else if (score >= 20) moodLabel = 'sad'
        else moodLabel = 'depressed'
      } catch (_) {}
    }, 2000)
  }

  function stopStatsPolling() {
    if (statsInterval) {
      clearInterval(statsInterval)
      statsInterval = null
    }
  }

  // ─── Keyboard Shortcuts ───────────────────────────────────────

  function onKeydown(e) {
    if (e.target.tagName === 'INPUT') return
    if (!loaded) return
    if (e.key === '1') handlePlayAction('idle')
    else if (e.key === '2') handlePlayAction('walk')
    else if (e.key === '3') handlePlayAction('click')
    else if (e.key === '4') handlePlayAction('special')
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
        <div class="bubble {bubble.kind}">
          {bubble.text}
        </div>
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
        <button class="btn" onclick={() => handlePlayAction('walk')}>Walk</button>
        <button class="btn" onclick={() => handlePlayAction('click')}>Click</button>
        <button class="btn" onclick={() => handlePlayAction('special')}>Special</button>
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
