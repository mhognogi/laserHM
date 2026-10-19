<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { call, status } from "../laser";
import {
  addShape,
  boxOf,
  clearSelection,
  copySelected,
  deleteSelected,
  DRAWING_TOOLS,
  editor,
  HANDLES,
  isSelected,
  selectMany,
  shapesInRect,
  moveGroup,
  paste,
  resizeFromHandle,
  sizeLocked,
  restore,
  rotateGroup,
  rotateVector,
  select,
  selectionBox,
  shapeFromDrag,
  snapshotSelected,
  svgPath,
  transformGroup,
  type Box,
  type ShapeType,
  type Handle,
  type Point,
  type Snapshot,
} from "../editor";
import { MARGIN_PX, origin, scale, svgSize, view, zoomAt, zoomToFit } from "../view";
import { editText, finishText, loadFonts, startText, textEdit, typeKey } from "../text";
import { adjustImageId, previewUrl } from "../images";
import { finishPen, pen, penDown, penMove, penUp } from "../pen";
import { redo, undo } from "../history";
import { clearMeasure, endMeasure, measurement, measureTo, startMeasure } from "../measure";
import {
  addCalibrationPoint,
  calibrating,
  calibrationHelp,
  camera,
  cameraPieces,
  cameraStream,
  cancelCalibration,
} from "../camera";
import { flatten, segment } from "../paths";
import {
  cancelNodeDrag,
  endNodeDrag,
  endNodeEdit,
  hitTest,
  hover as nodeHover,
  insertNode,
  isNodeDragging,
  nodeDragTo,
  nodeEdit,
  nodeKey,
  startNodeDrag,
  startNodeEdit,
} from "../nodes";
import WorkAreaForm from "./WorkAreaForm.vue";
import CalibrationPopup from "./CalibrationPopup.vue";

/** Distance between grid lines, in mm. */
const GRID_STEP = 10;
/** Every this many mm a grid line is emphasized. */
const MAJOR_STEP = 50;

// `computed`: a value derived from other tracked values; Vue recalculates it
// automatically when `status.workArea` changes (e.g. when the laser connects).
const gridLines = computed(() => {
  const area = status.workArea;
  if (!area) return [];
  const lines = [];
  for (let x = GRID_STEP; x < area.width; x += GRID_STEP) {
    lines.push({ x1: x, y1: 0, x2: x, y2: area.height, major: x % MAJOR_STEP === 0 });
  }
  // Y is counted from the origin at the bottom, like the axis labels;
  // SVG's Y grows downwards, so the line for machine Y `y` sits at height - y.
  for (let y = GRID_STEP; y < area.height; y += GRID_STEP) {
    const svgY = area.height - y;
    lines.push({ x1: 0, y1: svgY, x2: area.width, y2: svgY, major: y % MAJOR_STEP === 0 });
  }
  return lines;
});

// ----- View: zoom and pan (the shared state and math live in view.ts) -----

/** Label font size in screen pixels. */
const FONT_PX = 12;

/** Template ref: Vue puts the <svg ref="svg"> element here once it is rendered. */
const svg = ref<SVGSVGElement>();

// Keep view.ts informed of the <svg> size on screen.
// `watch` runs when `svg` changes: the <svg> appears only once a work area exists.
watch(svg, (element, _old, onCleanup) => {
  if (!element) return;
  const observer = new ResizeObserver(([entry]) => {
    svgSize.value = { width: entry.contentRect.width, height: entry.contentRect.height };
  });
  observer.observe(element);
  // Stop observing when the <svg> is removed.
  onCleanup(() => observer.disconnect());
});

/** The <video> copies of the camera overlay (see cameraPieces); each shows the camera's stream. */
const videos = new Set<HTMLVideoElement>();

/** Template ref of each copy: called when it appears (element) and disappears (null). */
function videoRef(element: unknown) {
  if (!(element instanceof HTMLVideoElement)) return;
  videos.add(element);
  if (element.srcObject !== cameraStream.value) element.srcObject = cameraStream.value;
}

watch(cameraStream, (stream) => {
  for (const element of videos) {
    // Forget copies no longer on the page.
    if (!element.isConnected) videos.delete(element);
    else element.srcObject = stream;
  }
});

/** The label font size in mm, so it is always FONT_PX on screen. */
const fontSize = computed(() => FONT_PX / scale.value);

/** viewBox covering the whole <svg>, so 1 mm is exactly `scale` pixels. */
const viewBox = computed(() => {
  const area = status.workArea;
  const { width, height } = svgSize.value;
  if (!area) return "";
  if (width === 0) return `0 0 ${area.width} ${area.height}`;
  const s = scale.value;
  return `${-origin.value.x / s} ${-origin.value.y / s} ${width / s} ${height / s}`;
});

/** Mouse wheel: zoom in or out, keeping the point under the cursor in place. */
function onWheel(event: WheelEvent) {
  const bounds = svg.value!.getBoundingClientRect();
  zoomAt(event.clientX - bounds.left, event.clientY - bounds.top, Math.exp(-event.deltaY * 0.0015));
}

/** "Zoom to fit" in the right-click menu. */
function onZoomToFit() {
  menu.value = null;
  zoomToFit();
}

/** The pan in progress (middle mouse button): where it started, and the pan at that moment. */
const pan = ref<{ x: number; y: number; panX: number; panY: number } | null>(null);

function startPan(event: PointerEvent) {
  svg.value!.setPointerCapture(event.pointerId);
  pan.value = { x: event.clientX, y: event.clientY, panX: view.value.panX, panY: view.value.panY };
}

function panView(event: PointerEvent) {
  if (!pan.value) return;
  view.value.panX = pan.value.panX + event.clientX - pan.value.x;
  view.value.panY = pan.value.panY + event.clientY - pan.value.y;
}

/** Stops the browser's auto-scroll mode, which the middle button would otherwise start. */
function onMouseDown(event: MouseEvent) {
  if (event.button === 1) event.preventDefault();
}

// ----- Axes: rulers with labels -----

/** Axis labels are at least this far apart on screen, in pixels. */
const MIN_LABEL_SPACING_PX = 80;
/** Possible distances between axis labels, in mm. */
const LABEL_STEPS = [1, 2, 5, 10, 25, 50, 100, 200];

/** The part of the SVG visible on screen, in SVG units (mm, Y down). */
const visible = computed(() => {
  const s = scale.value;
  const { width, height } = svgSize.value;
  return {
    left: -origin.value.x / s,
    right: (width - origin.value.x) / s,
    top: -origin.value.y / s,
    bottom: (height - origin.value.y) / s,
  };
});

/**
 * Where the axes are drawn, in SVG units: along the bed's bottom and left edges,
 * but kept on screen (like a ruler) when those edges are zoomed or panned out of view.
 */
const axes = computed(() => {
  const area = status.workArea;
  if (!area) return { x: 0, y: 0 };
  const s = scale.value;
  return {
    y: Math.min(area.height, visible.value.bottom - MARGIN_PX.bottom / s),
    x: Math.max(0, visible.value.left + MARGIN_PX.left / s),
  };
});

/** The ruler strips behind the labels; they hide the bed under them when zoomed in. */
const rulers = computed(() => {
  const s = scale.value;
  const v = visible.value;
  return {
    x: { x: v.left, y: axes.value.y, width: v.right - v.left, height: MARGIN_PX.bottom / s },
    y: { x: axes.value.x - MARGIN_PX.left / s, y: v.top, width: MARGIN_PX.left / s, height: v.bottom - v.top },
  };
});

