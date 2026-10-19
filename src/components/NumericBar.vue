<script setup lang="ts">
import { computed } from "vue";
import {
  Camera,
  CameraOff,
  Focus,
  Scan,
  ZoomIn,
  ZoomOut,
} from "lucide-vue-next";
import {
  calibrating,
  camera,
  canCalibrate,
  cancelCalibration,
  CALIBRATION_POINTS,
  chooseCamera,
  startCalibration,
  toggleCamera,
} from "../camera";
import {
  editor,
  normalizeAngle,
  rotateSelected,
  selectedShapes,
  selectionBox,
  sizeLocked,
  snapshotSelected,
  transformGroup,
} from "../editor";
import { zoomIn, zoomOut, zoomToBox, zoomToFit } from "../view";
import { fireSettings } from "../laser";
import { measured } from "../measure";
import TextPopup from "./TextPopup.vue";

/** Fire power: a whole or decimal % from 0 to 100; anything else restores the current value. */
function onFirePowerChange(event: Event) {
  const input = event.target as HTMLInputElement;
  const value = Number(input.value);
  if (input.value.trim() !== "" && Number.isFinite(value) && value >= 0 && value <= 100) {
    fireSettings.powerPercent = value;
  }
  input.value = String(fireSettings.powerPercent);
}

type Field = "x" | "y" | "width" | "height";

// The four inputs in LightBurn's order, in two groups: position and size.
// They describe the box around all selected shapes; xCtr/yCtr are its center.
const groups: { id: Field; label: string; title: string }[][] = [
  [
    { id: "x", label: "xCtr", title: "X position of the selection's center, in mm" },
    { id: "y", label: "yCtr", title: "Y position of the selection's center, in mm" },
  ],
  [
    { id: "width", label: "Width", title: "Width of the selection, in mm" },
    { id: "height", label: "Height", title: "Height of the selection, in mm" },
  ],
];

/** Current values of the selection's box, or null when nothing is selected. */
const values = computed(() => {
  const box = selectionBox();
  if (!box) return null;
  return {
    x: (box.left + box.right) / 2,
    y: (box.bottom + box.top) / 2,
    width: box.right - box.left,
    height: box.top - box.bottom,
  };
});

function isSize(field: Field): boolean {
  return field === "width" || field === "height";
}

function format(field: Field): string {
  return values.value ? values.value[field].toFixed(1) : "";
}

/**
 * Applies a typed value to the selection: the position moves all selected shapes,
 * the size scales them around the selection's center. Invalid input restores the old value.
 */
function apply(field: Field, event: Event) {
  const input = event.target as HTMLInputElement;
  const value = Number(input.value);
  const valid = input.value.trim() !== "" && Number.isFinite(value) && (!isSize(field) || value > 0);
  const box = selectionBox();

  if (box && values.value && valid) {
    const next = { ...values.value, [field]: value };
    transformGroup(snapshotSelected(), box, {
      left: next.x - next.width / 2,
      right: next.x + next.width / 2,
      bottom: next.y - next.height / 2,
      top: next.y + next.height / 2,
    });
  }
  // Always rewrite the input: shows the formatted new value, or the old one if invalid.
  input.value = format(field);
}

/**
 * The selection's angle for the Rotate field: the angle shared by all selected shapes,
 * or 0 when their angles differ (or nothing is selected).
 */
const angle = computed(() => {
  const angles = selectedShapes().map((shape) => shape.angle ?? 0);
  return angles.length > 0 && angles.every((a) => a === angles[0]) ? angles[0] : 0;
});

/**
 * The Rotate field sets an absolute angle: the selection turns around its center by the
 * difference to the angle shown. Invalid input restores the shown angle.
 */
function applyRotate(event: Event) {
  const input = event.target as HTMLInputElement;
  const degrees = Number(input.value);
  if (input.value.trim() !== "" && Number.isFinite(degrees)) {
    const delta = normalizeAngle(degrees - angle.value);
    if (delta !== 0) rotateSelected(delta);
  }
  // Always rewrite the input: shows the new angle, or the old one if invalid.
  input.value = angle.value.toFixed(1);
}

