// Edit Nodes, as in LightBurn: click a shape to show its nodes, then drag nodes and
// curve handles; with the pointer over a node or segment, keys change the path:
//   Delete / D  delete the node, or the segment (the path opens or splits there)
//   L / C       make the segment straight / curved
//   I / M       insert a node at the pointer / at the segment's middle (also: double-click)
//   B           break the path at the node
//   S           make the node smooth
// Dragging an end of an open path onto its other end closes it.
// Rectangles, ellipses and polygons become paths on their first change.
import { ref, watch } from "vue";
import { addShape, editor, select, type Point, type Shape } from "./editor";
import {
  CURVE_SEGMENTS,
  isCurve,
  machineNodes,
  mapNode,
  pathShape,
  pointOnSegment,
  type PathNode,
} from "./paths";

/** The shape whose nodes are edited, and its nodes in machine coordinates (mm). */
export const nodeEdit = ref<{ shapeId: number; nodes: PathNode[]; closed: boolean } | null>(null);

/** What is under the pointer: a node, a curve handle, or a point on a segment (`t`: 0 to 1 along it). */
export type NodeTarget =
  | { kind: "node"; index: number }
  | { kind: "handle"; index: number; which: "in" | "out" }
  | { kind: "segment"; index: number; t: number };

export const hover = ref<NodeTarget | null>(null);

/** The node or handle being dragged, where the drag started, and the nodes before it. */
let drag: { target: NodeTarget; start: Point; original: PathNode[] } | null = null;

const copyNodes = (nodes: PathNode[]) => nodes.map((node) => mapNode(node, (p) => ({ ...p })));
const distance = (a: Point, b: Point) => Math.hypot(a.x - b.x, a.y - b.y);
const lerp = (a: Point, b: Point, t: number) => ({ x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t });

/** Starts editing a shape's nodes and selects it. False for a text (it has no editable nodes). */
export function startNodeEdit(shape: Shape): boolean {
  // A fixed-size shape (Material Test) keeps its outline: its fill would change.
  const found = shape.fixedSize ? null : machineNodes(shape);
  select(shape.id, "replace");
  if (!found) return false;
  nodeEdit.value = { shapeId: shape.id, ...found };
  hover.value = null;
  return true;
}

export function endNodeEdit() {
  nodeEdit.value = null;
  hover.value = null;
  drag = null;
}

// Choosing another tool ends the editing.
watch(() => editor.tool, endNodeEdit);

// So does removing the edited shape (e.g. right-click → Delete).
watch(
  () => !!nodeEdit.value && !editor.shapes.some((shape) => shape.id === nodeEdit.value!.shapeId),
  (removed) => removed && endNodeEdit(),
);

/** Number of segments: a closed path also has the one from the last node back to the first. */
function segmentCount(nodes: PathNode[], closed: boolean): number {
  return closed ? nodes.length : nodes.length - 1;
}

/** The two nodes of segment `index`. */
function segmentNodes(index: number): [PathNode, PathNode] {
  const { nodes } = nodeEdit.value!;
  return [nodes[index], nodes[(index + 1) % nodes.length]];
}

/**
 * Writes the edited nodes back into the shape, which becomes (or stays) a path.
 * Fewer than 2 nodes left: the shape is removed.
 */
function commit() {
  const edit = nodeEdit.value;
  if (!edit) return;
  const shape = editor.shapes.find((other) => other.id === edit.shapeId);
  if (!shape) {
    endNodeEdit();
    return;
  }
  const next = pathShape(edit.nodes, edit.closed);
  if (!next) {
    editor.shapes = editor.shapes.filter((other) => other !== shape);
    editor.selectedIds = [];
    endNodeEdit();
    return;
  }
  Object.assign(shape, next);
}

/**
 * The edited path becomes `parts[0]`; any second part becomes a new path (when a
 * segment is deleted or the path is broken in the middle). Parts need 2 nodes.
 */
function split(parts: PathNode[][]) {
  const edit = nodeEdit.value!;
  const kept = parts.filter((part) => part.length >= 2);
  edit.nodes = kept[0] ?? [];
  edit.closed = false;
  commit();
  const extra = kept[1] && pathShape(kept[1], false);
  if (extra) {
    addShape(extra);
    // addShape selects the new path: keep the edited one selected.
    if (nodeEdit.value) select(nodeEdit.value.shapeId, "replace");
  }
}

// ----- Pointer -----

