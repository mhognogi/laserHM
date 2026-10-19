<script setup lang="ts">
import { computed, onUnmounted, reactive, ref, watch } from "vue";
import { House, Pause, Play, Route, Square, X } from "lucide-vue-next";
import {
  call,
  cutProgress,
  jobProgress,
  setCheckBeforeStart,
  shapeData,
  simulate,
  start,
  status,
  type JobSettings,
  type ShapeData,
} from "../laser";
import { editor, selectedShapes } from "../editor";
import { rasterData } from "../images";

// Button rules:
//   not connected                -> all disabled
//   connected, nothing selected  -> Home, "No object selected"
//   connected, objects selected  -> Start, Home, Simulate, "Click to start"
//   cut or simulation running    -> Pause, Stop
//   cut or simulation paused     -> Start (resumes), Stop
//   homing, moving the head, Fire -> Stop
const hasSelection = computed(() => editor.selectedIds.length > 0);

// ----- Job progress -----

const percent = computed(() => Math.min(100, Math.floor((jobProgress.value?.fraction ?? 0) * 100)));

/** Seconds the job has cut, from its first move; a pause does not count. */
const elapsed = ref(0);
let lastTick = Date.now();
const ticker = setInterval(() => {
  const now = Date.now();
  const cutting = status.busy && !status.paused && (status.task === "cut" || status.task === "simulate");
  if (cutting && jobProgress.value) elapsed.value += (now - lastTick) / 1000;
  lastTick = now;
}, 1000);
onUnmounted(() => clearInterval(ticker));
// Any command that moves the head (Home, moving it, Start, Sim) empties the bar and the
// times; Start and Sim then fill them again.
watch(
  () => status.task,
  (task) => {
    if (task === "home" || task === "move" || task === "cut" || task === "simulate") {
      jobProgress.value = null;
      elapsed.value = 0;
    }
  },
);

// A new job starts from 0: its first announcement is 0 %.
watch(jobProgress, (progress) => {
  if (progress?.fraction === 0) {
    elapsed.value = 0;
    lastTick = Date.now();
  }
});

/** "mm:ss", or "h:mm:ss" from one hour on. */
function clock(seconds: number): string {
  const s = Math.round(seconds);
  const pad = (n: number) => String(n).padStart(2, "0");
  const hours = Math.floor(s / 3600);
  const rest = `${pad(Math.floor((s % 3600) / 60))}:${pad(s % 60)}`;
  return hours > 0 ? `${hours}:${rest}` : rest;
}
/** The selection has shapes with their own settings (Material Test): the panel's are not used. */
const ownSettings = computed(() => selectedShapes().some((shape) => shape.cut || shape.type === "image"));
const idle = computed(() => status.connected && !status.busy);
const canStart = computed(() => (idle.value && hasSelection.value) || (status.busy && status.paused));
const canPause = computed(
  () => status.busy && !status.paused && (status.task === "cut" || status.task === "simulate"),
);
const canStop = computed(() => status.busy);
const canHome = idle;
const canSimulate = computed(() => idle.value && hasSelection.value);

/** Errors are always shown; otherwise, while idle, a hint about what to do next. */
const message = computed(() => {
  if (!idle.value || status.error) return status.message;
  return hasSelection.value ? "Click to start" : "No object selected";
});

/** Power, speed and passes for the next job; Rust checks the values on Start. */
const job = reactive<JobSettings>({ powerPercent: 50, feed: 2000, passes: 1 });

// The values saved last time replace the defaults once, when Rust first sends them.
let loaded = false;
watch(
  () => status.job,
  (saved) => {
    if (saved && !loaded) Object.assign(job, saved);
    loaded = true;
  },
);

// Every change is saved (a moment after the last key, not at each one); only valid numbers.
let saveTimer = 0;
watch(job, () => {
  clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => {
    const values = { powerPercent: Number(job.powerPercent), feed: Number(job.feed), passes: Number(job.passes) };
    const valid =
      values.powerPercent >= 0 &&
      values.powerPercent <= 100 &&
      values.feed > 0 &&
      Number.isInteger(values.passes) &&
      values.passes >= 1;
    if (valid) call("set_job_settings", { job: values });
  }, 500);
});

