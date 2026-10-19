<script setup lang="ts">
// Tools > Material Test: the settings, a preview of the grid, and Push to canvas.
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { X } from "lucide-vue-next";
import { materialTest as test, type Part } from "../materialTest";
import { addShapes, editor, type Shape } from "../editor";
import { outlinesShape } from "../paths";

const emit = defineEmits<{ close: [] }>();

/** The grid as Rust lays it out, or the reason it cannot. */
const parts = ref<Part[]>([]);
const error = ref("");

/** Fields Rust expects as whole numbers. */
const WHOLE = ["powerCount", "speedCount", "passes"];

/**
 * The values as numbers (inputs hold text); counts and passes rounded.
 * Throws when a field is empty or not a number.
 */
function numbers() {
  const entries = Object.entries(test).map(([key, value]) => {
    if (key === "mode" || typeof value === "boolean") return [key, value];
    const number = Number(value);
    if (String(value).trim() === "" || !Number.isFinite(number)) throw "Enter a number in every field";
    return [key, WHOLE.includes(key) ? Math.round(number) : number];
  });
  return Object.fromEntries(entries);
}

// The preview follows every change; an older answer arriving late is ignored.
let request = 0;
watch(
  test,
  async () => {
    const mine = ++request;
    try {
      const result = await invoke<Part[]>("material_test_preview", { test: numbers() });
      if (mine !== request) return;
      parts.value = result;
      error.value = "";
    } catch (e) {
      if (mine !== request) return;
      parts.value = [];
      error.value = String(e);
    }
  },
  { immediate: true },
);

/** Bounds of the whole test, in mm. */
const bounds = computed(() => {
  const points = parts.value.flatMap((part) => part.contours.flat());
  if (points.length === 0) return null;
  const xs = points.map((p) => p.x);
  const ys = points.map((p) => p.y);
  return { left: Math.min(...xs), right: Math.max(...xs), bottom: Math.min(...ys), top: Math.max(...ys) };
});

/** SVG viewBox with a 1 mm margin; Y is flipped by the group's transform. */
const viewBox = computed(() => {
  const b = bounds.value;
  return b ? `${b.left - 1} ${-b.top - 1} ${b.right - b.left + 2} ${b.top - b.bottom + 2}` : "0 0 1 1";
});

function pathOf(part: Part): string {
  return part.contours.map((c) => "M" + c.map((p) => `${p.x} ${p.y}`).join("L") + "Z").join("");
}

/** Cells darker with more power, as the burn will look. */
function shade(part: Part): number {
  return part.label ? 1 : 0.15 + 0.85 * (part.powerPercent / 100);
}

/**
 * Puts the test on the canvas, selected so it moves as one: each cell a rectangle and
 * each label a path, all with their own power and speed, which Start then uses.
 */
function onPush() {
  const values = numbers();
  const shapes: Omit<Shape, "id">[] = parts.value.map((part) => {
    const cut = {
      powerPercent: part.powerPercent,
      feed: part.feed,
      // Labels are engraved once; the cells get every pass.
      passes: part.label ? 1 : values.passes,
      fill: part.fill,
      interval: values.interval,
    };
    if (part.label) return { ...outlinesShape(part.contours)!, cut };
    const xs = part.contours[0].map((p) => p.x);
    const ys = part.contours[0].map((p) => p.y);
    const [left, right, bottom, top] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
    return {
      type: "rectangle",
      cx: (left + right) / 2,
      cy: (bottom + top) / 2,
      rx: (right - left) / 2,
      ry: (top - bottom) / 2,
      cut,
    };
  });
  // Fixed size: resizing would no longer match the cell size and the fill interval.
  addShapes(shapes.map((shape) => ({ ...shape, fixedSize: true })));
  editor.tool = "select";
  emit("close");
}