/** Half the size of the crosses on the corners clicked while calibrating the camera, in screen pixels. */
const CALIBRATION_MARK_PX = 8;

/** The crosses on the clicked corners, as SVG paths (SVG units, Y down). */
const calibrationMarks = computed(() => {
  const area = status.workArea;
  if (!area || !calibrating.value) return [];
  const r = CALIBRATION_MARK_PX / scale.value;
  return calibrating.value.image.map((p) => {
    const [x, y] = [p.x * area.width, p.y * area.height];
    return `M${x - r} ${y}H${x + r}M${x} ${y - r}V${y + r}`;
  });
});

/** Length of the grid's marks on the axes (camera on), in screen pixels. */
const TICK_PX = 4;
const MAJOR_TICK_PX = 8;

/**
 * With the camera on, the grid would hide the image: it shows only as marks on the
 * axes, at the grid lines' places (longer at the emphasized ones), pointing into the bed.
 */
const axisTicks = computed(() => {
  const area = status.workArea;
  if (!area || !camera.on) return [];
  const s = scale.value;
  const { x, y } = axes.value;
  const major = (value: number) => value % MAJOR_STEP === 0;
  const length = (value: number) => (major(value) ? MAJOR_TICK_PX : TICK_PX) / s;
  const ticks = [];
  for (let value = GRID_STEP; value < area.width; value += GRID_STEP) {
    ticks.push({ x1: value, y1: y, x2: value, y2: y - length(value), major: major(value) });
  }
  // Machine Y = height - SVG Y.
  for (let value = GRID_STEP; value < area.height; value += GRID_STEP) {
    const svgY = area.height - value;
    ticks.push({ x1: x, y1: svgY, x2: x + length(value), y2: svgY, major: major(value) });
  }
  return ticks;
});

/** The axis lines, over the part of the bed they measure. */
const axisLines = computed(() => {
  const area = status.workArea;
  if (!area) return null;
  const v = visible.value;
  return {
    x: { x1: Math.max(0, v.left), y1: axes.value.y, x2: Math.min(area.width, v.right), y2: axes.value.y },
    y: { x1: axes.value.x, y1: Math.max(0, v.top), x2: axes.value.x, y2: Math.min(area.height, v.bottom) },
  };
});

/**
 * Segments on the axes showing the extent of the selection (all selected shapes),
 * in SVG coordinates, clamped to the bed; null when nothing is selected.
 */
const axisShadows = computed(() => {
  const area = status.workArea;
  const box = selectionBox();
  if (!area || !box) return null;
  const clamp = (value: number, max: number) => Math.min(Math.max(value, 0), max);
  const left = clamp(box.left, area.width);
  const right = clamp(box.right, area.width);
  const bottom = clamp(box.bottom, area.height);
  const top = clamp(box.top, area.height);
  return {
    x: { x1: left, y1: axes.value.y, x2: right, y2: axes.value.y },
    // SVG's Y grows downwards.
    y: { x1: axes.value.x, y1: area.height - top, x2: axes.value.x, y2: area.height - bottom },
  };
});

/**
 * Labels for the visible part of the bed (origin bottom-left). The step grows
 * or shrinks with the zoom so labels stay at least MIN_LABEL_SPACING_PX apart.
 */
const axisLabels = computed(() => {
  const area = status.workArea;
  if (!area) return { x: [], y: [] };
  const step =
    LABEL_STEPS.find((candidate) => candidate * scale.value >= MIN_LABEL_SPACING_PX) ??
    LABEL_STEPS[LABEL_STEPS.length - 1];
  const values = (from: number, to: number) => {
    const result = [];
    for (let value = Math.ceil(Math.max(from, 0) / step) * step; value <= to; value += step) {
      result.push(value);
    }
    return result;
  };
  const v = visible.value;
  const x = values(v.left, Math.min(v.right, area.width));
  // Machine Y = height - SVG Y.
  let y = values(area.height - v.bottom, Math.min(area.height - v.top, area.height));
  // The origin gets a single "0": skip the Y one when the X axis already shows it.
  if (x.includes(0)) y = y.filter((value) => value !== 0);
  return {
    x: x.map((value) => ({ value, x: value })),
    y: y.map((value) => ({ value, y: area.height - value })),
  };
});

// ----- Drawing -----

/**
 * The drag in progress, or null when not drawing. `regular` (Ctrl): circle, square
 * or equal-sided polygon; `fromCenter` (Shift): the drag starts at the center.
 */
const drag = ref<{
  type: ShapeType;
  start: Point;
  end: Point;
  regular: boolean;
  fromCenter: boolean;
} | null>(null);

/** The shape being drawn, recalculated on every pointer move. */
const preview = computed(() => {
  const d = drag.value;
  return d && { type: d.type, ...shapeFromDrag(d.type, d.start, d.end, d.regular, d.fromCenter) };
});

/** Tooltip of an image with the Select tool. */
const IMAGE_TITLE = "Click to select, drag to move; double-click to adjust the image (mode, contrast, power, speed)";

/** Tooltip of every shape's outline with the Select tool. */
const SHAPE_TITLE =
  "Click to select, drag to move; Shift+click: add to the selection, Ctrl+click: add or remove";

/** True while a drawing tool (Ellipse, Rectangle, Polygon) is active. */
/** True while a tool that clicks points on the canvas is active (crosshair cursor). */
const drawingTool = computed(
  () =>
    !!DRAWING_TOOLS[editor.tool] ||
    editor.tool === "line" ||
    editor.tool === "position" ||
    editor.tool === "measure",
);

/** Converts a pointer position on screen to machine coordinates (mm, Y up). */
function toMachine(event: PointerEvent): Point {
  // getScreenCTM maps SVG units to screen pixels; its inverse maps back to mm.
  const matrix = svg.value!.getScreenCTM()!.inverse();
  const p = new DOMPoint(event.clientX, event.clientY).matrixTransform(matrix);
  // SVG's Y grows downwards, the machine's upwards.
  return { x: p.x, y: status.workArea!.height - p.y };
}

// ----- Moving -----

/** The box around all selected shapes, for the template; null when nothing is selected. */
const selection = computed(() => selectionBox());

/** The move in progress: where the pointer started, and the selected shapes at that moment. */
const move = ref<{ start: Point; snapshot: Snapshot } | null>(null);

// ----- Rotating -----

/** Rotate arrows: how far outside the corners they sit, their radius and arrowhead size, in screen pixels. */
const ROTATE_OFFSET_PX = 8;
const ROTATE_RADIUS_PX = 9;
const ARROWHEAD_PX = 4;

/**
 * A curved arrow outside the corner (hx, hy) of `box`: an arc around the corner's
 * diagonal with an arrowhead at each end, as an SVG path in machine coordinates.
 */
