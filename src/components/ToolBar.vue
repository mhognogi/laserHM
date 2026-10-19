<script setup lang="ts">
import { computed, onMounted, onUnmounted, type Component } from "vue";
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
} from "lucide-vue-next";
import { canCombine, combine, type BooleanOp } from "../boolean";
import { breakApart, canBreakApart } from "../breakApart";
import { editor, type ToolId } from "../editor";
import { call, fireSettings, status } from "../laser";
import LaserFireIcon from "./LaserFireIcon.vue";

// The tools, modeled after LightBurn. `title` is the explanatory tooltip.
const tools: { id: ToolId; title: string; icon: Component }[] = [
  {
    id: "select",
    title:
      "Select: click a shape's outline to select it (Shift: add, Ctrl: add or remove); " +
      "drag on empty space for a selection rectangle (to the right: shapes inside it, " +
      "to the left: also shapes it touches); drag a selected shape to move " +
      "(Ctrl: only horizontally or vertically); drag a handle to resize " +
      "(Shift: keep proportions, Ctrl: from the center); drag an arrow outside a corner to rotate; " +
      "Del or right-click → Delete removes it; " +
      "click empty space to deselect; Ctrl+Z undoes the last change (up to 100)",
    icon: MousePointer2,
  },
  {
    id: "nodes",
    title:
      "Edit Nodes: click a shape to show its nodes; drag nodes and curve handles; over a node or segment: " +
      "Del/D delete, L straight, C curved, I insert node (or double-click), M insert at the middle, " +
      "B break, S smooth; drag an end of an open line onto the other end to close it; " +
      "rectangles, ellipses and polygons become lines when changed; Esc: done",
    icon: Spline,
  },
  {
    id: "line",
    title:
      "Draw Lines: click to add points (straight segments); click and drag to curve the segment " +
      "ending there; Ctrl: 15° steps; click the first point to close the shape; " +
      "double-click, right-click, Enter or Esc to end an open line",
    icon: PenTool,
  },
  {
    id: "rectangle",
    title: "Rectangle: drag to draw; hold Ctrl for a square, Shift to draw from the center",
    icon: Square,
  },
  {
    id: "ellipse",
    title: "Ellipse: drag to draw; hold Ctrl for a circle, Shift to draw from the center",
    icon: Circle,
  },
  {
    id: "polygon",
    title: "Polygon: drag to draw a hexagon; hold Ctrl for equal sides, Shift to draw from the center",
    icon: Hexagon,
  },
  {
    id: "text",
    title:
      "Text: click on the canvas and type (Enter: new line, Backspace: delete, " +
      "Esc or click elsewhere: done); double-click a text with Select to edit it; " +
      "font and size are in the top bar, after the zoom buttons",
    icon: Type,
  },
  {
    id: "position",
    title:
      "Position Laser: click on the canvas to move the laser head there (laser off, at travel speed); " +
      "needs a connected, idle laser",
    icon: Crosshair,
  },
  {
    id: "measure",
    title:
      "Measure: drag on the canvas from one point to another; the ends stick to nearby nodes and corners; " +
      "distance, dX, dY and angle show in the top bar; Esc clears the measurement",
    icon: Ruler,
  },
];

// Boolean operations on the selected shapes, as in LightBurn; the result replaces them.
// `mirrored`: the icon is flipped left-right and turned 90° clockwise (the reversed Subtract).
const booleanOps: { op: BooleanOp; title: string; icon: Component; mirrored?: boolean }[] = [
  {
    op: "weld",
    title: "Weld: join all selected shapes (2 or more) into one outline, without the lines inside the overlaps",
    icon: Combine,
  },
  {
    op: "subtract",
    title: "Boolean Subtract: remove the second selected shape from the first",
    icon: SquaresSubtract,
  },
  {
    op: "subtractReverse",
    title: "Boolean Subtract (reversed): remove the first selected shape from the second",
    icon: SquaresSubtract,
    mirrored: true,
  },
  {
    op: "intersect",
    title: "Boolean Intersection: keep only the part the 2 selected shapes have in common",
    icon: SquaresIntersect,
  },
];

// ----- Fire: the laser is on only while the button is held -----

const firing = computed(() => status.task === "fire");
/** Usable with a connected, idle laser; while firing it stays enabled so it can be released. */
const canFire = computed(() => (status.connected && !status.busy) || firing.value);

function startFire(event: PointerEvent) {
  if (event.button !== 0 || !canFire.value) return;
  // The release then reaches this button even if the pointer has left it.
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  call("fire", { powerPercent: fireSettings.powerPercent });
}

/** Release, a lost pointer or a window losing focus all turn the laser off. */
function stopFire() {
  call("fire_off");
}

onMounted(() => window.addEventListener("blur", stopFire));
onUnmounted(() => window.removeEventListener("blur", stopFire));
</script>

<template>
  <aside class="tool-bar">
    <template v-for="tool in tools" :key="tool.id">
      <button
        class="tool"
        :class="{ active: tool.id === editor.tool }"
        :title="tool.title"
        @click="editor.tool = tool.id"
      >
        <!-- <component :is> renders the icon component picked from the list. -->
        <component :is="tool.icon" :size="20" />
      </button>
      <!-- Under Position Laser: Fire, an action (not a tool), on only while held. -->
      <button
        v-if="tool.id === 'position'"
        class="tool fire"
        :class="{ firing }"
        :disabled="!canFire"
        :title="`Fire: hold to turn the laser on without moving, at ${fireSettings.powerPercent}% power (set next to the zoom buttons while Position Laser is active); release to turn it off. Wear laser safety glasses`"
        @pointerdown="startFire"
        @pointerup="stopFire"
        @pointercancel="stopFire"
        @lostpointercapture="stopFire"
      >
        <LaserFireIcon :size="20" />
      </button>
      <!-- Under Measure: Break Apart, Weld, Subtract and Intersection, actions (not tools) on the selected shapes. -->
      <template v-if="tool.id === 'measure'">
        <button
          class="tool"
          :disabled="!canBreakApart()"
          title="Break Apart: split each selected text or shape made of separate pieces into one shape per piece (e.g. a welded &quot;Ascd&quot; into &quot;A&quot;, &quot;s&quot; and &quot;cd&quot;); holes stay with their piece"
          @click="breakApart"
        >
          <Ungroup :size="20" />
        </button>
        <button
          v-for="item in booleanOps"
          :key="item.op"
          class="tool"
          :disabled="!canCombine(item.op)"
          :title="item.title"
          @click="combine(item.op)"
        >
          <component :is="item.icon" :size="20" :class="{ mirrored: item.mirrored }" />
        </button>
      </template>
    </template>
  </aside>
</template>

<style scoped>
.tool-bar {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 4px;
  background: var(--panel);
  border-right: 1px solid var(--border);
}

.tool {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  border: 1px solid transparent;
  border-radius: 4px;
  background: none;
  color: inherit;
  cursor: pointer;
}

.tool:hover {
  background: var(--hover);
}

.tool.active {
  background: var(--accent-soft);
  border-color: var(--accent);
}

.tool:disabled {
  opacity: 0.4;
  cursor: default;
}

.tool:disabled:hover {
  background: none;
}

/* Flipped left-right, then turned 90° clockwise. */
.mirrored {
  transform: rotate(90deg) scaleX(-1);
}

/* While the laser is on. */
.tool.fire.firing {
  background: var(--danger);
  color: #ffffff;
}
</style>