function onKey(event: KeyboardEvent) {
  if (event.key === "Escape") emit("close");
}
onMounted(() => window.addEventListener("keydown", onKey));
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div class="backdrop" @pointerdown.self="emit('close')">
    <div class="dialog">
      <div class="title">
        Material Test
        <button class="icon" title="Close (Esc)" @click="emit('close')"><X :size="14" /></button>
      </div>

      <div class="body">
        <!-- One grid: a label column and three input columns, so the inputs line up;
             each group has a small header row naming its columns. -->
        <div class="form">
          <span>Mode</span>
          <select v-model="test.mode" title="Fill: each cell engraved full, line by line; Line: only its outline">
            <option value="fill">Fill</option>
            <option value="line">Line</option>
          </select>
          <span></span><span></span>

          <span></span><span class="head">Count</span><span class="head">Min</span><span class="head">Max</span>
          <span>Power %</span>
          <input v-model="test.powerCount" title="Number of columns, one power each" />
          <input v-model="test.powerMin" title="Power of the first column, in %" />
          <input v-model="test.powerMax" title="Power of the last column, in %" />
          <span>Speed</span>
          <input v-model="test.speedCount" title="Number of rows, one speed each" />
          <input v-model="test.speedMin" title="Speed of the top row, in mm/min" />
          <input v-model="test.speedMax" title="Speed of the bottom row, in mm/min" />

          <span></span><span class="head">W</span><span class="head">H</span><span class="head">Gap</span>
          <span>Cell</span>
          <input v-model="test.cellWidth" title="Width of a cell, in mm" />
          <input v-model="test.cellHeight" title="Height of a cell, in mm" />
          <input v-model="test.spacing" title="Gap between cells, in mm" />

          <span></span><span class="head">Interval</span><span class="head">Passes</span><span></span>
          <span>Burn</span>
          <input
            v-model="test.interval"
            :disabled="test.mode !== 'fill'"
            title="Fill: distance between the engraved lines, in mm"
          />
          <input v-model="test.passes" title="How many times each cell is burned" />
          <span></span>

          <span></span><span class="head">Size</span><span class="head">Power</span><span class="head">Speed</span>
          <label title="Engrave the power and speed values next to the grid">
            <input v-model="test.labels" type="checkbox" class="check" /> Labels
          </label>
          <input v-model="test.labelSize" :disabled="!test.labels" title="Height of the label text, in mm" />
          <input v-model="test.labelPower" :disabled="!test.labels" title="Power of the labels, in %" />
          <input v-model="test.labelSpeed" :disabled="!test.labels" title="Speed of the labels, in mm/min" />

          <span></span><span class="head">X</span><span class="head">Y</span><span></span>
          <span>Position</span>
          <input v-model="test.x" title="X of the test's bottom-left corner (labels included), in mm" />
          <input v-model="test.y" title="Y of the test's bottom-left corner (labels included), in mm" />
          <span></span>
        </div>

        <div class="preview">
          <svg :viewBox="viewBox">
            <title>Preview: darker cells have more power</title>
            <g transform="scale(1 -1)">
              <path
                v-for="(part, index) in parts"
                :key="index"
                :d="pathOf(part)"
                fill-rule="evenodd"
                :fill="part.fill ? 'var(--shape)' : 'none'"
                :fill-opacity="shade(part)"
                :stroke="part.fill ? 'none' : 'var(--shape)'"
                :stroke-opacity="shade(part)"
                vector-effect="non-scaling-stroke"
              />
            </g>
          </svg>
          <div v-if="bounds" class="size">
            {{ (bounds.right - bounds.left).toFixed(1) }} × {{ (bounds.top - bounds.bottom).toFixed(1) }} mm
          </div>
        </div>
      </div>

      <div class="footer">
        <span class="error">{{ error }}</span>
        <button
          class="start"
          :disabled="!!error || parts.length === 0"
          title="Put the test on the canvas, selected: drag it where you want, then Start cuts each cell with its own power and speed"
          @click="onPush"
        >
          Push to canvas
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: grid;
  place-items: center;
  background: rgba(0, 0, 0, 0.3);
}

.dialog {
  background: var(--panel);
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
}

.title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 2px 4px 2px 8px;
  border-bottom: 1px solid var(--border);
  font-weight: 600;
}

.body {
  display: flex;
  gap: 8px;
  padding: 6px 8px;
}

.form {
  display: grid;
  grid-template-columns: auto auto auto auto;
  align-items: center;
  align-content: start;
  gap: 2px 6px;
}

/* The header above a group's inputs, close to them. */
.head {
  margin-top: 4px;
  color: var(--muted);
  text-align: center;
  line-height: 1;
}

label {
  display: flex;
  align-items: center;
  gap: 4px;
}

input,
select {
  padding: 2px 4px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--button);
  color: inherit;
  font: inherit;
}

input {
  width: 6ch;
  text-align: right;
}

input.check {
  width: auto;
}

input:disabled {
  opacity: 0.5;
}

.preview {
  display: flex;
  flex-direction: column;
  align-items: center;
  width: 260px;
}

/* Only the preview: the close icon is an <svg> too. */
.preview svg {
  width: 260px;
  height: 200px;
  background: var(--bed);
  border: 1px solid var(--border);
}

.size {
  color: var(--muted);
}

.footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  padding: 4px 8px 6px;
}

.error {
  color: var(--danger);
}

button.icon {
  display: grid;
  place-items: center;
  padding: 2px;
  border: none;
  border-radius: 4px;
  background: none;
  color: inherit;
  cursor: pointer;
}

button.icon:hover {
  background: var(--hover);
}

.start {
  padding: 2px 16px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--button);
  color: inherit;
  font: inherit;
  cursor: pointer;
}

.start:hover:enabled {
  background: var(--hover);
}

.start:disabled {
  opacity: 0.5;
  cursor: default;
}
</style>