function rotateArrow(box: Box, hx: number, hy: number): string {
  const s = scale.value;
  const corner = { x: hx === 1 ? box.right : box.left, y: hy === 1 ? box.top : box.bottom };
  // The arc's center, out along the corner's diagonal.
  const offset = (ROTATE_OFFSET_PX / Math.SQRT2) / s;
  const center = { x: corner.x + hx * offset, y: corner.y + hy * offset };
  const radius = ROTATE_RADIUS_PX / s;
  const head = ARROWHEAD_PX / s;
  const base = Math.atan2(hy, hx);
  const span = (50 * Math.PI) / 180;
  const at = (angle: number, r = radius) => ({ x: center.x + r * Math.cos(angle), y: center.y + r * Math.sin(angle) });
  const arc = Array.from({ length: 11 }, (_, i) => at(base - span + (2 * span * i) / 10));
  // Arrowhead at an end of the arc: two short lines back from the tip, one on each side.
  const arrowhead = (angle: number, forward: number) => {
    const tip = at(angle);
    const back = { x: -forward * -Math.sin(angle) * head, y: -forward * Math.cos(angle) * head };
    const side = { x: Math.cos(angle) * head * 0.7, y: Math.sin(angle) * head * 0.7 };
    const p1 = { x: tip.x + back.x + side.x, y: tip.y + back.y + side.y };
    const p2 = { x: tip.x + back.x - side.x, y: tip.y + back.y - side.y };
    return `M${p1.x} ${p1.y}L${tip.x} ${tip.y}L${p2.x} ${p2.y}`;
  };
  return (
    "M" + arc.map((p) => `${p.x} ${p.y}`).join("L") + arrowhead(base + span, 1) + arrowhead(base - span, -1)
  );
}

/** The rotation in progress: the center it turns around, the pointer's starting angle, the shapes before. */
const rotation = ref<{ pivot: Point; startAngle: number; snapshot: Snapshot } | null>(null);

/** Direction from `pivot` to `point`, in degrees counterclockwise. */
function angleFrom(pivot: Point, point: Point): number {
  return (Math.atan2(point.y - pivot.y, point.x - pivot.x) * 180) / Math.PI;
}

/** Starts turning the selection around the center of its box. */
function startRotate(event: PointerEvent) {
  const box = selectionBox();
  if (!box) return;
  svg.value!.setPointerCapture(event.pointerId);
  const pivot = { x: (box.left + box.right) / 2, y: (box.bottom + box.top) / 2 };
  rotation.value = { pivot, startAngle: angleFrom(pivot, toMachine(event)), snapshot: snapshotSelected() };
}

/** The shapes turn by as much as the pointer has turned around the center. */
function rotateSelection(event: PointerEvent) {
  if (!rotation.value) return;
  const { pivot, startAngle, snapshot } = rotation.value;
  rotateGroup(snapshot, pivot, angleFrom(pivot, toMachine(event)) - startAngle);
}

/** Escape during a rotation turns the shapes back. */
function cancelRotate() {
  if (rotation.value) restore(rotation.value.snapshot);
  rotation.value = null;
}

// ----- Resizing -----

/** Handle size in screen pixels. */
const HANDLE_PX = 7;

/** The resize in progress: the dragged handle, the group box and the shapes before the resize. */
const resize = ref<{ handle: Handle; box: Box; snapshot: Snapshot } | null>(null);

/** The selection's bounding box and its 8 handles, in machine coordinates. */
const selectionFrame = computed(() => {
  const box = selection.value;
  if (!box || editor.tool !== "select") return null;
  const size = HANDLE_PX / scale.value;
  const cx = (box.left + box.right) / 2;
  const cy = (box.bottom + box.top) / 2;
  const rx = (box.right - box.left) / 2;
  const ry = (box.top - box.bottom) / 2;
  // No resize handles while a fixed-size shape (Material Test) is selected.
  const handles = (sizeLocked() ? [] : HANDLES).map((handle, index) => ({
    index,
    x: cx + handle.hx * rx - size / 2,
    y: cy + handle.hy * ry - size / 2,
    size,
    cursor: handleCursor(handle),
    title: handleTitle(handle),
  }));
  // A rotate arrow outside each corner, shown together with the resize handles.
  const rotateArrows = HANDLES.filter(({ hx, hy }) => hx !== 0 && hy !== 0).map(({ hx, hy }) => ({
    key: `${hx}${hy}`,
    d: rotateArrow(box, hx, hy),
  }));
  // A 1px line with 4px dashes, converted to mm (so both scale the same way).
  return { box, handles, rotateArrows, line: 1 / scale.value, dash: 4 / scale.value };
});

/** The mouse cursor showing the direction a handle resizes in. */
function handleCursor({ hx, hy }: Handle): string {
  if (hx === 0) return "ns-resize";
  if (hy === 0) return "ew-resize";
  // Y is up in machine coordinates: top-right and bottom-left go along "/".
  return hx === hy ? "nesw-resize" : "nwse-resize";
}

function handleTitle({ hx, hy }: Handle): string {
  if (hx === 0) return "Drag to change the height (Ctrl: from the center)";
  if (hy === 0) return "Drag to change the width (Ctrl: from the center)";
  return "Drag to resize (Shift: keep proportions, Ctrl: from the center)";
}

const ROTATE_ARROW_TITLE = "Drag to rotate the selection around its center";

function startResize(event: PointerEvent, handleIndex: number) {
  const box = selectionBox();
  if (!box) return;
  svg.value!.setPointerCapture(event.pointerId);
  resize.value = { handle: HANDLES[handleIndex], box, snapshot: snapshotSelected() };
}

/** Resizes the group box like a single shape, then fits every selected shape into it. */
function resizeSelected(event: PointerEvent) {
  if (!resize.value) return;
  const { handle, box, snapshot } = resize.value;
  const resized = resizeFromHandle(box, handle, toMachine(event), event.shiftKey, event.ctrlKey);
  transformGroup(snapshot, box, boxOf(resized));
}

/** Escape during a resize restores the original sizes and positions. */
function cancelResize() {
  if (resize.value) restore(resize.value.snapshot);
  resize.value = null;
}

function onPointerDown(event: PointerEvent) {
  // Middle button: pan the view, whatever the tool.
  if (event.button === 1) {
    startPan(event);
    return;
  }
  // Any other click on the canvas ends the typing of a text.
  finishText();
  // Right-click while drawing a line ends it (open), instead of opening the menu.
  if (event.button === 2 && pen.value) {
    finishPen();
    skipMenu = true;
    return;
  }
  if (event.button !== 0) return;
  // Camera calibration: each click marks the next corner of the cut square in the image.
  if (calibrating.value) {
    addCalibrationPoint(toMachine(event));
    return;
  }
  const target = event.target as Element;
  // closest: the clicked element itself or its nearest parent with the attribute.
  const handle = target.closest("[data-handle]") as SVGElement | null;
  const hit = target.closest("[data-shape-id]") as SVGElement | null;
  const hitId = hit ? Number(hit.dataset.shapeId) : null;
  // On the selection: one of its handles, its inside, or a selected shape.
  const onSelection =
    !!handle || !!target.closest("[data-selection]") || (hitId !== null && isSelected(hitId));
  const modifier = event.shiftKey || event.ctrlKey;

  if (editor.tool === "select") {
    if (target.closest("[data-rotate]")) startRotate(event);
    else if (handle) startResize(event, Number(handle.dataset.handle));
    else selectAndStartMove(event, hitId, onSelection);
    return;
  }
  // Measure: drag from one point to another; the selection stays.
  if (editor.tool === "measure") {
    svg.value!.setPointerCapture(event.pointerId);
    startMeasure(toMachine(event), nodeTolerance());
    return;
  }
  // Position Laser: the head moves to the click (laser off); Rust refuses it while
  // the laser is not connected, busy, or the point is outside its travel.
  // The selection stays, ready for Start.
  if (editor.tool === "position") {
    const { x, y } = toMachine(event);
    call("move_to", { x, y });
    return;
  }
  // With the other tools, a plain click outside the selection deselects it.
  if (!onSelection && !modifier) clearSelection();
  // Text tool: a click starts typing a new text there (its first line starts at the click).
  if (editor.tool === "text") {
    startText(toMachine(event));
    return;
  }
  // Edit Nodes: drag a node or handle of the edited shape, or click a shape to edit it.
  if (editor.tool === "nodes") {
    onNodesPointerDown(event, hitId);
    return;
  }
  // Draw Lines: each press adds a point (dragging then curves the segment ending there).
  if (editor.tool === "line") {
    svg.value!.setPointerCapture(event.pointerId);
    const point = toMachine(event);
    penDown(point, event.ctrlKey, isOnFirstPoint(point));
    return;
  }
  const type = DRAWING_TOOLS[editor.tool];
  if (!type) return;
  // Keep receiving moves even if the pointer leaves the work area while dragging.
  svg.value!.setPointerCapture(event.pointerId);
  const point = toMachine(event);
  drag.value = { type, start: point, end: point, regular: event.ctrlKey, fromCenter: event.shiftKey };
}