/** The node, handle or segment within `tolerance` mm of `point` (nodes first, then handles). */
export function hitTest(point: Point, tolerance: number): NodeTarget | null {
  const edit = nodeEdit.value;
  if (!edit) return null;
  let best: NodeTarget | null = null;
  let bestDistance = tolerance;
  const consider = (target: NodeTarget, d: number) => {
    if (d <= bestDistance) {
      best = target;
      bestDistance = d;
    }
  };
  edit.nodes.forEach((node, index) => consider({ kind: "node", index }, distance(node.p, point)));
  if (best) return best;
  edit.nodes.forEach((node, index) => {
    for (const which of ["in", "out"] as const) {
      const handle = node[which];
      if (handle) consider({ kind: "handle", index, which }, distance(handle, point));
    }
  });
  if (best) return best;
  for (let index = 0; index < segmentCount(edit.nodes, edit.closed); index++) {
    const [a, b] = segmentNodes(index);
    if (!isCurve(a, b)) {
      // The closest point of a straight segment: projection, kept between its ends.
      const length2 = (b.p.x - a.p.x) ** 2 + (b.p.y - a.p.y) ** 2;
      const t =
        length2 === 0
          ? 0
          : Math.min(1, Math.max(0, ((point.x - a.p.x) * (b.p.x - a.p.x) + (point.y - a.p.y) * (b.p.y - a.p.y)) / length2));
      consider({ kind: "segment", index, t }, distance(pointOnSegment(a, b, t), point));
    } else {
      // A curve: the closest of many points along it.
      const steps = CURVE_SEGMENTS * 4;
      for (let i = 0; i <= steps; i++) {
        const t = i / steps;
        consider({ kind: "segment", index, t }, distance(pointOnSegment(a, b, t), point));
      }
    }
  }
  return best;
}

/** Starts dragging a node or handle; returns false for anything else. */
export function startNodeDrag(target: NodeTarget, point: Point): boolean {
  if (!nodeEdit.value || target.kind === "segment") return false;
  drag = { target, start: point, original: copyNodes(nodeEdit.value.nodes) };
  return true;
}

export function isNodeDragging(): boolean {
  return drag !== null;
}

/**
 * Drag in progress: a node moves with its handles; a handle moves alone, or, on a
 * smooth node, turns the opposite handle with it (keeping that handle's length).
 */
export function nodeDragTo(point: Point) {
  const edit = nodeEdit.value;
  if (!edit || !drag) return;
  const { target, start, original } = drag;
  if (target.kind === "node") {
    const dx = point.x - start.x;
    const dy = point.y - start.y;
    edit.nodes[target.index] = mapNode(original[target.index], (p) => ({ x: p.x + dx, y: p.y + dy }));
  } else if (target.kind === "handle") {
    const node = { ...original[target.index], [target.which]: point };
    const other = target.which === "in" ? "out" : "in";
    const opposite = original[target.index][other];
    if (node.smooth && opposite) {
      const length = distance(opposite, node.p);
      const away = distance(point, node.p);
      if (away > 0) {
        node[other] = {
          x: node.p.x + ((node.p.x - point.x) / away) * length,
          y: node.p.y + ((node.p.y - point.y) / away) * length,
        };
      }
    }
    edit.nodes[target.index] = node;
  }
  commit();
}

/**
 * Ends a drag. An end node of an open path dropped on its other end (within
 * `tolerance` mm) joins them: the path becomes closed.
 */
export function endNodeDrag(tolerance: number) {
  const edit = nodeEdit.value;
  const target = drag?.target;
  drag = null;
  if (!edit || target?.kind !== "node" || edit.closed || edit.nodes.length < 3) return;
  const last = edit.nodes.length - 1;
  const other = target.index === 0 ? last : target.index === last ? 0 : -1;
  if (other < 0 || distance(edit.nodes[target.index].p, edit.nodes[other].p) > tolerance) return;
  if (target.index === last) {
    edit.nodes[0] = { ...edit.nodes[0], in: edit.nodes[last].in };
    edit.nodes.pop();
  } else {
    edit.nodes[last] = { ...edit.nodes[last], out: edit.nodes[0].out };
    edit.nodes.shift();
  }
  edit.closed = true;
  hover.value = null;
  commit();
}

/** Escape during a drag puts the nodes back. */
export function cancelNodeDrag() {
  if (!nodeEdit.value || !drag) return;
  nodeEdit.value.nodes = drag.original;
  drag = null;
  commit();
}

// ----- Keys -----

/** Delete / D: removes the node under the pointer, or the segment (opening or splitting the path). */
function deleteTarget(target: NodeTarget) {
  const edit = nodeEdit.value!;
  if (target.kind === "node") {
    edit.nodes.splice(target.index, 1);
    // A closed path needs 3 nodes to stay a shape.
    if (edit.closed && edit.nodes.length < 3) edit.closed = false;
    commit();
  } else if (target.kind === "segment") {
    const { nodes } = edit;
    const i = target.index;
    if (edit.closed) {
      // The path opens there: it now starts after the segment and ends before it.
      split([[...nodes.slice(i + 1), ...nodes.slice(0, i + 1)]]);
    } else {
      split([nodes.slice(0, i + 1), nodes.slice(i + 1)]);
    }
  }
}

