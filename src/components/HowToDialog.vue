<script setup lang="ts">
// Help > How to use: a large popup with instructions, one section per topic.
import { onMounted, onUnmounted, type Component } from "vue";
import {
  Circle,
  Combine,
  Crosshair,
  Hexagon,
  MousePointer2,
  PenTool,
  Ruler,
  Spline,
  Square,
  SquaresIntersect,
  SquaresSubtract,
  Type,
  Ungroup,
  X,
} from "lucide-vue-next";
import LaserFireIcon from "./LaserFireIcon.vue";

/** The tool bar's buttons, top to bottom, with the same icons. `mirrored`: as in the tool bar. */
const TOOLS: { name: string; icon: Component; mirrored?: boolean; text: string[] }[] = [
  {
    name: "Select",
    icon: MousePointer2,
    text: [
      "Click a shape to select it. Shift+click adds a shape. Ctrl+click adds or removes a shape.",
      "Drag on an empty area to select the shapes in a rectangle.",
      "Drag a selected shape to move it. Drag a small square to resize it.",
      "Drag a round arrow near a corner to rotate it. Push Del to delete it.",
      "Double-click a text to edit it.",
    ],
  },
  {
    name: "Edit Nodes",
    icon: Spline,
    text: [
      "Click a shape to show its nodes. Drag a node or a curve handle to change the shape.",
      "Over a node or a segment, push: D delete, L straight, C curve, I insert a node, M insert at the middle, B break, S smooth.",
      "Rectangles, ellipses and polygons become lines when you change them. Push Esc when you are done.",
    ],
  },
  {
    name: "Draw Lines",
    icon: PenTool,
    text: [
      "Click to add points. Click and drag to make a curve. Hold Ctrl for steps of 15°.",
      "Click the first point to close the shape. Double-click, right-click or push Esc to stop an open line.",
    ],
  },
  {
    name: "Rectangle",
    icon: Square,
    text: ["Drag to draw. Hold Ctrl for a square. Hold Shift to draw from the center."],
  },
  {
    name: "Ellipse",
    icon: Circle,
    text: ["Drag to draw. Hold Ctrl for a circle. Hold Shift to draw from the center."],
  },
  {
    name: "Polygon",
    icon: Hexagon,
    text: ["Drag to draw a hexagon. Hold Ctrl for equal sides. Hold Shift to draw from the center."],
  },
  {
    name: "Text",
    icon: Type,
    text: [
      "Click on the canvas and type. Push Enter for a new line. Push Esc when you are done.",
      "Set the font and the size in the top bar.",
    ],
  },
  {
    name: "Position Laser",
    icon: Crosshair,
    text: [
      "Click on the canvas. The laser head moves to this point with the laser off.",
      "The laser must be connected and not busy.",
    ],
  },
  {
    name: "Fire",
    icon: LaserFireIcon,
    text: [
      "Hold the button to turn on the laser without movement. Release it to turn the laser off.",
      "Set the power in the Fire field of the top bar. Wear safety glasses.",
    ],
  },
  {
    name: "Measure",
    icon: Ruler,
    text: [
      "Drag from one point to another. The ends attach to near nodes and corners.",
      "The top bar shows the distance, dX, dY and the angle. Push Esc to clear the measurement.",
    ],
  },
  {
    name: "Break Apart",
    icon: Ungroup,
    text: ["Splits each selected text or shape with separate pieces into one shape for each piece."],
  },
  {
    name: "Weld",
    icon: Combine,
    text: ["Joins 2 or more selected shapes into one outline. The lines inside the overlaps go away."],
  },
  {
    name: "Subtract",
    icon: SquaresSubtract,
    text: ["Removes the second selected shape from the first."],
  },
  {
    name: "Subtract (reversed)",
    icon: SquaresSubtract,
    mirrored: true,
    text: ["Removes the first selected shape from the second."],
  },
  {
    name: "Intersection",
    icon: SquaresIntersect,
    text: ["Keeps only the part that the 2 selected shapes have in common."],
  },
];

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
        <h2>Tools</h2>
        <!-- One row per tool: its icon, as in the tool bar on the left, then what it does. -->
        <div v-for="tool in TOOLS" :key="tool.name" class="tool">
          <component :is="tool.icon" :size="20" :class="{ mirrored: tool.mirrored }" />
          <div>
            <strong>{{ tool.name }}</strong>
            <p v-for="line in tool.text" :key="line">{{ line }}</p>
          </div>
        </div>

        <h2>Camera calibration</h2>
        <p>
          After the calibration, the camera image matches the work area. LaserHM corrects each quarter of the
          square separately, so the image is exact inside the square.
        </p>
        <ol>
          <li>Draw a large square. Cut it in a sheet of material. Do not move the material.</li>
          <li>In the top bar, click the camera button. The canvas shows the camera image.</li>
          <li>Select the square that you cut.</li>
          <li>Click <strong>Tools</strong> &gt; <strong>Calibrate camera</strong>. The "Camera calibration" window opens.</li>
          <li>
            In the camera image, click 9 points of the cut square in this order: the 4 corners (bottom-left,
            bottom-right, top-right, top-left), the middles of the 4 sides (bottom, right, top, left), the center.
          </li>
        </ol>
        <p>
          LaserHM aligns the camera image with the canvas and keeps the calibration. To cancel the calibration,
          push Esc.
        </p>
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

.tool {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 4px 0;
  border-bottom: 1px solid var(--border);
}

.tool p {
  margin: 0;
}

.content > p {
  margin: 4px 0;
}

ol {
  margin: 0;
  padding-left: 20px;
}

li + li {
  margin-top: 4px;
}

/* As in the tool bar: the reversed Subtract's icon is mirrored and turned. */
.mirrored {
  transform: rotate(90deg) scaleX(-1);
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