// ----- Edit Nodes -----

/** How close (screen pixels) the pointer must be to a node, handle or segment to act on it. */
const NODE_TOLERANCE_PX = 6;

function nodeTolerance(): number {
  return NODE_TOLERANCE_PX / scale.value;
}

function onNodesPointerDown(event: PointerEvent, hitId: number | null) {
  const point = toMachine(event);
  const target = hitTest(point, nodeTolerance());
  if (target && startNodeDrag(target, point)) {
    svg.value!.setPointerCapture(event.pointerId);
    return;
  }
  // On a segment of the edited shape: nothing (a double-click there adds a node).
  if (target) return;
  const shape = editor.shapes.find((candidate) => candidate.id === hitId);
  if (shape) {
    startNodeEdit(shape);
  } else {
    endNodeEdit();
    clearSelection();
  }
}

/**
 * The nodes of the edited shape: squares (filled when under the pointer), the curve
 * handles with lines to their nodes, and the segment under the pointer highlighted.
 */
const nodeView = computed(() => {
  const edit = nodeEdit.value;
  if (!edit) return null;
  const target = nodeHover.value;
  const size = 6 / scale.value;
  const nodes = edit.nodes.map((node, index) => ({
    index,
    x: node.p.x - size / 2,
    y: node.p.y - size / 2,
    hovered: target?.kind === "node" && target.index === index,
  }));
  const handles = edit.nodes.flatMap((node, index) =>
    (["in", "out"] as const)
      .filter((which) => node[which])
      .map((which) => ({
        key: `${index}${which}`,
        from: node.p,
        at: node[which]!,
        hovered: target?.kind === "handle" && target.index === index && target.which === which,
      })),
  );
  let segmentPath: string | null = null;
  if (target?.kind === "segment") {
    const a = edit.nodes[target.index];
    const b = edit.nodes[(target.index + 1) % edit.nodes.length];
    segmentPath = "M" + [a.p, ...segment(a, b)].map((p) => `${p.x} ${p.y}`).join("L");
  }
  return { nodes, handles, segmentPath, size, dot: 2.5 / scale.value };
});

const NODES_TITLE =
  "Drag a node or curve handle to change the shape; over a node or segment: " +
  "Del/D delete, L straight, C curved, I insert node (or double-click), M insert at the middle, " +
  "B break, S smooth; drag an end of an open line onto the other end to close it";

const TEXT_NODES_TITLE = "A text has no editable nodes";

// ----- Draw Lines -----

/** A click this close to the line's first point (screen pixels) closes the line. */
const CLOSE_DISTANCE_PX = 8;

/** True when `point` is on the first point of the line being drawn. */
function isOnFirstPoint(point: Point): boolean {
  const first = pen.value?.nodes[0];
  return !!first && Math.hypot(point.x - first.p.x, point.y - first.p.y) * scale.value <= CLOSE_DISTANCE_PX;
}

/** True once a right-click has ended a line, so that click does not open the menu. */
let skipMenu = false;

/**
 * The line being drawn: the segments so far plus the next one to the cursor, the curve
 * handles of the last point, and the first point (highlighted when a click would close).
 */
const penPreview = computed(() => {
  const current = pen.value;
  if (!current) return null;
  const last = current.nodes[current.nodes.length - 1];
  const points = flatten(current.nodes, false);
  if (!current.dragging && current.cursor) points.push(...segment(last, { p: current.cursor }));
  const first = current.nodes[0].p;
  const closing =
    !current.dragging && current.nodes.length >= 2 && !!current.cursor && isOnFirstPoint(current.cursor);
  return {
    d: "M" + points.map((p) => `${p.x} ${p.y}`).join("L"),
    handles: last.in && last.out ? { in: last.in, out: last.out } : null,
    first,
    closing,
    // Marker sizes stay the same on screen whatever the zoom.
    dot: 2.5 / scale.value,
    marker: 4 / scale.value,
  };
});

/**
 * Select tool, as in LightBurn:
 * - Shift+click adds the clicked shape to the selection, Ctrl+click adds or removes it;
 * - a plain click on an unselected shape selects only it, on empty space deselects all;
 * - a plain click on the selection (a selected shape or inside the selection's
 *   box) keeps it, and dragging moves all selected shapes.
 */
function selectAndStartMove(event: PointerEvent, hitId: number | null, onSelection: boolean) {
  const additive = event.shiftKey || event.ctrlKey;
  // Pressing on empty space starts a selection rectangle.
  if (hitId === null && !onSelection) {
    if (!additive) clearSelection();
    svg.value!.setPointerCapture(event.pointerId);
    const point = toMachine(event);
    marquee.value = { start: point, end: point, additive };
    return;
  }
  if (additive) {
    if (hitId !== null) select(hitId, event.shiftKey ? "add" : "toggle");
    return;
  }
  if (!onSelection && hitId !== null) select(hitId, "replace");
  svg.value!.setPointerCapture(event.pointerId);
  move.value = { start: toMachine(event), snapshot: snapshotSelected() };
}

/** Select tool: a double-click on a text continues typing at its end. */
function onDoubleClick(event: MouseEvent) {
  // Draw Lines: a double-click ends the line (open).
  if (editor.tool === "line") {
    finishPen();
    return;
  }
  // Edit Nodes: a double-click on a segment adds a node there.
  if (editor.tool === "nodes") {
    const target = nodeHover.value;
    if (target?.kind === "segment") insertNode(target.index, target.t);
    return;
  }
  if (editor.tool !== "select") return;
  const hit = (event.target as Element).closest("[data-shape-id]") as SVGElement | null;
  const shape = editor.shapes.find((candidate) => candidate.id === Number(hit?.dataset.shapeId));
  if (shape?.type === "image") {
    select(shape.id, "replace");
    adjustImageId.value = shape.id;
    return;
  }
  if (shape?.type !== "text") return;
  select(shape.id, "replace");
  editText(shape);
}

