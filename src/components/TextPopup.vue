<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";
import { ChevronDown } from "lucide-vue-next";
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

// ----- Font list -----
// A list of our own instead of a <select>: an open <select> tells nothing while the arrows
// move through it, and here every arrow press changes the font on the canvas at once.

const listOpen = ref(false);
/** The font highlighted in the list (index in `fonts`). */
const highlight = ref(0);
/** The font before the list opened: Esc goes back to it. */
let fontBefore = "";
const fontButton = ref<HTMLButtonElement>();
const fontList = ref<HTMLUListElement>();

function openList() {
  fontBefore = shownFont.value;
  highlight.value = Math.max(0, fonts.value.findIndex((font) => font.name === shownFont.value));
  listOpen.value = true;
  scrollToHighlight();
}

/** Closes the list and gives the keys back to the canvas (e.g. the text being typed). */
function closeList() {
  listOpen.value = false;
  fontButton.value?.blur();
}

function scrollToHighlight() {
  nextTick(() => fontList.value?.children[highlight.value]?.scrollIntoView({ block: "nearest" }));
}

/** Highlights font `index` and puts it on the text right away. */
function preview(index: number) {
  highlight.value = Math.min(fonts.value.length - 1, Math.max(0, index));
  apply(fonts.value[highlight.value].name, Number(shownSize.value), shownWelded.value);
  scrollToHighlight();
}

function choose(index: number) {
  preview(index);
  closeList();
}

/**
 * Keys on the font button: ↑/↓ (PageUp/PageDown: 10 at a time, Home/End) change the font
 * at once, with the list open or not; a letter jumps to the next font starting with it;
 * Enter closes the list; Esc closes it and goes back to the font before.
 */
function onFontKey(event: KeyboardEvent) {
  const moves: Record<string, number> = { ArrowDown: 1, ArrowUp: -1, PageDown: 10, PageUp: -10 };
  const current = listOpen.value ? highlight.value : fonts.value.findIndex((font) => font.name === shownFont.value);
  if (event.key in moves) {
    if (!listOpen.value) openList();
    preview(current + moves[event.key]);
  } else if (event.key === "Home" || event.key === "End") {
    if (!listOpen.value) openList();
    preview(event.key === "Home" ? 0 : fonts.value.length - 1);
  } else if (event.key === "Enter") {
    if (listOpen.value) closeList();
    else openList();
  } else if (event.key === "Escape") {
    if (listOpen.value) {
      const before = fonts.value.findIndex((font) => font.name === fontBefore);
      if (before >= 0) preview(before);
    }
    closeList();
  } else if (event.key.length === 1 && !event.ctrlKey) {
    const letter = event.key.toLowerCase();
    const count = fonts.value.length;
    for (let step = 1; step <= count; step++) {
      const index = (current + step) % count;
      if (fonts.value[index].name.toLowerCase().startsWith(letter)) {
        if (!listOpen.value) openList();
        preview(index);
        break;
      }
    }
  } else {
    return;
  }
  // These keys are the list's: not the canvas's (typed text, Delete…) nor the page's.
  event.preventDefault();
  event.stopPropagation();
}

// A click outside the list closes it.
function onPointerDown(event: PointerEvent) {
  if (listOpen.value && !(event.target as Element).closest(".font-picker")) closeList();
}
onMounted(() => window.addEventListener("pointerdown", onPointerDown));
onUnmounted(() => window.removeEventListener("pointerdown", onPointerDown));

/** The size must be a number above 0; anything else restores the shown size. */
function onSizeChange(event: Event) {
  const input = event.target as HTMLInputElement;
  const size = Number(input.value);
  if (input.value.trim() !== "" && Number.isFinite(size) && size > 0) {
    apply(shownFont.value, size, shownWelded.value);
  }
  input.value = shownSize.value;
}

/**
 * The last size asked with the wheel, and for which text: quick wheel steps start from it,
 * not from the shown size, which changes only once Rust has answered.
 */
let wheelSize: { target: unknown; size: number } | null = null;
/** Forgets `wheelSize` a moment after the last step (the size may then change otherwise). */
let wheelTimer = 0;

/** The mouse wheel over Size: ±1 mm at once on the text (up: bigger), never below 1 mm. */
function onSizeWheel(event: WheelEvent) {
  const target = textEdit.value ?? selectedText.value?.id ?? "next";
  const from = wheelSize?.target === target ? wheelSize.size : Math.round(Number(shownSize.value));
  const size = Math.max(1, from + (event.deltaY < 0 ? 1 : -1));
  wheelSize = { target, size };
  clearTimeout(wheelTimer);
  wheelTimer = window.setTimeout(() => (wheelSize = null), 1000);
  apply(shownFont.value, size, shownWelded.value);
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
    <div class="font-picker">
      <button
        ref="fontButton"
        class="font-button"
        :disabled="fonts.length === 0"
        title="Font of the text being typed or selected, and of the next one; ↑/↓ change it at once, Esc goes back"
        @click="listOpen ? closeList() : openList()"
        @keydown="onFontKey"
      >
        <span>{{ fonts.length === 0 ? "Loading fonts…" : shownFont }}</span>
        <ChevronDown :size="14" />
      </button>
      <ul v-if="listOpen" ref="fontList" class="font-list">
        <!-- mousedown.prevent: the button keeps the focus, so the keys still work. -->
        <li
          v-for="(font, index) in fonts"
          :key="font.name"
          :class="{ active: index === highlight }"
          @mousedown.prevent="choose(index)"
        >
          {{ font.name }}
        </li>
      </ul>
    </div>
    <label>
      Size
      <!-- :value + @change: the text changes on Enter, on leaving the field or on an arrow click.
           step 1: the arrows (and the ↑/↓ keys) go to the next whole mm. -->
      <input
        class="size"
        type="number"
        step="1"
        :value="shownSize"
        title="Height of the capital letters, in mm (arrows, ↑/↓ or mouse wheel: 1 mm steps); applies to the text being typed or selected, and to the next one"
        @change="onSizeChange"
        @keyup.up="onSizeChange"
        @keyup.down="onSizeChange"
        @wheel.prevent="onSizeWheel"
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
.font-button {
  padding: 2px 4px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--button);
  color: inherit;
  font: inherit;
}

.font-picker {
  position: relative;
}

.font-button {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 4px;
  width: 24ch;
  cursor: pointer;
  text-align: left;
}

.font-button span {
  overflow: hidden;
  text-overflow: ellipsis;
}

.font-button:disabled {
  opacity: 0.5;
  cursor: default;
}

.font-list {
  position: absolute;
  top: 100%;
  left: 0;
  z-index: 50;
  min-width: 100%;
  max-height: 320px;
  overflow-y: auto;
  margin: 2px 0 0;
  padding: 2px 0;
  list-style: none;
  background: var(--panel);
  border: 1px solid var(--border-strong);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
}

.font-list li {
  padding: 2px 8px;
  cursor: default;
}

.font-list li:hover {
  background: var(--hover);
}

.font-list li.active {
  background: var(--accent-soft);
}

/* ch = width of one digit: room for "999.9", plus the arrows. */
.size {
  width: calc(5ch + 16px);
  text-align: right;
}
</style>