/** B: breaks the path at the node: a closed path opens there, an open one splits in two. */
function breakAt(index: number) {
  const edit = nodeEdit.value!;
  const { nodes } = edit;
  const node = nodes[index];
  const asEnd = { ...node, out: undefined };
  const asStart = { ...node, in: undefined };
  if (edit.closed) {
    split([[asStart, ...nodes.slice(index + 1), ...nodes.slice(0, index), asEnd]]);
  } else if (index > 0 && index < nodes.length - 1) {
    split([
      [...nodes.slice(0, index), asEnd],
      [asStart, ...nodes.slice(index + 1)],
    ]);
  }
}

/** I / M / double-click: a new node at `t` on segment `index`; a curve keeps its shape. */
export function insertNode(index: number, t: number) {
  const edit = nodeEdit.value;
  if (!edit) return;
  const [a, b] = segmentNodes(index);
  let node: PathNode;
  if (!isCurve(a, b)) {
    node = { p: lerp(a.p, b.p, t) };
  } else {
    // De Casteljau: splits the curve at t into two curves that together look the same.
    const [p0, p1, p2, p3] = [a.p, a.out ?? a.p, b.in ?? b.p, b.p];
    const q0 = lerp(p0, p1, t);
    const q1 = lerp(p1, p2, t);
    const q2 = lerp(p2, p3, t);
    const r0 = lerp(q0, q1, t);
    const r1 = lerp(q1, q2, t);
    node = { p: lerp(r0, r1, t), in: r0, out: r1, smooth: true };
    a.out = q0;
    b.in = q2;
  }
  edit.nodes.splice(index + 1, 0, node);
  hover.value = null;
  commit();
}

/** S: makes the node smooth: its handles on one line through it (created if missing). */
function smooth(index: number) {
  const edit = nodeEdit.value!;
  const { nodes, closed } = edit;
  const node = nodes[index];
  const prev = index > 0 ? nodes[index - 1] : closed ? nodes[nodes.length - 1] : null;
  const next = index < nodes.length - 1 ? nodes[index + 1] : closed ? nodes[0] : null;
  // The direction of the curve through the node.
  let dx: number;
  let dy: number;
  if (node.in && node.out) [dx, dy] = [node.out.x - node.in.x, node.out.y - node.in.y];
  else {
    const from = prev?.p ?? node.p;
    const to = next?.p ?? node.p;
    [dx, dy] = [to.x - from.x, to.y - from.y];
  }
  const length = Math.hypot(dx, dy);
  if (length === 0) return;
  [dx, dy] = [dx / length, dy / length];
  const inLength = node.in ? distance(node.in, node.p) : prev ? distance(prev.p, node.p) / 3 : 0;
  const outLength = node.out ? distance(node.out, node.p) : next ? distance(next.p, node.p) / 3 : 0;
  nodes[index] = {
    p: node.p,
    in: prev ? { x: node.p.x - dx * inLength, y: node.p.y - dy * inLength } : undefined,
    out: next ? { x: node.p.x + dx * outLength, y: node.p.y + dy * outLength } : undefined,
    smooth: true,
  };
  commit();
}

/**
 * A key while editing nodes, acting on what is under the pointer. Returns true when
 * the key was used (so it does not also act on the whole shape, e.g. Delete).
 */
export function nodeKey(key: string): boolean {
  const edit = nodeEdit.value;
  const target = hover.value;
  if (!edit || !target) return false;
  const k = key.toLowerCase();
  if (k === "delete" || k === "d") deleteTarget(target);
  else if (target.kind === "node" && k === "b") breakAt(target.index);
  else if (target.kind === "node" && k === "s") smooth(target.index);
  else if (target.kind === "segment" && k === "i") insertNode(target.index, target.t);
  else if (target.kind === "segment" && k === "m") insertNode(target.index, 0.5);
  else if (target.kind === "segment" && (k === "l" || k === "c")) {
    const [a, b] = segmentNodes(target.index);
    if (k === "l") {
      a.out = undefined;
      b.in = undefined;
    } else if (!isCurve(a, b)) {
      // A curve that still looks straight, with handles at a third of the way from each end.
      a.out = lerp(a.p, b.p, 1 / 3);
      b.in = lerp(a.p, b.p, 2 / 3);
    }
    commit();
  } else return false;
  hover.value = null;
  return true;
}