// ----- Text -----

/** The blinking caret of the text being typed: from the descenders up to above the capitals. */
const caret = computed(() => {
  const edit = textEdit.value;
  if (!edit) return null;
  // Upright from the caret's point, turned with the text.
  const below = rotateVector(0, -0.25 * edit.size, edit.angle);
  const above = rotateVector(0, 1.1 * edit.size, edit.angle);
  const { x, y } = edit.caret;
  return { x1: x + below.x, y1: y + below.y, x2: x + above.x, y2: y + above.y };
});

// ----- Selection rectangle -----

/**
 * The selection rectangle being dragged: from `start` to `end` (machine coordinates).
 * `additive` (Shift or Ctrl held): the shapes found are added to the selection.
 */
const marquee = ref<{ start: Point; end: Point; additive: boolean } | null>(null);

/**
 * The rectangle as a box, and its mode, as in LightBurn: dragged to the left it
 * also selects shapes it only touches ("crossing"), to the right only shapes inside.
 */
const marqueeBox = computed(() => {
  const m = marquee.value;
  if (!m) return null;
  const box: Box = {
    left: Math.min(m.start.x, m.end.x),
    right: Math.max(m.start.x, m.end.x),
    bottom: Math.min(m.start.y, m.end.y),
    top: Math.max(m.start.y, m.end.y),
  };
  return { box, crossing: m.end.x < m.start.x };
});

function finishMarquee() {
  const m = marquee.value;
  const found = marqueeBox.value;
  marquee.value = null;
  if (!m || !found) return;
  selectMany(shapesInRect(found.box, found.crossing), m.additive);
}

function onPointerMove(event: PointerEvent) {
  if (pan.value) {
    panView(event);
    return;
  }
  if (pen.value) {
    penMove(toMachine(event), event.ctrlKey);
    return;
  }
  if (isNodeDragging()) {
    nodeDragTo(toMachine(event));
    return;
  }
  if (measurement.value?.dragging) {
    measureTo(toMachine(event), nodeTolerance());
    return;
  }
  if (nodeEdit.value) nodeHover.value = hitTest(toMachine(event), nodeTolerance());
  if (marquee.value) {
    marquee.value.end = toMachine(event);
    return;
  }
  if (resize.value) {
    resizeSelected(event);
    return;
  }
  if (rotation.value) {
    rotateSelection(event);
    return;
  }
  if (move.value) {
    moveSelected(event);
    return;
  }
  if (!drag.value) return;
  drag.value.end = toMachine(event);
  // Modifiers can be pressed or released during the drag, as in LightBurn.
  drag.value.regular = event.ctrlKey;
  drag.value.fromCenter = event.shiftKey;
}

/** New positions = start positions + how far the pointer has moved. */
function moveSelected(event: PointerEvent) {
  if (!move.value) return;
  const point = toMachine(event);
  let dx = point.x - move.value.start.x;
  let dy = point.y - move.value.start.y;
  // Ctrl: move only horizontally or vertically, whichever is larger (as in LightBurn).
  if (event.ctrlKey) {
    if (Math.abs(dx) > Math.abs(dy)) dy = 0;
    else dx = 0;
  }
  moveGroup(move.value.snapshot, dx, dy);
}

/** Escape during a move puts the shapes back where they were. */
function cancelMove() {
  if (move.value) restore(move.value.snapshot);
  move.value = null;
}

function onPointerUp() {
  pan.value = null;
  penUp();
  endMeasure();
  if (isNodeDragging()) endNodeDrag(nodeTolerance());
  finishMarquee();
  resize.value = null;
  rotation.value = null;
  move.value = null;
  const shape = preview.value;
  drag.value = null;
  // A plain click (no drag) gives a zero-size shape: ignore it.
  if (shape && shape.rx > 0 && shape.ry > 0) addShape(shape);
}

function onLostPointerCapture() {
  pan.value = null;
  marquee.value = null;
  drag.value = null;
  move.value = null;
  resize.value = null;
  rotation.value = null;
}

// ----- Right-click menu and dimensions dialog -----

const root = ref<HTMLElement>();
const menuElement = ref<HTMLElement>();
/** Position of the open right-click menu (relative to the work area), or null. */
/** `onShape`: the right-click was on a shape, so the menu offers shape actions. */
const menu = ref<{ x: number; y: number; onShape: boolean } | null>(null);

/**
 * Right-click on a shape offers Delete. An unselected shape becomes the only
 * selected one; on the selection, the whole selection is kept (as in LightBurn).
 */
function onContextMenu(event: MouseEvent) {
  if (skipMenu) {
    skipMenu = false;
    return;
  }
  const target = event.target as Element;
  const hit = target.closest("[data-shape-id]") as SVGElement | null;
  const hitId = hit ? Number(hit.dataset.shapeId) : null;
  const onSelection = !!target.closest("[data-selection]") || (hitId !== null && isSelected(hitId));
  if (hitId !== null && !onSelection) select(hitId, "replace");
  const bounds = root.value!.getBoundingClientRect();
  menu.value = {
    x: event.clientX - bounds.left,
    y: event.clientY - bounds.top,
    onShape: onSelection || hitId !== null,
  };
}

function onDelete() {
  menu.value = null;
  deleteSelected();
}

const dialog = ref<HTMLDialogElement>();
/** The form inside the dialog exists only while it is open, so every
 *  opening starts fresh, pre-filled with the laser's last dimensions. */
const dialogOpen = ref(false);

async function openDialog() {
  menu.value = null;
  dialogOpen.value = true;
  // Wait for Vue to render the form inside the dialog before showing it.
  await nextTick();
  dialog.value!.showModal();
}

/** A click on the dark backdrop around the dialog closes it. */
function onDialogClick(event: MouseEvent) {
  if (event.target === dialog.value) dialog.value!.close();
}

// ----- Window-wide listeners -----

function onKeyDown(event: KeyboardEvent) {
  const typing = (event.target as Element).closest("input, textarea, select");
  // While a text is typed on the canvas, the keys go to it.
  if (textEdit.value && !typing) {
    onTextKey(event);
    return;
  }
  // Escape or Enter ends the line being drawn (open).
  if (pen.value && !typing && (event.key === "Escape" || event.key === "Enter")) {
    finishPen();
    return;
  }
  // Edit Nodes: Escape cancels a drag or ends the editing; other keys act on the node
  // or segment under the pointer (and then not on the whole shape).
  if (nodeEdit.value && !typing && !event.ctrlKey) {
    if (event.key === "Escape") {
      if (isNodeDragging()) cancelNodeDrag();
      else endNodeEdit();
      return;
    }
    if (nodeKey(event.key)) {
      event.preventDefault();
      return;
    }
  }
  if (event.key === "Escape") {
    cancelCalibration();
    clearMeasure();
    drag.value = null;
    marquee.value = null;
    cancelMove();
    cancelResize();
    cancelRotate();
    menu.value = null;
  }
  // Delete, Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+Z and Ctrl+Shift+Z act on shapes, except while typing in a field
  // (there they edit text) or in the middle of a move, resize or rotation.
  if (typing || move.value || resize.value || rotation.value) return;
  if (event.key === "Delete") {
    menu.value = null;
    deleteSelected();
  } else if (event.ctrlKey && event.key.toLowerCase() === "z") {
    event.preventDefault();
    menu.value = null;
    if (event.shiftKey) redo();
    else undo();
  } else if (event.ctrlKey && event.key.toLowerCase() === "c") {
    copySelected();
  } else if (event.ctrlKey && event.key.toLowerCase() === "x") {
    // Cut: copied, then removed; Ctrl+V puts it back.
    menu.value = null;
    copySelected();
    deleteSelected();
  } else if (event.ctrlKey && event.key.toLowerCase() === "v") {
    paste();
  }
}

