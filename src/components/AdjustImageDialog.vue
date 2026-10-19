<script setup lang="ts">
// Adjust Image, as in LightBurn: the original on the left, how it will be engraved on the
// right; the settings change a copy, which OK writes to the image (Cancel drops it).
import { computed, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import { X } from "lucide-vue-next";
import { editor } from "../editor";
import {
  adjustImageId,
  DEFAULT_IMAGE_SETTINGS,
  IMAGE_MODES,
  imageUrl,
  rasterize,
  rasterUrl,
  type ImageSettings,
} from "../images";

const shape = computed(() => editor.shapes.find((other) => other.id === adjustImageId.value));
const draft = reactive<ImageSettings>({ ...DEFAULT_IMAGE_SETTINGS, ...shape.value?.imageSettings });

/** DPI ⇄ Line Interval: two ways to say the same distance between dots. */
const dpi = computed({
  get: () => Math.round((25.4 / draft.interval) * 10) / 10,
  set: (value: number) => {
    if (value > 0) draft.interval = Math.round((25.4 / value) * 10000) / 10000;
  },
});

/** The engraving preview and its size in dots; recomputed once per frame at most. */
const preview = ref("");
const size = ref("");
let frame = 0;
watch(
  draft,
  () => {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => {
      if (!shape.value) return;
      const raster = rasterize(shape.value, draft);
      preview.value = raster ? rasterUrl(raster) : "";
      size.value = raster ? `${raster.columns} × ${raster.rows}` : "";
    });
  },
  { immediate: true },
);

function close() {
  adjustImageId.value = null;
}

function ok() {
  if (shape.value) shape.value.imageSettings = { ...draft };
  close();
}

function reset() {
  Object.assign(draft, DEFAULT_IMAGE_SETTINGS);
}

function onKey(event: KeyboardEvent) {
  if (event.key === "Escape") close();
}
onMounted(() => window.addEventListener("keydown", onKey));
onUnmounted(() => {
  window.removeEventListener("keydown", onKey);
  cancelAnimationFrame(frame);
});
</script>

<template>
  <div v-if="shape" class="backdrop" @pointerdown.self="close">
    <div class="dialog">
      <div class="title">
        Adjust Image
        <button class="icon" title="Cancel (Esc)" @click="close"><X :size="14" /></button>
      </div>

      <div class="pictures">
        <figure>
          <img :src="imageUrl(shape)" alt="Original image" title="The original image" />
        </figure>
        <figure>
          <img :src="preview" alt="Engraving preview" title="How it will be engraved: dark dots burn" />
          <figcaption>{{ size }} dots</figcaption>
        </figure>
      </div>

      <div class="settings">
        <!-- Like the form of the Material Test: a label column and input columns. -->
        <div class="form pairs">
          <span>Image Mode</span>
          <select v-model="draft.mode" :title="IMAGE_MODES.find((m) => m.id === draft.mode)?.title">
            <option v-for="mode in IMAGE_MODES" :key="mode.id" :value="mode.id" :title="mode.title">
              {{ mode.name }}
            </option>
          </select>
          <span>Line Interval</span>
          <input
            v-model.number="draft.interval"
            type="number"
            step="0.01"
            min="0.01"
            title="Distance between engraved lines and between dots on a line, in mm"
          />
          <span>DPI</span>
          <input v-model.number="dpi" type="number" step="1" min="1" title="Dots per inch: the same distance as Line Interval" />
          <span>Negative</span>
          <input
            v-model="draft.negative"
            type="checkbox"
            class="check"
            title="Burn the light parts instead of the dark ones"
          />
        </div>

        <div class="form">
          <span>Contrast</span>
          <input v-model.number="draft.contrast" type="range" min="-100" max="100" title="Contrast: -100 to 100" />
          <input v-model.number="draft.contrast" type="number" title="Contrast: -100 to 100" />
          <span>Brightness</span>
          <input v-model.number="draft.brightness" type="range" min="-100" max="100" title="Brightness: -100 to 100" />
          <input v-model.number="draft.brightness" type="number" title="Brightness: -100 to 100" />
          <span>Gamma</span>
          <input
            v-model.number="draft.gamma"
            type="range"
            min="0.1"
            max="3"
            step="0.05"
            title="Gamma: above 1 lighter mid-tones, below 1 darker"
          />
          <input v-model.number="draft.gamma" type="number" step="0.05" title="Gamma: above 1 lighter mid-tones, below 1 darker" />
        </div>

        <div class="form pairs">
          <span>Speed</span>
          <input v-model.number="draft.speed" type="number" title="Engraving speed, in mm/min" />
          <span>Power max</span>
          <input v-model.number="draft.powerMax" type="number" title="Power of a black dot, in % of the laser's maximum" />
          <span>Power min</span>
          <input
            v-model.number="draft.powerMin"
            type="number"
            :disabled="draft.mode !== 'grayscale'"
            title="Grayscale: power of a white dot, in %"
          />
        </div>
      </div>

      <div class="footer">
        <button title="Back to the default settings" @click="reset">Reset All</button>
        <span class="grow"></span>
        <button title="Keep these settings" @click="ok">OK</button>
        <button title="Drop the changes (Esc)" @click="close">Cancel</button>
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

.pictures {
  display: flex;
  gap: 8px;
  padding: 6px 8px 0;
}

figure {
  display: flex;
  flex-direction: column;
  align-items: center;
  margin: 0;
}

/* Both pictures in the same box, whole and undistorted. */
img {
  width: 360px;
  height: 360px;
  object-fit: contain;
  background: var(--bed);
  border: 1px solid var(--border);
}

/* The preview at its real dots: no smoothing between them. */
figure + figure img {
  image-rendering: pixelated;
}

figcaption {
  color: var(--muted);
}

.settings {
  display: flex;
  gap: 16px;
  padding: 6px 8px;
}

.form {
  display: grid;
  grid-template-columns: auto auto auto;
  align-items: center;
  align-content: start;
  gap: 2px 6px;
}

/* Groups of label + input (the sliders' group has label + slider + number). */
.form.pairs {
  grid-template-columns: auto auto;
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

input[type="number"] {
  width: 7ch;
  text-align: right;
}

input[type="range"] {
  width: 140px;
  padding: 0;
  border: none;
  background: none;
}

input.check {
  justify-self: start;
}

input:disabled {
  opacity: 0.5;
}

.footer {
  display: flex;
  gap: 8px;
  padding: 4px 8px 6px;
}

.grow {
  flex: 1;
}

.footer button {
  padding: 2px 16px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--button);
  color: inherit;
  font: inherit;
  cursor: pointer;
}

.footer button:hover {
  background: var(--hover);
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
</style>