/** Middle zoom button: the selection if there is one, otherwise the whole work area. */
function zoomToSelectionOrFit() {
  const box = selectionBox();
  if (box) zoomToBox(box);
  else zoomToFit();
}

/** Font and size show while the Text tool is active or a single text is selected. */
const showText = computed(() => {
  const shapes = selectedShapes();
  return editor.tool === "text" || (shapes.length === 1 && shapes[0].type === "text");
});
</script>

<template>
  <div class="numeric-bar">
    <div v-for="(fields, index) in groups" :key="index" class="group">
      <label v-for="field in fields" :key="field.id">
        {{ field.label }}
        <!-- :value + @change instead of v-model: the shape changes only on Enter or leaving the field. -->
        <input
          type="text"
          inputmode="decimal"
          :value="format(field.id)"
          :disabled="!values || (isSize(field.id) && sizeLocked())"
          :title="isSize(field.id) && sizeLocked() ? 'A Material Test cannot be resized' : field.title"
          @change="apply(field.id, $event)"
          @keydown.enter="($event.target as HTMLInputElement).blur()"
        />
      </label>
      <!-- After Height: the selection's angle, applied like the other fields (Enter or leaving it). -->
      <label v-if="index === 1">
        Rotate
        <input
          type="text"
          inputmode="decimal"
          :value="values ? angle.toFixed(1) : ''"
          :disabled="!values"
          title="Angle of the selection in degrees, counterclockwise (negative: clockwise); type a new angle to turn the selection to it around its center (0 when the selected shapes have different angles)"
          @change="applyRotate"
          @keydown.enter="($event.target as HTMLInputElement).blur()"
        />
      </label>
    </div>

    <div class="zoom">
      <button title="Zoom out" @click="zoomOut"><ZoomOut :size="16" /></button>
      <button
        :title="values ? 'Zoom to the selected shapes' : 'Zoom to fit: show the whole work area'"
        @click="zoomToSelectionOrFit"
      >
        <Scan :size="16" />
      </button>
      <button title="Zoom in" @click="zoomIn"><ZoomIn :size="16" /></button>
    </div>

    <!-- Camera overlay: on/off, which camera, and how transparent it is over the bed. -->
    <div class="zoom camera">
      <button
        :class="{ on: camera.on }"
        :title="
          camera.on
            ? 'Turn off the camera overlay'
            : 'Show the live camera image over the work area (stretched to the bed, not calibrated yet)'
        "
        @click="toggleCamera"
      >
        <component :is="camera.on ? Camera : CameraOff" :size="16" />
      </button>
      <select
        v-if="camera.devices.length > 1"
        :value="camera.deviceId"
        title="Camera used for the overlay"
        @change="chooseCamera(($event.target as HTMLSelectElement).value)"
      >
        <option v-for="device in camera.devices" :key="device.deviceId" :value="device.deviceId">
          {{ device.label }}
        </option>
      </select>
      <!-- The wrapper gets the hover while the button is disabled, to show what is missing. -->
      <span v-if="camera.on" class="calibrate">
        <button
          :class="{ on: calibrating }"
          :disabled="!calibrating && !canCalibrate()"
          :title="
            calibrating
              ? 'Cancel the camera calibration (Esc)'
              : canCalibrate()
                ? 'Calibrate the camera: with the cut square selected (and still on the bed), click 9 of its points in the image (corners, middles of the sides, center); the image is then aligned with the canvas'
                : undefined
          "
          @click="calibrating ? cancelCalibration() : startCalibration()"
        >
          <Focus :size="16" />
        </button>
        <span v-if="!calibrating && !canCalibrate()" class="missing">