/**
 * A key while typing a text: Escape ends it, Enter starts a new line, Backspace deletes
 * the last character, and a character is added. Ctrl shortcuts are ignored, but
 * AltGr (reported as Ctrl+Alt) still types its characters.
 */
function onTextKey(event: KeyboardEvent) {
  const shortcut = (event.ctrlKey && !event.altKey) || event.metaKey;
  if (event.key === "Escape") finishText();
  // Ctrl+Z ends the typing and undoes it; Ctrl+Shift+Z ends it and redoes.
  else if (shortcut && event.key.toLowerCase() === "z") (event.shiftKey ? redo : undo)();
  else if (event.key === "Enter" || event.key === "Backspace") typeKey(event.key);
  // Named keys (Shift, ArrowLeft, Dead…) are longer than one character.
  else if (event.key.length === 1 && !shortcut) typeKey(event.key);
  else return;
  // Stop the browser's own reaction (e.g. Space scrolling, Backspace going back).
  event.preventDefault();
}

/** Any click outside the right-click menu closes it. */
function onWindowPointerDown(event: PointerEvent) {
  if (menu.value && !menuElement.value?.contains(event.target as Node)) menu.value = null;
}

// These keys and clicks work anywhere in the window, so listen on `window`
// while this component exists, and stop listening when it is removed.
onMounted(() => {
  // The font list is read once, when the app starts.
  loadFonts();
  window.addEventListener("keydown", onKeyDown);
  window.addEventListener("pointerdown", onWindowPointerDown);
});
onUnmounted(() => {
  window.removeEventListener("keydown", onKeyDown);
  window.removeEventListener("pointerdown", onWindowPointerDown);
});
</script>

