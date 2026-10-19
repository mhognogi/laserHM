// Editor state shared by components: the active tool and the drawn shapes.
import { reactive } from "vue";
import type { PathNode } from "./paths";
import type { ImageSettings } from "./images";

export type ToolId =
  | "select"
  | "nodes"
  | "line"
  | "rectangle"
  | "ellipse"
  | "polygon"
  | "text"
  | "position"
  | "measure";

/** A point in machine coordinates (mm): origin at the bottom-left, Y grows upwards. */
export interface Point {
  x: number;
  y: number;
}

/**
 * Where a shape is and how big: the center and half the width and height of the
 * box it fills, in machine coordinates (mm), and how much that box is turned.
 */
export interface Geometry {
  cx: number;
  cy: number;
  rx: number;
  ry: number;
  /** Rotation around the center, in degrees, counterclockwise (Y is up); missing = 0. */
  angle?: number;
}

/** The kinds of shapes the editor draws; the same names are used by Rust. */
export type ShapeType = "ellipse" | "rectangle" | "polygon" | "text" | "path" | "image";

/** An installed font face, as Rust lists it (see src-tauri/src/text.rs). */
export interface FontInfo {
  name: string;
  path: string;
  index: number;
}

export interface Shape extends Geometry {
  id: number;
  type: ShapeType;
  /** Text only: the text and its font. */
  text?: string;
  font?: FontInfo;
  /** Text and path only: the outlines in the box from -1 to 1 (a text's letters come
   *  from Rust; a path has one, with its curves split into short segments). */
  contours?: Point[][];
  /** Path only: true when it ends where it started. */
  closed?: boolean;
  /** Path only: its nodes and curve handles in the box from -1 to 1 (see paths.ts). */
  nodes?: PathNode[];
  /** Text only: a capital letter's height, as a fraction of the box's height. */
  capHeight?: number;
  /** Text only: where the first line starts (on its baseline), in the box from -1 to 1. */
  origin?: Point;
  /** Text only: overlapping letters are joined into one outline (LightBurn's "Welded"). */
  welded?: boolean;
  /** Its own cut settings (e.g. a Material Test cell), used by Start instead of the panel's. */
  cut?: CutSettings;
  /** Image only: the imported picture (see images.ts) and how it is engraved. */
  imageId?: number;
  imageSettings?: ImageSettings;
  /** Can be moved and rotated, not resized (a Material Test: its cells would no longer match). */
  fixedSize?: boolean;
}

/** Same fields as the `CutSettings` struct in src-tauri/src/job.rs. */
export interface CutSettings {
  powerPercent: number;
  feed: number;
  passes: number;
  /** True: engraved full, line by line; false: only the outline. */
  fill: boolean;
  /** Fill only: distance between the engraved lines, in mm. */
  interval: number;
}

/** A shape's kind and size, enough to draw its outline. */
type Outlined = Geometry & Pick<Shape, "type" | "contours" | "closed">;

/** False only for an open path (drawn with Draw Lines and not closed). */
export function isClosed(shape: Outlined): boolean {
  return shape.type !== "path" || !!shape.closed;
}

/** The tools that draw a shape by dragging, and the shape each one draws. */
export const DRAWING_TOOLS: Partial<Record<ToolId, ShapeType>> = {
  ellipse: "ellipse",
  rectangle: "rectangle",
  polygon: "polygon",
};

export const editor = reactive({
  tool: "select" as ToolId,
  shapes: [] as Shape[],
  /** Ids of the selected shapes; empty when nothing is selected. */
  selectedIds: [] as number[],
});

let nextId = 1;

/** Adds a shape and selects only it, as LightBurn does with a newly drawn shape. */
export function addShape(shape: Omit<Shape, "id">): number {
  const id = nextId++;
  editor.shapes.push({ ...shape, id });
  editor.selectedIds = [id];
  return id;
}

/** Puts `shapes` in place of all the shapes (New, Open), nothing selected. */
export function replaceShapes(shapes: Shape[]) {
  editor.shapes = shapes;
  editor.selectedIds = [];
  // New shapes get ids after the loaded ones.
  nextId = Math.max(0, ...shapes.map((shape) => shape.id)) + 1;
}

/** Adds several shapes and selects them together (e.g. a Material Test, to move it as one). */
export function addShapes(shapes: Omit<Shape, "id">[]) {
  editor.selectedIds = shapes.map((shape) => {
    const id = nextId++;
    editor.shapes.push({ ...shape, id });
    return id;
  });
}