/**
 * The mouse wheel over a field: up adds `step`, down takes it away, within `min`–`max`.
 * Not while a job runs (the fields are locked then).
 */
function onWheel(event: WheelEvent, field: keyof JobSettings, step: number, min: number, max: number) {
  if (status.busy) return;
  const value = Number(job[field]) || 0;
  const next = value + (event.deltaY < 0 ? step : -step);
  // Back onto the steps (e.g. 2050 + 100 → 2100), then within the limits.
  job[field] = Math.min(max, Math.max(min, Math.round(next / step) * step));
}

/** All selected shapes, as Rust expects them. */
function selectedShapeData(): ShapeData[] {
  return selectedShapes().map(shapeData);
}

/** As `selectedShapeData`, with each image's dots (computed now, at its current size). */
function cutShapeData(): ShapeData[] {
  return selectedShapes().map((shape) =>
    shape.type === "image" ? { ...shapeData(shape), raster: rasterData(shape) } : shapeData(shape),
  );
}

/**
 * Start resumes a paused job. Otherwise, with Verify ticked, the first click only
 * shows the reminder popup; the next click (after closing it) cuts.
 */
function onStart() {
  if (status.paused) {
    call("resume");
    return;
  }
  if (status.checkBeforeStart && !reminded.value) {
    openChecks();
    return;
  }
  // The next job shows the reminder again.
  reminded.value = false;
  startCut();
}

function startCut() {
  // An empty input holds "" instead of a number: send numbers, so Rust replies
  // with a clear message (e.g. "Speed must be greater than 0"). Passes must be whole.
  start(
    {
      powerPercent: Number(job.powerPercent),
      feed: Number(job.feed),
      passes: Math.round(Number(job.passes)),
    },
    cutShapeData(),
  );
}

function onSimulate() {
  simulate(Number(job.feed), selectedShapeData());
}

function onCheckBeforeStartChange(event: Event) {
  setCheckBeforeStart((event.target as HTMLInputElement).checked);
}

// ----- Pre-start checks dialog -----

/** Reminders shown before cutting when Verify is ticked; nothing has to be confirmed. */
const CHECKS = [
  "Is the laser height (focus) adjusted?",
  "Is the laser head in the right position?",
  "Is the compressed air (air assist) on?",
];

/** The reminders for this job: a Material Test (shapes with their own settings) skips the focus one. */
const checks = computed(() => (ownSettings.value ? CHECKS.slice(1) : CHECKS));

const checksDialog = ref<HTMLDialogElement>();
/** True once the reminder was shown and closed: the next Start cuts. */
const reminded = ref(false);

function openChecks() {
  checksDialog.value!.showModal();
}

/** A click on the dimmed backdrop around the popup closes it. */
function onChecksClick(event: MouseEvent) {
  if (event.target === checksDialog.value) checksDialog.value!.close();
}

/** Closing the popup (✕, Escape or a click outside) does not cut; the next Start does. */
function onChecksClosed() {
  reminded.value = true;
}
</script>

