<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { calibrating, camera, cancelCalibration, startCalibration } from "../camera";
import { editor, selectionBox } from "../editor";
import { adjustImageId, importImage } from "../images";
import { status } from "../laser";
import { materialTestOpen } from "../materialTest";
import { redo, undo } from "../history";
import { newProject, openProject, saveProject, saveProjectAs } from "../project";
import AboutDialog from "./AboutDialog.vue";
import HowToDialog from "./HowToDialog.vue";
import SafetyDialog from "./SafetyDialog.vue";
import AdjustImageDialog from "./AdjustImageDialog.vue";
import MaterialTestDialog from "./MaterialTestDialog.vue";

/**
 * A dropdown entry: its label, tooltip and what it does.
 * `missing`: what the user must do to enable it (shown in red); null when enabled.
 */
type Item = { name: string; title: string; action: () => void; missing?: string | null; shortcut?: string };

const about = ref(false);
const howTo = ref(false);
const safety = ref(false);

/** The hidden file picker behind File > Import. */
const fileInput = ref<HTMLInputElement>();

/** A picked file goes on the canvas, then Adjust Image opens on it. */
async function onFilePicked() {
  const file = fileInput.value?.files?.[0];
  if (!file) return;
  // Cleared, so picking the same file again still triggers `change`.
  fileInput.value!.value = "";
  try {
    const shape = await importImage(file);
    editor.tool = "select";
    adjustImageId.value = shape.id;
  } catch {
    status.message = `Cannot read the image ${file.name}`;
    status.error = true;
  }
}

// Dropdowns only where there are items; `title` is the explanatory tooltip.
// computed: the Calibrate entry follows the camera and the selection.
const menus = computed<{ name: string; title: string; items?: Item[] }[]>(() => [
  {
    name: "File",
    title: "Projects and images",
    items: [
      { name: "New", shortcut: "Ctrl+N", title: "Start an empty project", action: newProject },
      { name: "Open…", shortcut: "Ctrl+O", title: "Open a laserHM project (.lhm)", action: openProject },
      { name: "Save", shortcut: "Ctrl+S", title: "Save the project to its file", action: saveProject },
      {
        name: "Save As…",
        shortcut: "Ctrl+Shift+S",
        title: "Save the project to a new file",
        action: saveProjectAs,
      },
      {
        name: "Import…",
        shortcut: "Ctrl+I",
        title: "Import an image (PNG, JPG, BMP) to engrave it line by line",
        action: () => fileInput.value?.click(),
      },
    ],
  },
  {
    name: "Edit",
    title: "Undo and redo",
    items: [
      { name: "Undo", shortcut: "Ctrl+Z", title: "Undo the last change", action: undo },
      { name: "Redo", shortcut: "Ctrl+Shift+Z", title: "Do again the last change undone", action: redo },
    ],
  },
  {
    name: "Tools",
    title: "Camera calibration and shape tools",
    items: [
      calibrating.value
        ? { name: "Cancel calibration", title: "Cancel the camera calibration (Esc)", action: cancelCalibration }
        : {
            name: "Calibrate camera",
            title:
              "Calibrate the camera: with the cut square selected (and still on the bed), click 9 of its points in the image; the image is then aligned with the canvas",
            action: startCalibration,
            missing: !camera.on ? "Turn on the camera" : !selectionBox() ? "Draw a square and select it" : null,
          },
      {
        name: "Material Test",
        title: "Burn a grid of cells with different powers and speeds, to find the best settings for a material",
        action: () => (materialTestOpen.value = true),
      },
    ],
  },
  {
    name: "Help",
    title: "App information",
    items: [
      { name: "How to use", title: "Instructions for using the app", action: () => (howTo.value = true) },
      { name: "Safety", title: "Laser safety warnings", action: () => (safety.value = true) },
      { name: "About laserHM", title: "Version and author of the app", action: () => (about.value = true) },
    ],
  },
]);

/** The menu whose dropdown is open, or null. */
const open = ref<string | null>(null);

function choose(item: Item) {
  open.value = null;
  item.action();
}