Draw a square and select it
        </span>
      </span>
      <span v-if="calibrating" class="hint">
        Click the {{ CALIBRATION_POINTS[calibrating.image.length].name }} of the square ({{
          calibrating.image.length + 1
        }}/{{ CALIBRATION_POINTS.length }})
      </span>
      <input
        v-if="camera.on"
        v-model.number="camera.opacity"
        type="range"
        min="0.1"
        max="1"
        step="0.05"
        title="Opacity of the camera image: left more transparent, right more visible"
      />
    </div>

    <TextPopup v-if="showText" class="text" />

    <!-- With Measure: the measured distance (mm), its horizontal and vertical parts,
         and its direction (degrees, counterclockwise from the X axis). -->
    <div v-if="editor.tool === 'measure'" class="measure">
      <template v-if="measured">
        <span>Dist {{ measured.distance.toFixed(1) }}</span>
        <span>dX {{ measured.dx.toFixed(1) }}</span>
        <span>dY {{ measured.dy.toFixed(1) }}</span>
        <span>Angle {{ measured.angle.toFixed(1) }}°</span>
      </template>
      <span v-else class="hint">Drag on the canvas to measure</span>
    </div>

    <!-- With Position Laser: the power of the Fire button under it. -->
    <label v-if="editor.tool === 'position'" class="fire">
      Fire
      <input
        type="number"
        min="0"
        max="100"
        step="5"
        :value="fireSettings.powerPercent"
        title="Fire power, in % of the laser's maximum ($30); arrows or ↑/↓: 5% steps"
        @change="onFirePowerChange"
        @keyup.up="onFirePowerChange"
        @keyup.down="onFirePowerChange"
        @keydown.enter="($event.target as HTMLInputElement).blur()"
      />
    </label>
  </div>
</template>

<style scoped>
.numeric-bar {
  display: flex;
  align-items: center;
  padding: 4px 0;
  background: var(--panel);
  border-bottom: 1px solid var(--border);
}

/* Fields of a group sit close together; groups are separated by a vertical border. */
.group {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 12px;
}

.group + .group {
  border-left: 1px solid var(--border-strong);
}

label {
  display: flex;
  align-items: center;
  gap: 4px;
}

input {
  /* ch = width of the digit "0": room for "9999.9" (the dot is narrower than a digit). */
  width: 6ch;
  padding: 2px 4px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--button);
  color: inherit;
  font: inherit;
  text-align: right;
}

input:disabled {
  opacity: 0.5;
}

/* Zoom buttons, after a darker border than the one between the field groups. */
.zoom {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 0 8px;
  border-left: 1px solid var(--muted);
}

/* Measured values, after the zoom buttons; tabular digits so they do not jump while dragging. */
.measure {
  display: flex;
  gap: 12px;
  padding: 0 8px;
  border-left: 1px solid var(--muted);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.measure .hint {
  color: var(--muted);
}

/* Fire power, after the zoom buttons; ch: room for "100" plus the arrows. */
.fire {
  padding: 0 8px;
  border-left: 1px solid var(--muted);
}

.fire input {
  width: calc(3ch + 16px);
}

/* Font and size, after the zoom buttons. */
.text {
  padding: 0 8px;
  border-left: 1px solid var(--muted);
}

.zoom button {
  display: grid;
  place-items: center;
  padding: 2px;
  border: 1px solid transparent;
  border-radius: 4px;
  background: none;
  color: inherit;
  cursor: pointer;
}

.zoom button:hover:enabled {
  background: var(--hover);
}

/* The camera button while the overlay is on. */
.zoom button.on {
  background: var(--accent-soft);
  border-color: var(--accent);
}

.camera select {
  max-width: 18ch;
  padding: 2px 4px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--button);
  color: inherit;
  font: inherit;
}

/* Calibrate: when disabled, a red tooltip under it says what to do. */
.calibrate {
  position: relative;
  display: flex;
}

/* A disabled button gets no mouse events; this lets the hover reach the wrapper. */
.calibrate button:disabled {
  pointer-events: none;
}

.calibrate .missing {
  display: none;
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  z-index: 10;
  white-space: nowrap;
  padding: 4px 6px;
  border: 1px solid var(--danger);
  border-radius: 4px;
  background: var(--panel);
  color: var(--danger);
}

.calibrate:hover .missing {
  display: block;
}

.camera .hint {
  white-space: nowrap;
  color: var(--danger);
}

/* The opacity slider: the general input style does not apply to it. */
.camera input[type="range"] {
  width: 70px;
  padding: 0;
  border: none;
  background: none;
}

.zoom button:disabled {
  opacity: 0.4;
  cursor: default;
}
</style>
