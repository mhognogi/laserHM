<script setup lang="ts">
import { computed } from "vue";
import { selectedShapes } from "../editor";
import { applyTextSettings, fonts, textEdit, textSettings, textSize } from "../text";

// Font and size of the text, in the top bar after the zoom buttons. The bar shows
// them while the Text tool is active or a single text is selected.

/** The selected text, when exactly one text is selected. */
const selectedText = computed(() => {
  const shapes = selectedShapes();
  return shapes.length === 1 && shapes[0].type === "text" ? shapes[0] : null;
});

/** What the fields show: the text being typed, else the selected text, else the next text's settings. */
const shownFont = computed(
  () => (textEdit.value?.font ?? selectedText.value?.font ?? textSettings.font)?.name ?? "",
);
const shownSize = computed(() => {
  if (textEdit.value) return textEdit.value.size.toFixed(1);
  return (selectedText.value ? textSize(selectedText.value) : textSettings.size).toFixed(1);
});

const shownWelded = computed(
  () => textEdit.value?.welded ?? selectedText.value?.welded ?? textSettings.welded,
);

function apply(fontName: string, size: number, welded: boolean) {
  const font = fonts.value.find((candidate) => candidate.name === fontName);
  if (font) applyTextSettings(font, size, welded, selectedText.value);
}

function onFontChange(event: Event) {
  const select = event.target as HTMLSelectElement;
  apply(select.value, Number(shownSize.value), shownWelded.value);
  // Give the keys back to the text being typed on the canvas.
  select.blur();
}

/** The size must be a number above 0; anything else restores the shown size. */
function onSizeChange(event: Event) {
  const input = event.target as HTMLInputElement;
  const size = Number(input.value);
  if (input.value.trim() !== "" && Number.isFinite(size) && size > 0) {
    apply(shownFont.value, size, shownWelded.value);
  }
  input.value = shownSize.value;
}

function onWeldedChange(event: Event) {
  const checkbox = event.target as HTMLInputElement;
  apply(shownFont.value, Number(shownSize.value), checkbox.checked);
  // Give the keys back to the text being typed on the canvas.
  checkbox.blur();
}
</script>

<template>
  <div class="text-popup">
    <select
      :value="shownFont"
      :disabled="fonts.length === 0"
      title="Font of the text being typed or selected, and of the next one"
      @change="onFontChange"
    >
      <option v-if="fonts.length === 0" value="">Loading fonts…</option>
      <option v-for="font in fonts" :key="font.name" :value="font.name">{{ font.name }}</option>
    </select>
    <label>
      Size
      <!-- :value + @change: the text changes on Enter, on leaving the field or on an arrow click.
           step 1: the arrows (and the ↑/↓ keys) go to the next whole mm. -->
      <input
        class="size"
        type="number"
        step="1"
        :value="shownSize"
        title="Height of the capital letters, in mm (arrows or ↑/↓: 1 mm steps); applies to the text being typed or selected, and to the next one"
        @change="onSizeChange"
        @keyup.up="onSizeChange"
        @keyup.down="onSizeChange"
        @keydown.enter="($event.target as HTMLInputElement).blur()"
      />
    </label>
    <label>
      Welded
      <input
        type="checkbox"
        :checked="shownWelded"
        title="Join overlapping letters into one outline, so the laser does not cut the lines inside the overlaps; applies to the text being typed or selected, and to the next one"
        @change="onWeldedChange"
      />
    </label>
  </div>
</template>

<style scoped>
.text-popup {
  display: flex;
  align-items: center;
  gap: 4px;
  white-space: nowrap;
}

label {
  display: flex;
  align-items: center;
  gap: 3px;
}

.size,
select {
  padding: 2px 4px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--button);
  color: inherit;
  font: inherit;
}

select {
  max-width: 24ch;
}

/* ch = width of one digit: room for "999.9", plus the arrows. */
.size {
  width: calc(5ch + 16px);
  text-align: right;
}
</style>
