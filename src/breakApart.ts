// Break Apart (named as in LightBurn, Arrange → Break Apart): a shape made of several
// separate pieces (e.g. a welded text "Ascd": "A", "s" and "cd") becomes one shape per
// piece. Unlike LightBurn, each hole stays with the piece it is in (the inside of "A" and "d").
import { addShape, editor, outlines, selectedShapes, type Point, type Shape } from "./editor";
import { outlinesShape } from "./paths";

/** Whether `point` is inside the closed outline (even-odd rule). */
function inside(point: Point, outline: Point[]): boolean {
  let result = false;
  for (let i = 0, j = outline.length - 1; i < outline.length; j = i++) {
    const [a, b] = [outline[i], outline[j]];
    if (a.y > point.y !== b.y > point.y && point.x < ((b.x - a.x) * (point.y - a.y)) / (b.y - a.y) + a.x) {
      result = !result;
    }
  }
  return result;
}

/**
 * The outlines grouped into pieces: each outer outline with the holes directly inside it.
 * An outline inside an even number of others is an outer one (a piece; also an island
 * inside a hole), inside an odd number it is a hole of the smallest outer one around it.
 */
function pieces(all: Point[][]): Point[][][] {
  const containers = all.map((outline, i) =>
    all.flatMap((other, j) => (j !== i && inside(outline[0], other) ? [j] : [])),
  );
  const depth = containers.map((list) => list.length);
  const groups = new Map<number, Point[][]>();
  all.forEach((outline, i) => {
    if (depth[i] % 2 === 0) groups.set(i, [outline]);
  });
  all.forEach((outline, i) => {
    if (depth[i] % 2 === 0) return;
    // The outer outline right around this hole is the one container nested one level less.
    const parent = containers[i].find((j) => depth[j] === depth[i] - 1);
    if (parent !== undefined) groups.get(parent)!.push(outline);
  });
  return [...groups.values()];
}

/** Whether a selected shape has more than one piece (only texts and paths can). */
export function canBreakApart(): boolean {
  return selectedShapes().some((shape) => isSplittable(shape) && pieces(outlines(shape)).length > 1);
}

/** Open paths and texts being single outlines have nothing to split. */
function isSplittable(shape: Shape): boolean {
  return (shape.type === "text" || (shape.type === "path" && !!shape.closed)) && (shape.contours?.length ?? 0) > 1;
}

/** Each selected shape with several pieces is replaced by one path per piece; the pieces are selected. */
export function breakApart() {
  const newIds: number[] = [];
  const keptIds: number[] = [];
  for (const shape of selectedShapes()) {
    const parts = isSplittable(shape) ? pieces(outlines(shape)) : [];
    if (parts.length < 2) {
      keptIds.push(shape.id);
      continue;
    }
    editor.shapes = editor.shapes.filter((other) => other.id !== shape.id);
    for (const part of parts) {
      const piece = outlinesShape(part);
      if (piece) newIds.push(addShape(piece));
    }
  }
  editor.selectedIds = [...keptIds, ...newIds];
}
