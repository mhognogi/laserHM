<script setup lang="ts">
// Instructions for the camera calibration, over the canvas; drag its title bar to move it.
import { ref } from "vue";
import { X } from "lucide-vue-next";
import { CALIBRATION_POINTS, calibrating } from "../camera";

const emit = defineEmits<{ close: [] }>();

/** The picture is shown at its full size (the popup widens). */
const enlarged = ref(false);

/** Position inside the work area, in pixels from its top-left. */
const position = ref({ x: 12, y: 12 });

/** The move in progress: where the pointer was and where the popup was when it started. */
let move: { x: number; y: number; left: number; top: number } | null = null;

function startMove(event: PointerEvent) {
  if (event.button !== 0 || (event.target as Element).closest("button")) return;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  move = { x: event.clientX, y: event.clientY, left: position.value.x, top: position.value.y };
}

function moveTo(event: PointerEvent) {
  if (!move) return;
  position.value = {
    x: Math.max(0, move.left + event.clientX - move.x),
    y: Math.max(0, move.top + event.clientY - move.y),
  };
}

function endMove() {
  move = null;
}
</script>

<template>
  <div
    class="popup"
    :class="{ enlarged }"
    :style="{ left: `${position.x}px`, top: `${position.y}px` }"
  >
    <div
      class="title"
      title="Drag to move these instructions"
      @pointerdown="startMove"
      @pointermove="moveTo"
      @pointerup="endMove"
      @lostpointercapture="endMove"
    >
      Camera calibration
      <button title="Close these instructions (the calibration continues)" @click="emit('close')">
        <X :size="14" />
      </button>
    </div>
    <ol>
      <li>
        Cut the square as large as possible, close to the work area's size: the calibration is exact only
        inside it.
      </li>
      <li>
        The cut square stays on the bed exactly where it was cut, and is selected on the canvas. Mark the
        middles of its sides and its center so they can be seen in the image.
      </li>
      <li>
        Click 9 of its points in the camera image, in this order: the corners (bottom-left, bottom-right,
        top-right, top-left), the middles of the sides (bottom, right, top, left), the center.
        <div v-if="calibrating" class="next">
          Next: {{ CALIBRATION_POINTS[calibrating.image.length].name }} ({{ calibrating.image.length + 1 }}/{{
            CALIBRATION_POINTS.length
          }})
        </div>
      </li>
      <li>Click the middle of the cut line; zoom in with the mouse wheel for precision.</li>
      <li>After the last click the image is aligned with the canvas and the calibration is saved.</li>
    </ol>
    <!-- From public/: served at the app's root. -->
    <img
      src="/calibration-3x3.svg"
      alt="The 9 points in click order, and how each quarter of the square is aligned"
      :title="enlarged ? 'Click to make the picture small again' : 'Click to enlarge the picture'"
      @click="enlarged = !enlarged"
    />
    <div class="note">Esc or the calibrate button cancels.</div>
  </div>
</template>

<style scoped>
.popup {
  position: absolute;
  z-index: 10;
  width: 280px;
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
  cursor: move;
  user-select: none;
}

.title button {
  display: grid;
  place-items: center;
  padding: 2px;
  border: none;
  border-radius: 4px;
  background: none;
  color: inherit;
  cursor: pointer;
}

.title button:hover {
  background: var(--hover);
}

ol {
  margin: 0;
  padding: 4px 8px 0 24px;
}

li + li {
  margin-top: 2px;
}

/* Full size: the picture is 880 px wide. */
.popup.enlarged {
  width: 882px;
}

img {
  display: block;
  width: 100%;
  margin-top: 4px;
  border-top: 1px solid var(--border);
  cursor: zoom-in;
}

.popup.enlarged img {
  cursor: zoom-out;
}

.next {
  color: var(--danger);
  font-weight: 600;
}

.note {
  padding: 2px 8px 4px;
  color: var(--muted);
}
</style>