// ----- Outlines -----

/** Number of sides of the polygon tool (fixed for now). */
const POLYGON_SIDES = 6;
/** Points used to draw or test an ellipse's outline. */
const ELLIPSE_SAMPLES = 72;

/**
 * The polygon's corners for a box from -1 to 1 on both axes: a regular polygon
 * with one corner at the top, stretched to fill the box exactly (so the shape's
 * width and height are those of its box, like the other shapes).
 */
export const UNIT_POLYGON: Point[] = (() => {
  const corners = Array.from({ length: POLYGON_SIDES }, (_, i) => {
    const angle = Math.PI / 2 + (2 * Math.PI * i) / POLYGON_SIDES;
    return { x: Math.cos(angle), y: Math.sin(angle) };
  });
  const xs = corners.map((p) => p.x);
  const ys = corners.map((p) => p.y);
  const [minX, maxX, minY, maxY] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  return corners.map((p) => ({
    x: ((p.x - minX) / (maxX - minX)) * 2 - 1,
    y: ((p.y - minY) / (maxY - minY)) * 2 - 1,
  }));
})();

/**
 * Width / height of the "regular" version of each shape (Ctrl while drawing):
 * a circle, a square, and the polygon with equal sides (a regular hexagon is
 * narrower than it is tall).
 */
const REGULAR_ASPECT: Record<ShapeType, number> = {
  ellipse: 1,
  rectangle: 1,
  polygon: Math.cos(Math.PI / POLYGON_SIDES),
  // Text is placed with a click, paths point by point and images imported, not drawn by dragging.
  text: 1,
  path: 1,
  image: 1,
};

/** Turns the vector (x, y) by `degrees`, counterclockwise. */
export function rotateVector(x: number, y: number, degrees: number): Point {
  const radians = (degrees * Math.PI) / 180;
  const [sin, cos] = [Math.sin(radians), Math.cos(radians)];
  return { x: x * cos - y * sin, y: x * sin + y * cos };
}

/**
 * A point of the unit box (-1 to 1 on both axes) placed on a shape: scaled to its
 * box, turned by its angle, then moved to its center (the same as Rust's `place`).
 */
export function placeOn(shape: Geometry, ux: number, uy: number): Point {
  const turned = rotateVector(shape.rx * ux, shape.ry * uy, shape.angle ?? 0);
  return { x: shape.cx + turned.x, y: shape.cy + turned.y };
}

/**
 * A shape's outlines as closed lists of corners (machine coordinates).
 * Text has one per contour (letters and the holes in them); the other shapes one.
 */
export function outlines(shape: Outlined): Point[][] {
  const at = (ux: number, uy: number) => placeOn(shape, ux, uy);
  switch (shape.type) {
    case "rectangle":
    // An image: its frame.
    case "image":
      return [[at(1, -1), at(1, 1), at(-1, 1), at(-1, -1)]];
    case "polygon":
      return [UNIT_POLYGON.map((p) => at(p.x, p.y))];
    case "text":
    case "path":
      return (shape.contours ?? []).map((contour) => contour.map((p) => at(p.x, p.y)));
    case "ellipse":
      return [
        Array.from({ length: ELLIPSE_SAMPLES }, (_, i) => {
          const angle = (2 * Math.PI * i) / ELLIPSE_SAMPLES;
          return at(Math.cos(angle), Math.sin(angle));
        }),
      ];
  }
}

/** The outlines as an SVG path, e.g. "M10 20L30 40L50 20Z": one piece per outline,
 *  closed ("Z") except for an open path. */
export function svgPath(shape: Outlined): string {
  const end = isClosed(shape) ? "Z" : "";
  return outlines(shape)
    .map((outline) => "M" + outline.map((p) => `${p.x} ${p.y}`).join("L") + end)
    .join("");
}

// ----- Selection -----

export function isSelected(id: number): boolean {
  return editor.selectedIds.includes(id);
}

/**
 * Changes the selection after a click on shape `id`, as in LightBurn:
 * "replace" selects only it, "add" (Shift) adds it, "toggle" (Ctrl) adds or removes it.
 */
export function select(id: number, mode: "replace" | "add" | "toggle") {
  if (mode === "replace") editor.selectedIds = [id];
  else if (!isSelected(id)) editor.selectedIds.push(id);
  else if (mode === "toggle") editor.selectedIds = editor.selectedIds.filter((other) => other !== id);
}

export function clearSelection() {
  editor.selectedIds = [];
}