<template>
  <aside class="laser-panel">
    <p
      class="status"
      :class="{ connected: status.connected, error: status.error }"
      title="Status of the laser; select the text to copy it (Ctrl+C)"
    >
      {{ message }}
    </p>

    <!-- How far the job is, as in LightBurn: time worked, the bar, estimated total time. -->
    <div
      class="progress"
      title="Time worked, progress of the job, and estimated total time (from the moves and their speeds)"
    >
      <!-- Without data both times show 00:00. -->
      <span class="time">{{ jobProgress ? clock(elapsed) : "00:00" }}</span>
      <div class="bar"><div class="done" :style="{ width: `${percent}%` }"></div></div>
      <span class="time">{{ jobProgress ? clock(jobProgress.totalSeconds) : "00:00" }}</span>
    </div>

    <div class="buttons">
      <!-- :disabled is recalculated automatically when `status` or the selection changes. -->
      <button
        :disabled="!canStart"
        :title="
          status.paused
            ? 'Resume the paused job'
            : 'Home the head ($H), then cut the selected shapes at their positions on the work area (absolute coordinates)'
        "
        @click="onStart"
      >
        <Play :size="16" /> Start
      </button>
      <button
        :disabled="!canPause"
        title="Pause the running job: motion stops and the laser goes off; Start resumes it"
        @click="call('pause')"
      >
        <Pause :size="16" /> Pause
      </button>
      <button
        class="stop"
        :disabled="!canStop"
        title="Stop immediately (soft reset), then unlock the laser"
        @click="call('stop')"
      >
        <Square :size="16" /> Stop
      </button>
    </div>

    <div class="tools">
      <button :disabled="!canHome" title="Homing ($H): move the head to the machine's origin" @click="call('home')">
        <House :size="16" /> Home
      </button>
      <button
        :disabled="!canSimulate"
        title="Home the head ($H), then follow the selected shapes once with the laser off, at the chosen speed"
        @click="onSimulate"
      >
        <Route :size="16" /> Simulate
      </button>
      <label class="check">
        Verify
        <input
          type="checkbox"
          :checked="status.checkBeforeStart"
          title="When ticked, Start first reminds you to check focus, head position and air assist"
          @change="onCheckBeforeStartChange"
        />
      </label>
    </div>

    <!-- Shapes with their own settings (Material Test): the fields are locked and show
         what the laser is cutting with, changing as the cut moves from cell to cell. -->
    <div v-if="ownSettings" class="job-settings">
      <label>
        Power
        <input
          class="power live"
          :value="cutProgress?.powerPercent ?? ''"
          placeholder="cell"
          disabled
          title="Each cell has its own power; during the cut: the current one, in %"
        />
      </label>
      <label>
        Speed
        <input
          class="speed live"
          :value="cutProgress?.feed ?? ''"
          placeholder="cell"
          disabled
          title="Each cell has its own speed; during the cut: the current one, in mm/min"
        />
      </label>
      <label>
        Passes
        <input
          class="passes live"
          :value="cutProgress ? `${cutProgress.pass}/${cutProgress.passes}` : ''"
          placeholder="cell"
          disabled
          title="During the cut: the current pass / the passes of the current cell"
        />
      </label>
    </div>
    <!-- Job settings; locked while a job runs. -->
    <div v-else class="job-settings">
      <label>
        Power
        <input
          v-model.number="job.powerPercent"
          class="power"
          type="number"
          min="0"
          max="100"
          :disabled="status.busy"
          title="Laser power, in % of the maximum ($30); mouse wheel: ±1"
          @wheel.prevent="onWheel($event, 'powerPercent', 1, 0, 100)"
        />
      </label>
      <label>
        Speed
        <input
          v-model.number="job.feed"
          class="speed"
          type="number"
          min="1"
          :disabled="status.busy"
          title="Cutting speed, in mm/min; mouse wheel: ±100"
          @wheel.prevent="onWheel($event, 'feed', 100, 1, 9999)"
        />
      </label>
      <label>
        Passes
        <input
          v-model.number="job.passes"
          class="passes"
          type="number"
          min="1"
          max="100"
          step="1"
          :disabled="status.busy"
          title="How many times the shapes are cut; each pass cuts every selected shape in turn; mouse wheel: ±1"
          @wheel.prevent="onWheel($event, 'passes', 1, 1, 100)"
        />
      </label>
    </div>

    <!-- Closing the popup (✕, Escape or a click outside) arms Start: the next click cuts. -->
    <dialog ref="checksDialog" class="checks-dialog" @click="onChecksClick" @close="onChecksClosed">
      <!-- The padding lives on this inner element, so a click inside the popup
           never has the dialog itself as target (that would close it). -->
      <div class="checks-content">
        <button
          class="close"
          title="Close this reminder; then click Start again to cut"
          @click="checksDialog?.close()"
        >
          <X :size="16" />
        </button>
        <p>Before starting:</p>
        <ul>
          <li v-for="check in checks" :key="check">{{ check }}</li>
        </ul>
      </div>
    </dialog>
  </aside>