<template>
  <main ref="root" class="work-area">
    <!-- v-if / v-else: shows one of the two elements depending on the condition. -->
    <!-- @contextmenu.prevent: our menu instead of the browser's right-click menu. -->
    <svg
      v-if="status.workArea"
      ref="svg"
      class="bed"
      :class="{
        drawing: drawingTool,
        selecting: editor.tool === 'select',
        texting: editor.tool === 'text',
        calibrating: !!calibrating,
      }"
      :viewBox="viewBox"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @lostpointercapture="onLostPointerCapture"
      @dblclick="onDoubleClick"
      @contextmenu.prevent="onContextMenu"
      @wheel.prevent="onWheel"
      @mousedown="onMouseDown"
    >
      <!-- SVG coordinates are in mm, thanks to viewBox. -->
      <rect class="surface" :width="status.workArea.width" :height="status.workArea.height">
        <title>Mouse wheel: zoom at the cursor; middle button drag: pan; right-click: menu</title>
      </rect>
      <!-- Camera overlay: the live video, over the bed's color and under the grid and shapes;
           stretched over the bed, then put in perspective piece by piece by the calibration
           (cameraPieces in camera.ts). The opacity applies to all pieces together. -->
      <foreignObject
        v-if="camera.on"
        class="camera"
        :width="status.workArea.width"
        :height="status.workArea.height"
        :style="{ opacity: camera.opacity }"
      >
        <div class="camera-pieces">
          <video
            v-for="(piece, index) in cameraPieces"
            :key="index"
            :ref="videoRef"
            autoplay
            muted
            playsinline
            :style="{ transform: piece.transform, clipPath: piece.clip }"
          />
        </div>
      </foreignObject>
      <!-- The grid; with the camera on it moves to the axes (see axisTicks). -->
      <line
        v-for="(line, index) in camera.on ? [] : gridLines"
        :key="index"
        v-bind="line"
        :class="line.major ? 'grid major' : 'grid'"
      />

      <!-- Shapes use machine coordinates (Y up): flip the Y axis for SVG (Y down). -->
      <g :transform="`translate(0 ${status.workArea.height}) scale(1 -1)`">
        <!-- Invisible area over the selection's box: grab it anywhere inside to move it.
             Drawn first, so the outlines of other shapes stay clickable on top. -->
        <rect
          v-if="selection && editor.tool === 'select'"
          class="move-area"
          data-selection
          :x="selection.left"
          :y="selection.bottom"
          :width="selection.right - selection.left"
          :height="selection.top - selection.bottom"
        >
          <title>Drag to move the selected shapes (Ctrl: only horizontally or vertically)</title>
        </rect>
        <g
          v-for="shape in editor.shapes"
          :key="shape.id"
          :data-shape-id="shape.id"
          :class="{ selected: isSelected(shape.id) }"
        >
          <!-- Ellipses use <ellipse>; the other shapes a <path> through their outlines
               (text has several: letters and the holes in them).
               The second element is an invisible wide outline that makes the thin shape easy to click. -->
          <!-- A turned ellipse: rotate() turns it around its center (counterclockwise,
               since these coordinates have Y up). -->
          <template v-if="shape.type === 'ellipse'">
            <ellipse
              class="shape"
              :cx="shape.cx"
              :cy="shape.cy"
              :rx="shape.rx"
              :ry="shape.ry"
              :transform="shape.angle ? `rotate(${shape.angle} ${shape.cx} ${shape.cy})` : undefined"
            />
            <ellipse
              class="hit"
              :cx="shape.cx"
              :cy="shape.cy"
              :rx="shape.rx"
              :ry="shape.ry"
              :transform="shape.angle ? `rotate(${shape.angle} ${shape.cx} ${shape.cy})` : undefined"
            >
              <title v-if="editor.tool === 'select'">{{ SHAPE_TITLE }}</title>
              <title v-else-if="editor.tool === 'nodes'">{{ NODES_TITLE }}</title>
            </ellipse>
          </template>
          <!-- An image: how it will be engraved, upright again (these coordinates have Y up)
               and turned around its center; the invisible frame on top makes it easy to click. -->
          <template v-else-if="shape.type === 'image'">
            <image
              :href="previewUrl(shape)"
              :x="-shape.rx"
              :y="-shape.ry"
              :width="2 * shape.rx"
              :height="2 * shape.ry"
              preserveAspectRatio="none"
              :transform="`translate(${shape.cx} ${shape.cy}) rotate(${shape.angle ?? 0}) scale(1 -1)`"
            />
            <path class="image-hit" :d="svgPath(shape)">
              <title v-if="editor.tool === 'select'">{{ IMAGE_TITLE }}</title>
            </path>
          </template>
          <template v-else>
            <path class="shape" :d="svgPath(shape)" />
            <path class="hit" :d="svgPath(shape)">
              <title v-if="editor.tool === 'select'">{{ SHAPE_TITLE }}</title>
              <title v-else-if="editor.tool === 'nodes'">{{ shape.type === 'text' ? TEXT_NODES_TITLE : NODES_TITLE }}</title>
            </path>
          </template>
        </g>
        <template v-if="preview">
          <ellipse
            v-if="preview.type === 'ellipse'"
            class="shape preview"
            :cx="preview.cx"
            :cy="preview.cy"
            :rx="preview.rx"
            :ry="preview.ry"
          />
          <path v-else class="shape preview" :d="svgPath(preview)" />
        </template>

        <!-- Where the next typed character goes. -->
        <line v-if="caret" class="caret" v-bind="caret" />

        <!-- Measure: the measured line, with a circle at each end. -->
        <g v-if="measurement" class="measure">
          <line
            :x1="measurement.start.x"
            :y1="measurement.start.y"
            :x2="measurement.end.x"
            :y2="measurement.end.y"
            :stroke-dasharray="4 / scale"
          />
          <circle :cx="measurement.start.x" :cy="measurement.start.y" :r="3 / scale" />
          <circle :cx="measurement.end.x" :cy="measurement.end.y" :r="3 / scale" />
        </g>

        <!-- Edit Nodes: the edited shape's segment under the pointer, curve handles and nodes. -->
        <g v-if="nodeView" class="node-edit">
          <path v-if="nodeView.segmentPath" class="node-segment" :d="nodeView.segmentPath" />
          <g v-for="handle in nodeView.handles" :key="handle.key">
            <line class="pen-handle" :x1="handle.from.x" :y1="handle.from.y" :x2="handle.at.x" :y2="handle.at.y" />
            <circle
              class="pen-dot"
              :class="{ hovered: handle.hovered }"
              :cx="handle.at.x"
              :cy="handle.at.y"
              :r="handle.hovered ? nodeView.dot * 1.6 : nodeView.dot"
            />
          </g>
          <rect
            v-for="node in nodeView.nodes"
            :key="node.index"
            class="node"
            :class="{ hovered: node.hovered }"
            :x="node.x"
            :y="node.y"
            :width="nodeView.size"
            :height="nodeView.size"
          />
        </g>

        <!-- The line being drawn: its segments up to the cursor, the curve handles of the
             last point, and the first point (filled when a click there would close the line). -->
        <g v-if="penPreview" class="pen">
          <path class="shape preview" :d="penPreview.d" />
          <template v-if="penPreview.handles">
            <line
              class="pen-handle"
              :x1="penPreview.handles.in.x"
              :y1="penPreview.handles.in.y"
              :x2="penPreview.handles.out.x"
              :y2="penPreview.handles.out.y"
            />
            <circle class="pen-dot" :cx="penPreview.handles.in.x" :cy="penPreview.handles.in.y" :r="penPreview.dot" />
            <circle class="pen-dot" :cx="penPreview.handles.out.x" :cy="penPreview.handles.out.y" :r="penPreview.dot" />
          </template>
          <rect
            class="pen-start"
            :class="{ closing: penPreview.closing }"
            :x="penPreview.first.x - penPreview.marker"
            :y="penPreview.first.y - penPreview.marker"
            :width="2 * penPreview.marker"
            :height="2 * penPreview.marker"
          />
        </g>

        <!-- Selection rectangle: solid = shapes inside it, dashed = shapes it touches. -->
        <rect
          v-if="marqueeBox"
          class="marquee"
          :x="marqueeBox.box.left"
          :y="marqueeBox.box.bottom"
          :width="marqueeBox.box.right - marqueeBox.box.left"
          :height="marqueeBox.box.top - marqueeBox.box.bottom"
          :stroke-width="1 / scale"
          :stroke-dasharray="marqueeBox.crossing ? 4 / scale : undefined"
        />

        <!-- Selection frame: dashed bounding box, 8 resize handles and 4 rotate arrows
             outside the corners, on top of the shapes. -->
        <g v-if="selectionFrame">
          <rect
            class="selection-box"
            :x="selectionFrame.box.left"
            :y="selectionFrame.box.bottom"
            :width="selectionFrame.box.right - selectionFrame.box.left"
            :height="selectionFrame.box.top - selectionFrame.box.bottom"
            :stroke-width="selectionFrame.line"
            :stroke-dasharray="selectionFrame.dash"
          />
          <!-- Each arrow is drawn twice: visible, and as a wide invisible line that is easy to grab. -->
          <g v-for="arrow in selectionFrame.rotateArrows" :key="arrow.key" data-rotate class="rotate-arrow">
            <path class="arrow" :d="arrow.d" />
            <path class="grab" :d="arrow.d">
              <title>{{ ROTATE_ARROW_TITLE }}</title>
            </path>
          </g>
          <rect
            v-for="handle in selectionFrame.handles"
            :key="handle.index"
            class="handle"
            :data-handle="handle.index"
            :x="handle.x"
            :y="handle.y"
            :width="handle.size"
            :height="handle.size"
            :style="{ cursor: handle.cursor }"
          >
            <!-- In SVG the tooltip is a <title> child element, not an attribute. -->
            <title>{{ handle.title }}</title>
          </rect>
        </g>
      </g>

      <!-- Camera calibration: a cross on each corner clicked so far. -->
      <g v-if="calibrating" class="calibration-mark">
        <path v-for="(mark, index) in calibrationMarks" :key="index" :d="mark" />
      </g>

      <!-- Rulers: drawn last so they stay on top when zooming brings them over the bed. -->
      <g class="ruler">
        <rect v-bind="rulers.x" />
        <rect v-bind="rulers.y" />
      </g>
      <g v-if="axisLines" class="axis-line">
        <line v-bind="axisLines.x" />
        <line v-bind="axisLines.y" />
        <line
          v-for="(tick, index) in axisTicks"
          :key="index"
          :x1="tick.x1"
          :y1="tick.y1"
          :x2="tick.x2"
          :y2="tick.y2"
          :class="{ major: tick.major }"
        />
      </g>

      <!-- Extent of the selection on the axes. -->
      <g v-if="axisShadows" class="axis-shadow">
        <line v-bind="axisShadows.x" />
        <line v-bind="axisShadows.y" />
      </g>

      <!-- Axis labels in mm. The origin's "0" sits left of the corner, right-aligned like
           the Y labels, so it lines up with both axes. -->
      <g class="axis-labels" :font-size="fontSize">
        <text
          v-for="label in axisLabels.x"
          :key="`x${label.value}`"
          :x="label.value === 0 ? axes.x - fontSize * 0.4 : label.x"
          :y="axes.y + fontSize * 0.3"
          :text-anchor="label.value === 0 ? 'end' : 'middle'"
          dominant-baseline="hanging"
        >
          {{ label.value }}
        </text>
        <text
          v-for="label in axisLabels.y"
          :key="`y${label.value}`"
          :x="axes.x - fontSize * 0.4"
          :y="label.y"
          text-anchor="end"
          dominant-baseline="middle"
        >
          {{ label.value }}
        </text>
      </g>
    </svg>

    <div v-else class="empty">
      <span>Connect laser or set dimension in mm</span>
      <WorkAreaForm
        submit-label="Set dimensions"
        submit-title="Setting the dimensions is required while the laser is not connected"
      />
    </div>

    <CalibrationPopup v-if="calibrating && calibrationHelp" @close="calibrationHelp = false" />

    <div
      v-if="menu"
      ref="menuElement"
      class="context-menu"
      :style="{ left: `${menu.x}px`, top: `${menu.y}px` }"
    >
      <template v-if="menu.onShape">
        <button title="Delete the selected shapes (Del key)" @click="onDelete">Delete</button>
      </template>
      <template v-else>
        <button title="Open a dialog to set the canvas width and height in mm" @click="openDialog">
          Set dimension canvas
        </button>
        <button title="Reset zoom and pan so the whole work area is visible" @click="onZoomToFit">
          Zoom to fit
        </button>
      </template>
    </div>

    <dialog ref="dialog" class="dimensions-dialog" @close="dialogOpen = false" @click="onDialogClick">
      <!-- The padding lives on this inner element, so a click inside the dialog
           never has the dialog itself as target (that would close it). -->
      <div class="dialog-content">
        <WorkAreaForm
          v-if="dialogOpen"
          :initial="status.laserWorkArea"
          submit-label="Set"
          submit-title="Save these dimensions; they are used even when a laser is connected"
          @saved="dialog?.close()"
        />
      </div>
    </dialog>
  </main>
