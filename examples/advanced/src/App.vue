<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { drawFrame } from "tauri-plugin-sprite-pet-api";
import type { PetConfig, LoadPetResult }  from "tauri-plugin-sprite-pet-api";

// interface PetConfig {
//   id: string;
//   displayName: string;
//   spritesheetPath: string;
//   layout: { columns: number; rows: number; cellWidth: number; cellHeight: number };
//   actions: Array<{ name: string; row: number; frameCount: number; frameDurationMs: number; looping: boolean; interruptible: boolean }>;
// }

// interface LoadResult {
//   config: PetConfig;
//   spritesheet_bytes: number[];
// }

// ─── State ──────────────────────────────────────────────────────

const petId = ref("");
const apiUrl = ref("https://codex-pets.net");
const loading = ref(false);
const loaded = ref(false);
const petConfig = ref<PetConfig | null>(null);
const error = ref("");

const currentAction = ref("idle");
const currentFrame = ref(0);
const facingLeft = ref(false);

const bubbleText = ref("Hello!");
const bubble = ref<{ text: string; kind: string } | null>(null);
const ambientEnabled = ref(true);
const logs = ref<{ time: string; type: string; detail: string }[]>([]);

// ─── Canvas ─────────────────────────────────────────────────────

const canvasRef = ref<HTMLCanvasElement | null>(null);
let spriteImage: HTMLImageElement | null = null;
let configRef: PetConfig | null = null;

// ─── Event Listeners ────────────────────────────────────────────

const unlisteners: UnlistenFn[] = [];

onMounted(async () => {
  // Listen for render commands from backend
  unlisteners.push(
    await listen("pet://command", (event) => {
      const cmd = event.payload as any;
      logEvent(cmd);

      if (cmd.type === "render") {
        currentAction.value = cmd.action;
        currentFrame.value = cmd.frame_index;
        facingLeft.value = cmd.facing === "left";

        if (canvasRef.value && spriteImage && configRef) {
          drawFrame(
            canvasRef.value,
            spriteImage,
            configRef,
            cmd.action,
            cmd.frame_index,
            cmd.facing
          );
        }
      } else if (cmd.type === "bubble") {
        bubble.value = { text: cmd.text, kind: cmd.kind };
      } else if (cmd.type === "dismiss_bubble") {
        bubble.value = null;
      }
    })
  );
});

onUnmounted(() => {
  unlisteners.forEach((fn) => fn());
});

// ─── Logging ────────────────────────────────────────────────────

function logEvent(cmd: any) {
  const time = new Date().toLocaleTimeString();
  let detail = "";
  if (cmd.type === "render")
    detail = `${cmd.action} frame ${cmd.frame_index} ${cmd.facing}`;
  else if (cmd.type === "bubble") detail = `${cmd.kind}: "${cmd.text}"`;
  else detail = JSON.stringify(cmd).slice(0, 60);
  logs.value = [{ time, type: cmd.type, detail }, ...logs.value].slice(0, 50);
}

// ─── Actions ────────────────────────────────────────────────────

async function handleLoad() {
  const id = petId.value.trim();
  if (!id) return;
  loading.value = true;
  error.value = "";
  try {
    // 1) Backend: Pet::start() downloads, validates, starts runtime, returns config + bytes
    const result = await invoke<LoadPetResult>("load_pet", {
      petId: id,
      apiUrl: apiUrl.value,
    });
    petConfig.value = result.config;
    configRef = result.config;
    loaded.value = true;

    // 2) Wait for canvas to render
    await nextTick();

    // 3) Load spritesheet image from bytes returned by backend
    const blob = new Blob([new Uint8Array(result.spritesheetBytes)]);
    const blobUrl = URL.createObjectURL(blob);

    await new Promise<void>((resolve, reject) => {
      const img = new Image();
      img.onload = () => {
        spriteImage = img;
        if (canvasRef.value && configRef) {
          drawFrame(canvasRef.value, img, configRef, "idle", 0, "right");
        }
        resolve();
      };
      img.onerror = () => reject(new Error("Failed to load spritesheet image"));
      img.src = blobUrl;
    });
  } catch (e: any) {
    error.value = typeof e === "string" ? e : e?.message || JSON.stringify(e);
  } finally {
    loading.value = false;
  }
}