/** Points tested along each edge of an outline when checking whether a rectangle touches it. */
const EDGE_SAMPLES = 16;

/**
 * Ids of the shapes found by a selection rectangle, as in LightBurn:
 * `crossing` false: only shapes entirely inside it;
 * `crossing` true: also shapes whose outline it touches.
 */
export function shapesInRect(rect: Box, crossing: boolean): number[] {
  const inside = (p: Point) => p.x >= rect.left && p.x <= rect.right && p.y >= rect.bottom && p.y <= rect.top;
  return editor.shapes
    .filter((shape) => {
      const box = shapeBox(shape);
      if (inside({ x: box.left, y: box.bottom }) && inside({ x: box.right, y: box.top })) return true;
      if (!crossing) return false;
      // Shapes are outlines (not filled): it counts only if an outline enters the rectangle.
      const closed = isClosed(shape);
      return outlines(shape).some((corners) =>
        corners.some((a, i) => {
          // An open path has no edge from its last point back to the first.
          if (!closed && i === corners.length - 1) return inside(a);
          const b = corners[(i + 1) % corners.length];
          return Array.from({ length: EDGE_SAMPLES + 1 }, (_, k) => k / EDGE_SAMPLES).some((t) =>
            inside({ x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t }),
          );
        }),
      );
    })
    .map((shape) => shape.id);
}

/** Selects `ids`, or adds them to the current selection when `add` is true. */
export function selectMany(ids: number[], add: boolean) {
  editor.selectedIds = add ? [...new Set([...editor.selectedIds, ...ids])] : ids;
}

/** The selected shapes. Reactive when used in a template or `computed`. */
export function selectedShapes(): Shape[] {
  return editor.shapes.filter((shape) => isSelected(shape.id));
}

/** Removes the selected shapes; afterwards nothing is selected. */
export function deleteSelected() {
  editor.shapes = editor.shapes.filter((shape) => !isSelected(shape.id));
  clearSelection();
}

// ----- Copy and paste -----

/** How far a pasted copy is moved from what was copied: right and down, in mm. */
const PASTE_OFFSET = { x: 10, y: -10 };

/** The copied shapes (without ids), kept inside the app. */
let clipboard: Omit<Shape, "id">[] = [];

/** Ctrl+C: remembers the selected shapes. */
export function copySelected() {
  if (editor.selectedIds.length === 0) return;
  // Everything except the id (a text keeps its text, font and outlines).
  clipboard = selectedShapes().map(({ id: _id, ...shape }) => shape);
}

/**
 * Ctrl+V: adds the copied shapes, moved right and down so they are visible,
 * and selects them. Pasting again moves the next copy a step further.
 */
export function paste() {
  if (clipboard.length === 0) return;
  clipboard = clipboard.map((shape) => ({
    ...shape,
    cx: shape.cx + PASTE_OFFSET.x,
    cy: shape.cy + PASTE_OFFSET.y,
  }));
  const ids = clipboard.map((shape) => {
    const id = nextId++;
    editor.shapes.push({ ...shape, id });
    return id;
  });
  editor.selectedIds = ids;
}

// ----- Group geometry -----

/** Geometry of the given shapes, to restore or transform them. */
export type Snapshot = { id: number; geometry: Geometry }[];

export function snapshotSelected(): Snapshot {
  return selectedShapes().map(({ id, cx, cy, rx, ry, angle }) => ({ id, geometry: { cx, cy, rx, ry, angle } }));
}

/** Puts the shapes back as they were in `snapshot` (e.g. Escape during a move). */
export function restore(snapshot: Snapshot) {
  for (const { id, geometry } of snapshot) {
    const shape = editor.shapes.find((other) => other.id === id);
    if (shape) Object.assign(shape, geometry);
  }
}

/** True when a selected shape must keep its size (no resize handles, Width/Height disabled). */
export function sizeLocked(): boolean {
  return selectedShapes().some((shape) => shape.fixedSize);
}

/** The box around all selected shapes, or null when nothing is selected. */
export function selectionBox(): Box | null {
  const boxes = selectedShapes().map(shapeBox);
  if (boxes.length === 0) return null;
  return {
    left: Math.min(...boxes.map((box) => box.left)),
    right: Math.max(...boxes.map((box) => box.right)),
    bottom: Math.min(...boxes.map((box) => box.bottom)),
    top: Math.max(...boxes.map((box) => box.top)),
  };
}

