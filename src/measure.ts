// Measure: drag on the canvas to measure from one point to another. The ends stick
// to the nodes and corners of shapes near the pointer. The measurement stays until
// the next one, Escape or another tool; its values show in the top bar.
import { computed, ref, watch } from "vue";
import { editor, type Point } from "./editor";
import { machineNodes } from "./paths";

/** The measured line (machine coordinates, mm); `dragging` while the button is held. */
export const measurement = ref<{ start: Point; end: Point; dragging: boolean } | null>(null);

/** Distance, horizontal and vertical difference (mm) and direction (degrees, counterclockwise). */
export const measured = computed(() => {
  const m = measurement.value;
  if (!m) return null;
  const dx = m.end.x - m.start.x;
  const dy = m.end.y - m.start.y;
  return { distance: Math.hypot(dx, dy), dx, dy, angle: (Math.atan2(dy, dx) * 180) / Math.PI };
});

/**
 * The nodes and corners of all shapes: rectangle and polygon corners, the 4 ends of an
 * ellipse's axes, a line's nodes. Texts have too many points to be useful and are skipped.
 */
function snapPoints(): Point[] {
  return editor.shapes.flatMap((shape) => machineNodes(shape)?.nodes.map((node) => node.p) ?? []);
}

/** `point`, moved onto the nearest node or corner within `tolerance` mm, if there is one. */
function snap(point: Point, tolerance: number): Point {
  let best = point;
  let bestDistance = tolerance;
  for (const candidate of snapPoints()) {
    const d = Math.hypot(candidate.x - point.x, candidate.y - point.y);
    if (d <= bestDistance) {
      best = candidate;
      bestDistance = d;
    }
  }
  return { ...best };
}

export function startMeasure(point: Point, tolerance: number) {
  const start = snap(point, tolerance);
  measurement.value = { start, end: { ...start }, dragging: true };
}

export function measureTo(point: Point, tolerance: number) {
  if (measurement.value?.dragging) measurement.value.end = snap(point, tolerance);
}

export function endMeasure() {
  if (measurement.value) measurement.value.dragging = false;
}

export function clearMeasure() {
  measurement.value = null;
}

// Choosing another tool removes the measurement.
watch(() => editor.tool, clearMeasure);