async function handleUnload() {
  try {
    await invoke("unload_pet");
  } catch (_) {}
  loaded.value = false;
  petConfig.value = null;
  configRef = null;
  spriteImage = null;
  bubble.value = null;
  currentAction.value = "idle";
  currentFrame.value = 0;
  facingLeft.value = false;
}

async function handlePlay(action: string) {
  try {
    await invoke("play_action", { action });
  } catch (_) {}
}

async function handleSay() {
  if (!bubbleText.value.trim()) return;
  try {
    await invoke("say", { text: bubbleText.value.trim() });
  } catch (_) {}
}

async function handleThink() {
  if (!bubbleText.value.trim()) return;
  try {
    await invoke("think", { text: bubbleText.value.trim() });
  } catch (_) {}
}

async function handleDismiss() {
  try {
    await invoke("dismiss_bubble");
  } catch (_) {}
}

async function handleToggleAmbient() {
  ambientEnabled.value = !ambientEnabled.value;
  try {
    await invoke("toggle_ambient", { enabled: ambientEnabled.value });
  } catch (_) {}
}

// ─── Drag ───────────────────────────────────────────────────────

let dragging = false;

function onPointerDown(e: PointerEvent) {
  dragging = true;
  invoke("trigger_drag", { eventType: "drag_start" }).catch(() => {});
  (e.target as HTMLElement).setPointerCapture(e.pointerId);
}

function onPointerMove(e: PointerEvent) {
  if (!dragging) return;
  invoke("trigger_drag", {
    eventType: "drag_move",
    x: e.clientX,
    y: e.clientY,
  }).catch(() => {});
}

function onPointerUp() {
  if (!dragging) return;
  dragging = false;
  invoke("trigger_drag", { eventType: "drag_drop" }).catch(() => {});
}
</script>

<template>
  <div class="app">
    <!-- Header -->
    <div class="header">
      <select v-model="apiUrl" :disabled="loading || loaded">
        <option value="https://codex-pets.net">codex-pets.net</option>
        <option value="https://codexpet.xyz">codexpet.xyz</option>
      </select>
      <input
        v-model="petId"
        placeholder="Pet ID (e.g. her-os1, endminguga)"
        @keydown.enter="handleLoad"
        :disabled="loading || loaded"
      />
      <button
        v-if="!loaded"
        class="btn primary"
        @click="handleLoad"
        :disabled="loading || !petId.trim()"
      >
        {{ loading ? "Loading..." : "Load Pet" }}
      </button>
      <button v-else class="btn danger" @click="handleUnload">Unload</button>
      <span class="status">
        {{
          loaded
            ? petConfig?.displayName || petConfig?.id
            : loading
            ? "Loading..."
            : "No pet loaded"
        }}
      </span>
    </div>

    <div v-if="error" class="error-bar">{{ error }}</div>

    <!-- Pet Display -->
    <div class="pet-area" :class="{ empty: !loaded }">
      <template v-if="loaded">
        <div v-if="bubble" class="bubble" :class="bubble.kind">
          {{ bubble.text }}
        </div>
        <canvas
          ref="canvasRef"
          class="pet-canvas"
          :width="petConfig?.layout.cellWidth ?? 192"
          :height="petConfig?.layout.cellHeight ?? 208"
          @pointerdown="onPointerDown"
          @pointermove="onPointerMove"
          @pointerup="onPointerUp"
        />
      </template>
      <span v-else>Load a pet to begin</span>
    </div>

    <!-- Controls -->
    <div v-if="loaded" class="controls">
      <div class="control-group">
        <button class="btn" @click="handlePlay('idle')">Idle</button>
        <button class="btn" @click="handlePlay('running_right')">Run</button>
        <button class="btn" @click="handlePlay('waving')">Wave</button>
        <button class="btn" @click="handlePlay('jumping')">Jump</button>
      </div>
      <div class="divider"></div>
      <div class="bubble-input">
        <input
          v-model="bubbleText"
          placeholder="Say something..."
          @keydown.enter="handleSay"
        />
        <button class="btn" @click="handleSay">Say</button>
        <button class="btn" @click="handleThink">Think</button>
        <button class="btn" @click="handleDismiss">Dismiss</button>
      </div>
      <div class="divider"></div>
      <button
        class="btn"
        :class="{ active: ambientEnabled }"
        @click="handleToggleAmbient"
      >
        Ambient: {{ ambientEnabled ? "ON" : "OFF" }}
      </button>
    </div>

    <!-- Event Log -->
    <div class="event-log">
      <div v-if="logs.length === 0" class="log-entry">
        <span class="time">--:--:--</span>
        <span class="detail">Waiting for events...</span>
      </div>
      <div v-for="entry in logs" :key="entry.time + entry.detail" class="log-entry">
        <span class="time">{{ entry.time }}</span>
        <span class="type">{{ entry.type }}</span>
        <span class="detail">{{ entry.detail }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  padding: 12px;
  gap: 12px;
  font-family: system-ui, -apple-system, sans-serif;
  font-size: 14px;
}