/**
 * Fits the shapes of `snapshot`, whose group box was `from`, into the box `to`:
 * every shape keeps its place relative to the group, scaled with it.
 *
 * A turned shape stretched more in one direction than the other would become
 * slanted, which a box with an angle cannot describe. Its sides are then scaled
 * by how much each one stretches, and it is turned to where its width side goes:
 * exact for upright or quarter-turned shapes and for equal scaling, close otherwise.
 */
export function transformGroup(snapshot: Snapshot, from: Box, to: Box) {
  const scaleX = (to.right - to.left) / Math.max(from.right - from.left, 1e-9);
  const scaleY = (to.top - to.bottom) / Math.max(from.top - from.bottom, 1e-9);
  for (const { id, geometry } of snapshot) {
    const shape = editor.shapes.find((other) => other.id === id);
    if (!shape) continue;
    shape.cx = to.left + (geometry.cx - from.left) * scaleX;
    shape.cy = to.bottom + (geometry.cy - from.bottom) * scaleY;
    const angle = geometry.angle ?? 0;
    if (angle === 0) {
      shape.rx = geometry.rx * scaleX;
      shape.ry = geometry.ry * scaleY;
      continue;
    }
    // The shape's width and height sides, before and after the scaling.
    const side = rotateVector(geometry.rx, 0, angle);
    const up = rotateVector(0, geometry.ry, angle);
    const width = { x: side.x * scaleX, y: side.y * scaleY };
    const height = { x: up.x * scaleX, y: up.y * scaleY };
    shape.rx = Math.hypot(width.x, width.y);
    shape.ry = Math.hypot(height.x, height.y);
    shape.angle = (Math.atan2(width.y, width.x) * 180) / Math.PI;
  }
}

/**
 * Turns the shapes of `snapshot` by `degrees` (counterclockwise) around `pivot`:
 * each center goes around the pivot and each shape turns by the same amount.
 */
export function rotateGroup(snapshot: Snapshot, pivot: Point, degrees: number) {
  for (const { id, geometry } of snapshot) {
    const shape = editor.shapes.find((other) => other.id === id);
    if (!shape) continue;
    const center = rotateVector(geometry.cx - pivot.x, geometry.cy - pivot.y, degrees);
    shape.cx = pivot.x + center.x;
    shape.cy = pivot.y + center.y;
    shape.angle = normalizeAngle((geometry.angle ?? 0) + degrees);
  }
}

/** The same angle between -180 and 180 degrees. */
export function normalizeAngle(degrees: number): number {
  const turned = (((degrees + 180) % 360) + 360) % 360 - 180;
  // Rounding errors (e.g. 89.99999999) would show up in the angle field.
  return Math.round(turned * 1e6) / 1e6;
}

/** Turns the selection by `degrees` around the center of its box (the Rotate field). */
export function rotateSelected(degrees: number) {
  const box = selectionBox();
  if (!box) return;
  rotateGroup(snapshotSelected(), { x: (box.left + box.right) / 2, y: (box.bottom + box.top) / 2 }, degrees);
}

/** Moves the shapes of `snapshot` by (dx, dy) mm from where they were. */
export function moveGroup(snapshot: Snapshot, dx: number, dy: number) {
  for (const { id, geometry } of snapshot) {
    const shape = editor.shapes.find((other) => other.id === id);
    if (!shape) continue;
    shape.cx = geometry.cx + dx;
    shape.cy = geometry.cy + dy;
  }
}

// ----- Resizing -----

/** Smallest width or height a shape can be resized to, in mm. */
const MIN_SIZE = 0.1;

/** A shape's bounding box in machine coordinates (mm, Y up). */
export interface Box {
  left: number;
  right: number;
  bottom: number;
  top: number;
}

/** The upright box of the size `shape` gives, ignoring its angle. */
export function boxOf(shape: Geometry): Box {
  return {
    left: shape.cx - shape.rx,
    right: shape.cx + shape.rx,
    bottom: shape.cy - shape.ry,
    top: shape.cy + shape.ry,
  };
}

/** The upright box around a shape as it is drawn, turned shapes included. */
export function shapeBox(shape: Outlined): Box {
  const angle = shape.angle ?? 0;
  if (angle === 0) return boxOf(shape);
  if (shape.type === "ellipse") {
    // The exact half-width and half-height of a turned ellipse.
    const radians = (angle * Math.PI) / 180;
    const [sin, cos] = [Math.sin(radians), Math.cos(radians)];
    const halfWidth = Math.hypot(shape.rx * cos, shape.ry * sin);
    const halfHeight = Math.hypot(shape.rx * sin, shape.ry * cos);
    return boxOf({ cx: shape.cx, cy: shape.cy, rx: halfWidth, ry: halfHeight });
  }
  const points = outlines(shape).flat();
  const xs = points.map((p) => p.x);
  const ys = points.map((p) => p.y);
  return { left: Math.min(...xs), right: Math.max(...xs), bottom: Math.min(...ys), top: Math.max(...ys) };
}