</template>

<style scoped>
.laser-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px;
  background: var(--panel);
  border-left: 1px solid var(--border);
}

.status {
  margin: 0;
  padding: 6px 8px;
  border-radius: 4px;
  background: var(--canvas);
  color: var(--muted);
  /* Messages (e.g. errors) can be selected and copied, unlike the rest of the UI. */
  user-select: text;
  cursor: text;
}

.status.connected {
  color: inherit;
}

.status.error {
  color: var(--danger);
}

.buttons {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 6px;
}

/* Home, Simulate and the Verify box on one row, in the same three columns as Start, Pause, Stop. */
.tools {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  align-items: center;
  gap: 6px;
}

/* Verify at the right: its box ends where the inputs and the dividing lines do. */
.check {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 3px;
}

.check input {
  margin-right: 0;
}

.checks-dialog {
  padding: 0;
  border: 1px solid var(--border-strong);
  border-radius: 6px;
  background: var(--panel);
  color: var(--text);
}

.checks-dialog::backdrop {
  background: rgba(0, 0, 0, 0.3);
}

.checks-dialog p {
  margin: 0 0 8px;
}

.checks-dialog ul {
  margin: 0;
  padding-left: 18px;
}

/* Room on the right for the ✕ in the top-right corner. */
.checks-content {
  position: relative;
  padding: 12px 32px 12px 12px;
}

.checks-content .close {
  position: absolute;
  top: 4px;
  right: 4px;
  padding: 2px;
  border: none;
  background: none;
}

.checks-content .close:hover {
  background: var(--hover);
}

button {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: 6px 4px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--button);
  color: inherit;
  font: inherit;
  cursor: pointer;
}

button:hover:enabled {
  border-color: var(--accent);
}

button:disabled {
  opacity: 0.4;
  cursor: default;
}

.stop:enabled {
  background: var(--danger);
  border-color: var(--danger);
  color: #ffffff;
}

/* Label and input pairs on one row, separated from the buttons like the zoom
   buttons are from the size fields in the top bar. */
/* Above Start, with a dividing line below. */
.progress {
  display: flex;
  align-items: center;
  gap: 5px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--muted);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

/* As wide as their text, so they start and end where the dividing line does. */
.time {
  flex: none;
}

.bar {
  flex: 1;
  height: 8px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--button);
  overflow: hidden;
}

.done {
  height: 100%;
  background: var(--accent);
}

/* The same three columns as the buttons: Power under Start (at its left), Speed under
   Pause (centered), Passes under Stop (ending where the dividing line does). */
.job-settings {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 6px;
  padding-top: 8px;
  border-top: 1px solid var(--muted);
}

.job-settings label:nth-child(2) {
  justify-self: center;
}

.job-settings label:nth-child(3) {
  justify-self: end;
}

.job-settings label {
  display: flex;
  align-items: center;
  gap: 3px;
}

/* ch = width of one digit: power 3 digits, speed 4, passes 1.
   The arrows of number inputs are hidden. */
.job-settings input {
  padding: 2px 4px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--button);
  color: inherit;
  font: inherit;
  text-align: right;
  appearance: textfield;
}

.job-settings input::-webkit-inner-spin-button,
.job-settings input::-webkit-outer-spin-button {
  -webkit-appearance: none;
  margin: 0;
}

.job-settings input:disabled {
  opacity: 0.5;
}

/* A little wider than 3 digits. */
.job-settings .power {
  width: calc(3ch + 3px);
}

/* 4 digits: up to 9999 mm/min. */
.job-settings .speed {
  width: 4ch;
}

.job-settings .passes {
  width: 2ch;
}

/* The locked fields showing the cut's current values: readable, room for "12.5" or "1/2". */
.job-settings input.live {
  width: 4ch;
  opacity: 1;
}
</style>
