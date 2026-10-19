<script setup lang="ts">
// Help > How to use: a large popup with instructions, one section per topic.
import { onMounted, onUnmounted } from "vue";
import { X } from "lucide-vue-next";

const emit = defineEmits<{ close: [] }>();

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
        How to use
        <button class="icon" title="Close (Esc)" @click="emit('close')"><X :size="14" /></button>
      </div>

      <div class="content">
        <h2>Engraving images</h2>
        <ol>
          <li>File → Import… (or Ctrl+I, as in LightBurn) and pick a PNG, JPG or BMP.</li>
          <li>
            The image appears in the middle of the work area, at the size its DPI gives (254 DPI if the file
            has none). Adjust Image opens by itself.
          </li>
          <li>
            In Adjust Image: the original on the left, the engraving preview on the right, with the number of
            dots under it. Settings: Image Mode (Threshold, Floyd-Steinberg, Jarvis, Stucki, Grayscale), Line
            Interval / DPI, Negative, Contrast, Brightness, Gamma, Speed, Power max and Power min (Grayscale
            only). Reset All, OK and Cancel.
          </li>
          <li>
            On the canvas the image shows as it will be engraved. Move, rotate and resize it like any shape.
            Double-click it to open Adjust Image again.
          </li>
          <li>
            Start engraves it line by line, with a 2.5 mm overscan. Power, Speed and Passes in the laser panel
            are locked and show the image's settings, as with Material Test.
          </li>
        </ol>
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

/* Large: most of the window, scrolling when the text is longer. */
.dialog {
  display: flex;
  flex-direction: column;
  width: min(900px, 90vw);
  height: 85vh;
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

.content {
  overflow-y: auto;
  padding: 4px 12px 8px;
  /* The text can be selected and copied. */
  user-select: text;
}

h2 {
  margin: 4px 0;
  font-size: 1.1em;
}

ol {
  margin: 0;
  padding-left: 20px;
}

li + li {
  margin-top: 4px;
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