/**
 * A resize handle on the bounding box: `hx` is -1 (left), 0 (middle) or 1 (right),
 * `hy` is -1 (bottom), 0 (middle) or 1 (top). Corners have both non-zero.
 */
export interface Handle {
  hx: -1 | 0 | 1;
  hy: -1 | 0 | 1;
}

/** The 8 handles: 4 corners and 4 edge midpoints. */
export const HANDLES: Handle[] = [
  { hx: -1, hy: 1 },
  { hx: 0, hy: 1 },
  { hx: 1, hy: 1 },
  { hx: 1, hy: 0 },
  { hx: 1, hy: -1 },
  { hx: 0, hy: -1 },
  { hx: -1, hy: -1 },
  { hx: -1, hy: 0 },
];

/**
 * The box (as center and half sizes) after dragging `handle` of `box` to `pointer`.
 * Normally the opposite side stays in place; `fromCenter` (Ctrl) keeps the center
 * in place instead; `keepAspect` (Shift, corners only) keeps the width/height ratio.
 */
export function resizeFromHandle(
  box: Box,
  handle: Handle,
  pointer: Point,
  keepAspect: boolean,
  fromCenter: boolean,
): Geometry {
  const width0 = box.right - box.left;
  const height0 = box.top - box.bottom;
  const cx0 = (box.left + box.right) / 2;
  const cy0 = (box.bottom + box.top) / 2;
  // The side that stays in place: opposite to the dragged handle.
  const anchorX = handle.hx === 1 ? box.left : box.right;
  const anchorY = handle.hy === 1 ? box.bottom : box.top;

  let width = width0;
  let height = height0;
  if (handle.hx !== 0) {
    width = fromCenter ? 2 * Math.abs(pointer.x - cx0) : Math.abs(pointer.x - anchorX);
  }
  if (handle.hy !== 0) {
    height = fromCenter ? 2 * Math.abs(pointer.y - cy0) : Math.abs(pointer.y - anchorY);
  }
  if (keepAspect && handle.hx !== 0 && handle.hy !== 0) {
    const factor = Math.max(width / width0, height / height0);
    width = width0 * factor;
    height = height0 * factor;
  }
  width = Math.max(width, MIN_SIZE);
  height = Math.max(height, MIN_SIZE);

  let cx = cx0;
  let cy = cy0;
  if (!fromCenter) {
    // The shape grows from the anchor towards the pointer (it may flip past the anchor).
    if (handle.hx !== 0) cx = anchorX + (Math.sign(pointer.x - anchorX) || handle.hx) * (width / 2);
    if (handle.hy !== 0) cy = anchorY + (Math.sign(pointer.y - anchorY) || handle.hy) * (height / 2);
  }
  return { cx, cy, rx: width / 2, ry: height / 2 };
}

/**
 * The shape drawn by dragging from `start` to `end`, as in LightBurn:
 * normally it fits the dragged rectangle; `regular` (Ctrl) gives the regular
 * version (circle, square, equal-sided polygon); `fromCenter` (Shift) makes
 * `start` the center instead of a corner.
 */
export function shapeFromDrag(
  type: ShapeType,
  start: Point,
  end: Point,
  regular: boolean,
  fromCenter: boolean,
): Geometry {
  let dx = end.x - start.x;
  let dy = end.y - start.y;
  if (regular) {
    const aspect = REGULAR_ASPECT[type];
    // The larger of the two drag directions decides the size.
    const height = Math.max(Math.abs(dx) / aspect, Math.abs(dy));
    // Keep the drag direction; `|| 1` avoids Math.sign(0) = 0.
    dx = Math.sign(dx || 1) * height * aspect;
    dy = Math.sign(dy || 1) * height;
  }
  if (fromCenter) {
    return { cx: start.x, cy: start.y, rx: Math.abs(dx), ry: Math.abs(dy) };
  }
  return { cx: start.x + dx / 2, cy: start.y + dy / 2, rx: Math.abs(dx) / 2, ry: Math.abs(dy) / 2 };
}
