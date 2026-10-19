// Paths: shapes made of nodes joined by straight or curved (cubic Bézier) segments.
// Draw Lines creates them; Edit Nodes changes them, and turns rectangles, ellipses
// and polygons into them.
import { placeOn, UNIT_POLYGON, type Point, type Shape } from "./editor";

/**
 * A node of a path. `in` and `out` are its curve handles (Bézier control points):
 * the segment arriving at the node curves along `in`, the one leaving along `out`.
 * No handle: the segment is straight at that end. `smooth`: moving one handle
 * turns the other with it, so the curve has no corner there.
 */
export interface PathNode {
  p: Point;
  in?: Point;
  out?: Point;
  smooth?: boolean;
}

/** Straight pieces used to draw and cut one curved segment. */
export const CURVE_SEGMENTS = 24;

/** The point at `t` (0 to 1) on the cubic Bézier curve p0 → p3. */
function bezier(p0: Point, p1: Point, p2: Point, p3: Point, t: number): Point {
  const u = 1 - t;
  return {
    x: u * u * u * p0.x + 3 * u * u * t * p1.x + 3 * u * t * t * p2.x + t * t * t * p3.x,
    y: u * u * u * p0.y + 3 * u * u * t * p1.y + 3 * u * t * t * p2.y + t * t * t * p3.y,
  };
}

/** True when the segment from `a` to `b` is curved (either end has a handle). */
export function isCurve(a: PathNode, b: PathNode): boolean {
  return !!a.out || !!b.in;
}

/** The points of the segment from `a` to `b`, without `a`: one point if straight, else many. */
export function segment(a: PathNode, b: PathNode): Point[] {
  if (!isCurve(a, b)) return [b.p];
  const [p1, p2] = [a.out ?? a.p, b.in ?? b.p];
  return Array.from({ length: CURVE_SEGMENTS }, (_, i) => bezier(a.p, p1, p2, b.p, (i + 1) / CURVE_SEGMENTS));
}

/** The point at `t` (0 to 1) along the segment from `a` to `b`. */
export function pointOnSegment(a: PathNode, b: PathNode, t: number): Point {
  if (!isCurve(a, b)) return { x: a.p.x + (b.p.x - a.p.x) * t, y: a.p.y + (b.p.y - a.p.y) * t };
  return bezier(a.p, a.out ?? a.p, b.in ?? b.p, b.p, t);
}

/** All points of the path; closed: also the segment back to the first node (without repeating it). */
export function flatten(nodes: PathNode[], closed: boolean): Point[] {
  const points = [nodes[0].p];
  for (let i = 1; i < nodes.length; i++) points.push(...segment(nodes[i - 1], nodes[i]));
  if (closed) {
    points.push(...segment(nodes[nodes.length - 1], nodes[0]));
    points.pop();
  }
  return points;
}

/** Applies `f` to a node's point and handles. */
export function mapNode(node: PathNode, f: (p: Point) => Point): PathNode {
  return {
    p: f(node.p),
    in: node.in && f(node.in),
    out: node.out && f(node.out),
    smooth: node.smooth,
  };
}

/**
 * The shape for a path whose nodes are in machine coordinates: the upright box around
 * the drawn path, and the nodes and outline in that box (-1 to 1). Null when the
 * path is a single point.
 */
export function pathShape(nodes: PathNode[], closed: boolean): Omit<Shape, "id"> | null {
  if (nodes.length < 2) return null;
  const points = flatten(nodes, closed);
  const xs = points.map((p) => p.x);
  const ys = points.map((p) => p.y);
  const [minX, maxX, minY, maxY] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  if (maxX - minX < 1e-6 && maxY - minY < 1e-6) return null;
  const rx = (maxX - minX) / 2;
  const ry = (maxY - minY) / 2;
  const cx = (minX + maxX) / 2;
  const cy = (minY + maxY) / 2;
  // A straight horizontal or vertical line has no height or width: its points sit at 0 there.
  const toBox = (p: Point) => ({ x: rx > 0 ? (p.x - cx) / rx : 0, y: ry > 0 ? (p.y - cy) / ry : 0 });
  return {
    type: "path",
    cx,
    cy,
    rx,
    ry,
    angle: 0,
    nodes: nodes.map((node) => mapNode(node, toBox)),
    contours: [points.map(toBox)],
    closed,
  };
}

/**
 * A closed shape from outlines in machine coordinates (e.g. the result of a Weld):
 * a single outline becomes a path with a node at each corner, so Edit Nodes can change it;
 * several (pieces, holes) become one path drawn and cut outline by outline.
 */
export function outlinesShape(outlines: Point[][]): Omit<Shape, "id"> | null {
  if (outlines.length === 1) return pathShape(outlines[0].map((p) => ({ p })), true);
  const points = outlines.flat();
  if (points.length === 0) return null;
  const xs = points.map((p) => p.x);
  const ys = points.map((p) => p.y);
  const [minX, maxX, minY, maxY] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  const rx = (maxX - minX) / 2;
  const ry = (maxY - minY) / 2;
  const cx = (minX + maxX) / 2;
  const cy = (minY + maxY) / 2;
  const toBox = (p: Point) => ({ x: rx > 0 ? (p.x - cx) / rx : 0, y: ry > 0 ? (p.y - cy) / ry : 0 });
  return {
    type: "path",
    cx,
    cy,
    rx,
    ry,
    angle: 0,
    contours: outlines.map((outline) => outline.map(toBox)),
    closed: true,
  };
}

/** Handle length for a quarter of a circle drawn with one cubic Bézier curve (radius 1). */
const KAPPA = 0.5522847498;

/** The nodes of an ellipse, rectangle or polygon in its box (-1 to 1); null for other shapes. */
function unitNodes(shape: Shape): PathNode[] | null {
  switch (shape.type) {
    case "rectangle":
      // Same order as its outline: from the bottom-right corner, counterclockwise.
      return [{ p: { x: 1, y: -1 } }, { p: { x: 1, y: 1 } }, { p: { x: -1, y: 1 } }, { p: { x: -1, y: -1 } }];
    case "polygon":
      return UNIT_POLYGON.map((p) => ({ p }));
    case "ellipse":
      // Four smooth nodes (right, top, left, bottom), each quarter one curve.
      return [
        { p: { x: 1, y: 0 }, in: { x: 1, y: -KAPPA }, out: { x: 1, y: KAPPA }, smooth: true },
        { p: { x: 0, y: 1 }, in: { x: KAPPA, y: 1 }, out: { x: -KAPPA, y: 1 }, smooth: true },
        { p: { x: -1, y: 0 }, in: { x: -1, y: KAPPA }, out: { x: -1, y: -KAPPA }, smooth: true },
        { p: { x: 0, y: -1 }, in: { x: -KAPPA, y: -1 }, out: { x: KAPPA, y: -1 }, smooth: true },
      ];
    default:
      return null;
  }
}

/**
 * A shape's nodes in machine coordinates, with its size and rotation applied;
 * null for a text (it stays text) or a path without nodes.
 */
export function machineNodes(shape: Shape): { nodes: PathNode[]; closed: boolean } | null {
  const unit = shape.type === "path" ? shape.nodes : unitNodes(shape);
  if (!unit || unit.length === 0) return null;
  const closed = shape.type === "path" ? !!shape.closed : true;
  return { nodes: unit.map((node) => mapNode(node, (p) => placeOn(shape, p.x, p.y))), closed };
}
