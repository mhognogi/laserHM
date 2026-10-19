// Draw Lines, as in LightBurn: each click adds a point; click and drag makes the
// segment ending there curved; a click on the first point closes the shape; double-click,
// right-click, Enter or Escape ends an open line. Ctrl snaps the segment to 15° steps.
import { ref, watch } from "vue";
import { addShape, editor, type Point } from "./editor";
import { pathShape, type PathNode } from "./paths";

/** Ctrl: the segment's direction snaps to multiples of this angle. */
const SNAP_DEGREES = 15;
/** A drag shorter than this (in mm) is a plain click, not a curve. */
const MIN_DRAG = 1e-3;

/**
 * The line being drawn, or null. `cursor` is where the next point would go (for the
 * preview); `dragging` is true while the button is held after adding a point.
 */
export const pen = ref<{ nodes: PathNode[]; cursor: Point | null; dragging: boolean } | null>(null);

/** `to`, turned around `from` to the nearest multiple of 15° (same distance). */
function snapAngle(from: Point, to: Point): Point {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const step = (SNAP_DEGREES * Math.PI) / 180;
  const angle = Math.round(Math.atan2(dy, dx) / step) * step;
  const length = Math.hypot(dx, dy);
  return { x: from.x + length * Math.cos(angle), y: from.y + length * Math.sin(angle) };
}

/** Where a click at `point` puts the next point: snapped with Ctrl, relative to the last point. */
function nextPoint(point: Point, ctrl: boolean): Point {
  const nodes = pen.value?.nodes;
  const last = nodes?.[nodes.length - 1];
  return ctrl && last ? snapAngle(last.p, point) : point;
}

/**
 * A press with the line tool. Starts a line, adds a point, or (on the first point,
 * with at least 2 points) closes the line and creates the shape.
 */
export function penDown(point: Point, ctrl: boolean, onFirstPoint: boolean) {
  if (!pen.value) {
    pen.value = { nodes: [{ p: point }], cursor: point, dragging: true };
    return;
  }
  if (onFirstPoint && pen.value.nodes.length >= 2) {
    finishPen(true);
    return;
  }
  const p = nextPoint(point, ctrl);
  pen.value.nodes.push({ p });
  pen.value.cursor = p;
  pen.value.dragging = true;
}

/**
 * Pointer moves: while the button is held after adding a point, the drag sets that
 * point's curve handles (symmetric, as in LightBurn); otherwise it moves the preview.
 */
export function penMove(point: Point, ctrl: boolean) {
  const current = pen.value;
  if (!current) return;
  if (current.dragging) {
    const node = current.nodes[current.nodes.length - 1];
    if (Math.hypot(point.x - node.p.x, point.y - node.p.y) < MIN_DRAG) return;
    node.out = point;
    node.in = { x: 2 * node.p.x - point.x, y: 2 * node.p.y - point.y };
    node.smooth = true;
    return;
  }
  current.cursor = nextPoint(point, ctrl);
}

export function penUp() {
  if (pen.value) pen.value.dragging = false;
}

/**
 * Ends the line and adds it as a shape (closed or open). Points repeated in place
 * (e.g. by the clicks of a double-click) count once; fewer than 2 points add nothing.
 */
export function finishPen(closed = false) {
  const current = pen.value;
  pen.value = null;
  if (!current) return;
  const nodes = current.nodes.filter((node, i, all) => {
    const next = all[i + 1];
    return !next || Math.hypot(next.p.x - node.p.x, next.p.y - node.p.y) > MIN_DRAG;
  });
  // The first node has no segment arriving at it in an open line.
  if (!closed && nodes.length > 0) nodes[0] = { ...nodes[0], in: undefined };
  const shape = pathShape(nodes, closed);
  if (shape) addShape(shape);
}

// Choosing another tool ends the line being drawn.
watch(() => editor.tool, () => finishPen());