</template>

<style scoped>
.work-area {
  position: relative;
  display: grid;
  place-items: center;
  overflow: hidden;
  background: var(--canvas);
}

.bed {
  width: 100%;
  height: 100%;
}

.surface {
  fill: var(--bed);
  stroke: var(--border-strong);
  /* Line width stays 1 screen pixel regardless of scale. */
  vector-effect: non-scaling-stroke;
}

/* Only an image: clicks go through to the bed, as if it were not there. */
.camera {
  pointer-events: none;
}

/* The copies of the video lie on top of each other, each the size of the bed. */
.camera-pieces {
  position: relative;
  width: 100%;
  height: 100%;
}

.camera video {
  position: absolute;
  inset: 0;
  display: block;
  width: 100%;
  height: 100%;
  /* Stretched to the bed's shape; the calibration's transform starts from there. */
  object-fit: fill;
  transform-origin: 0 0;
}

/* Calibrating the camera: a crosshair everywhere on the canvas, also over the
   selected square (otherwise the move cursor). */
.bed.calibrating,
.bed.calibrating * {
  cursor: crosshair !important;
}

.calibration-mark {
  fill: none;
  stroke: var(--danger);
  stroke-width: 2px;
  vector-effect: non-scaling-stroke;
  pointer-events: none;
}

.grid {
  stroke: var(--grid);
  vector-effect: non-scaling-stroke;
}

.grid.major {
  stroke: var(--grid-major);
}

/* Ruler strips behind the labels: same color as the area around the bed. */
.ruler {
  fill: var(--canvas);
  pointer-events: none;
}

/* Same color and width as the bed border, so at "fit" it simply covers it. */
.axis-line {
  stroke: var(--border-strong);
  vector-effect: non-scaling-stroke;
  pointer-events: none;
}

/* The longer marks on the axes (every 50 mm, camera on). */
.axis-line .major {
  stroke: #555555;
}

.axis-shadow {
  /* Same width as the bed border; only the color changes. */
  stroke: #555555;
  vector-effect: non-scaling-stroke;
  pointer-events: none;
}

.axis-labels {
  fill: var(--muted);
  /* Labels never catch clicks meant for the bed. */
  pointer-events: none;
}

.bed.drawing {
  cursor: crosshair;
}

.bed.texting {
  cursor: text;
}

.shape {
  fill: none;
  stroke: var(--shape);
  stroke-width: 1.5;
  vector-effect: non-scaling-stroke;
}

/* The shape being drawn, before the mouse button is released. */
.shape.preview {
  stroke: var(--accent);
  opacity: 0.6;
}

.selected .shape {
  stroke: var(--accent);
}

/* An image reacts to the pointer over its whole surface, like in LightBurn. */
.image-hit {
  fill: transparent;
  stroke: none;
}

.hit {
  fill: none;
  stroke: transparent;
  stroke-width: 10;
  vector-effect: non-scaling-stroke;
  /* Only the outline reacts to the pointer, not the inside. */
  pointer-events: stroke;
}

/* With the Select tool, shapes can be grabbed and moved. */
.bed.selecting .hit,
.move-area {
  cursor: move;
}

.move-area {
  fill: transparent;
}

.selection-box {
  fill: none;
  stroke: var(--accent);
  /* Only a visual guide: clicks go through to the shapes and the move area. */
  pointer-events: none;
}

.marquee {
  fill: var(--accent);
  fill-opacity: 0.08;
  stroke: var(--accent);
  pointer-events: none;
}

.handle {
  fill: var(--bed);
  stroke: var(--accent);
  vector-effect: non-scaling-stroke;
}

.rotate-arrow .arrow {
  fill: none;
  stroke: var(--accent);
  stroke-width: 1.5;
  vector-effect: non-scaling-stroke;
}

.rotate-arrow .grab {
  fill: none;
  stroke: transparent;
  stroke-width: 10;
  vector-effect: non-scaling-stroke;
  pointer-events: stroke;
}

/* A circular-arrow cursor (there is no standard "rotate" cursor). */
.rotate-arrow {
  cursor:
    url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='20' height='20' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M21 12a9 9 0 1 1-2.64-6.36L21 8'/%3E%3Cpath d='M21 3v5h-5'/%3E%3C/svg%3E")
      10 10,
    crosshair;
}

/* The line being drawn only shows; clicks go to the canvas. */
.pen {
  pointer-events: none;
}

.pen-handle {
  stroke: var(--accent);
  stroke-width: 1;
  vector-effect: non-scaling-stroke;
}

.pen-dot {
  fill: var(--accent);
}

.pen-start {
  fill: var(--bed);
  stroke: var(--accent);
  vector-effect: non-scaling-stroke;
}

/* Measure: only shows; the values are in the top bar. */
.measure {
  pointer-events: none;
  stroke: var(--accent);
  vector-effect: non-scaling-stroke;
  fill: var(--bed);
}

.measure * {
  vector-effect: non-scaling-stroke;
}

/* Edit Nodes: only shows; the pointer is tested in nodes.ts. */
.node-edit {
  pointer-events: none;
}

.node {
  fill: var(--bed);
  stroke: var(--accent);
  vector-effect: non-scaling-stroke;
}

.node.hovered {
  fill: var(--accent);
}

.node-segment {
  fill: none;
  stroke: var(--accent);
  stroke-width: 3;
  vector-effect: non-scaling-stroke;
}

/* A click now would close the line. */
.pen-start.closing {
  fill: var(--accent);
}

/* Blinks like a text cursor. */
.caret {
  stroke: var(--accent);
  stroke-width: 1.5;
  vector-effect: non-scaling-stroke;
  pointer-events: none;
  animation: blink 1s steps(1) infinite;
}

@keyframes blink {
  50% {
    opacity: 0;
  }
}

/* The text and the form on one row. */
.empty {
  display: flex;
  align-items: center;
  gap: 12px;
}


.context-menu {
  position: absolute;
  padding: 4px 0;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--panel);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
}

.context-menu button {
  display: block;
  width: 100%;
  padding: 4px 16px;
  border: none;
  background: none;
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: default;
}

.context-menu button:hover {
  background: var(--accent-soft);
}

.dimensions-dialog {
  padding: 0;
  border: 1px solid var(--border-strong);
  border-radius: 6px;
  background: var(--panel);
  color: var(--text);
}

.dialog-content {
  padding: 16px;
}

/* The dimmed area behind the open dialog. */
.dimensions-dialog::backdrop {
  background: rgba(0, 0, 0, 0.3);
}
</style>