// A click anywhere else or Esc closes the dropdown.
function onPointerDown(event: PointerEvent) {
  if (!(event.target as Element).closest(".menu-bar")) open.value = null;
}
function onKey(event: KeyboardEvent) {
  if (event.key === "Escape") open.value = null;
  // File shortcuts, except while typing in a field.
  if (!event.ctrlKey || (event.target as Element).closest("input, textarea, select")) return;
  const actions: Record<string, () => void> = {
    i: () => fileInput.value?.click(),
    n: newProject,
    o: openProject,
    s: event.shiftKey ? saveProjectAs : saveProject,
  };
  const action = actions[event.key.toLowerCase()];
  if (action) {
    event.preventDefault();
    action();
  }
}
onMounted(() => {
  window.addEventListener("pointerdown", onPointerDown);
  window.addEventListener("keydown", onKey);
});
onUnmounted(() => {
  window.removeEventListener("pointerdown", onPointerDown);
  window.removeEventListener("keydown", onKey);
});
</script>

<template>
  <nav class="menu-bar">
    <!-- v-for repeats the element for each value in the list. -->
    <div v-for="menu in menus" :key="menu.name" class="menu-wrap">
      <button
        class="menu"
        :class="{ open: open === menu.name }"
        :title="menu.title"
        @click="menu.items && (open = open === menu.name ? null : menu.name)"
        @pointerenter="open && menu.items && (open = menu.name)"
      >
        {{ menu.name }}
      </button>
      <div v-if="open === menu.name && menu.items" class="dropdown">
        <!-- The wrapper gets the hover while the entry is disabled, to show what is missing. -->
        <span v-for="item in menu.items" :key="item.name" class="item">
          <button :disabled="!!item.missing" :title="item.missing ? undefined : item.title" @click="choose(item)">
            {{ item.name }}
            <span v-if="item.shortcut" class="shortcut">{{ item.shortcut }}</span>
          </button>
          <span v-if="item.missing" class="missing">{{ item.missing }}</span>
        </span>
      </div>
    </div>
    <!-- Dialogs inside <nav> (fixed, so they still cover the window): with a single root element the
         class from App.vue (full-width) applies to it. -->
    <AboutDialog v-if="about" @close="about = false" />
    <HowToDialog v-if="howTo" @close="howTo = false" />
    <!-- At the first start (until accepted), and from Help > Safety. -->
    <SafetyDialog
      v-if="safety || status.safetyAccepted === false"
      :first-run="status.safetyAccepted === false"
      @close="safety = false"
    />
    <MaterialTestDialog v-if="materialTestOpen" @close="materialTestOpen = false" />
    <!-- :key: reopened on another image, the dialog starts from that image's settings. -->
    <AdjustImageDialog v-if="adjustImageId !== null" :key="adjustImageId" />
    <input
      ref="fileInput"
      type="file"
      accept=".png,.jpg,.jpeg,.bmp"
      hidden
      @change="onFilePicked"
    />
  </nav>
</template>

<style scoped>
.menu-bar {
  display: flex;
  background: var(--panel);
  border-bottom: 1px solid var(--border);
}

.menu-wrap {
  position: relative;
}

.menu {
  padding: 4px 10px;
  border: none;
  background: none;
  color: inherit;
  font: inherit;
  cursor: default;
}

.menu:hover,
.menu.open {
  background: var(--hover);
}

.dropdown {
  position: absolute;
  top: 100%;
  left: 0;
  z-index: 50;
  display: flex;
  flex-direction: column;
  min-width: 140px;
  padding: 2px 0;
  background: var(--panel);
  border: 1px solid var(--border-strong);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
}

.item {
  position: relative;
  display: flex;
}

.dropdown button {
  flex: 1;
  padding: 4px 12px;
  border: none;
  background: none;
  color: inherit;
  font: inherit;
  text-align: left;
  white-space: nowrap;
  cursor: default;
}

/* The shortcut, at the right of the entry. */
.dropdown button {
  display: flex;
  justify-content: space-between;
  gap: 24px;
}

.shortcut {
  color: var(--muted);
}

.dropdown button:hover:enabled {
  background: var(--hover);
}

/* A disabled entry: greyed, and no mouse events, so the hover reaches the wrapper. */
.dropdown button:disabled {
  color: var(--muted);
  pointer-events: none;
}

/* What is missing, in red, to the right of the disabled entry. */
.missing {
  display: none;
  position: absolute;
  top: 0;
  left: calc(100% + 4px);
  padding: 4px 6px;
  border: 1px solid var(--danger);
  border-radius: 4px;
  background: var(--panel);
  color: var(--danger);
  white-space: nowrap;
}

.item:hover .missing {
  display: block;
}
</style>