.header {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.header select,
.header input {
  padding: 6px 10px;
  border: 1px solid #ccc;
  border-radius: 6px;
  font-size: 13px;
}

.header input {
  flex: 1;
  min-width: 200px;
}

.status {
  font-weight: 500;
  color: #555;
}

.btn {
  padding: 6px 14px;
  border: 1px solid #ccc;
  border-radius: 6px;
  background: #fff;
  cursor: pointer;
  font-size: 13px;
  transition: all 0.15s;
}

.btn:hover {
  border-color: #396cd8;
  background: #f0f4ff;
}

.btn.primary {
  background: #396cd8;
  color: white;
  border-color: #396cd8;
}

.btn.primary:hover {
  background: #2a5bc7;
}

.btn.danger {
  background: #e74c3c;
  color: white;
  border-color: #e74c3c;
}

.btn.active {
  background: #27ae60;
  color: white;
  border-color: #27ae60;
}

.error-bar {
  padding: 8px 12px;
  background: #fee;
  border: 1px solid #e74c3c;
  border-radius: 6px;
  color: #c0392b;
  font-size: 13px;
}

.pet-area {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 220px;
  background: #f8f8f8;
  border: 1px solid #eee;
  border-radius: 8px;
  position: relative;
}

.pet-area.empty {
  color: #999;
}

.pet-canvas {
  image-rendering: pixelated;
}

.bubble {
  position: absolute;
  top: 10px;
  left: 50%;
  transform: translateX(-50%);
  padding: 6px 14px;
  border-radius: 12px;
  font-size: 13px;
  max-width: 260px;
  text-align: center;
}

.bubble.speech {
  background: #fff;
  border: 1px solid #ddd;
}

.bubble.thought {
  background: #e8f4fd;
  border: 1px solid #b3d9f2;
  font-style: italic;
}

.controls {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.control-group {
  display: flex;
  gap: 4px;
}

.bubble-input {
  display: flex;
  gap: 4px;
  flex: 1;
}

.bubble-input input {
  flex: 1;
  padding: 6px 10px;
  border: 1px solid #ccc;
  border-radius: 6px;
  font-size: 13px;
}

.divider {
  width: 1px;
  height: 24px;
  background: #ddd;
}

.event-log {
  flex: 1;
  overflow-y: auto;
  background: #1e1e1e;
  color: #d4d4d4;
  padding: 8px;
  border-radius: 6px;
  font-family: "Cascadia Code", "Fira Code", monospace;
  font-size: 12px;
  line-height: 1.6;
}

.log-entry {
  display: flex;
  gap: 8px;
}

.time {
  color: #6a9955;
  min-width: 70px;
}

.type {
  color: #569cd6;
  min-width: 90px;
}

.detail {
  color: #d4d4d4;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
