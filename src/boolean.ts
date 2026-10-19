// Weld, Union, Subtract and Intersection of the selected shapes, as in LightBurn.
// Rust computes the result (src-tauri/src/boolean.rs); it replaces the selected shapes.
import { invoke } from "@tauri-apps/api/core";
import { addShape, editor, type Point, type Shape } from "./editor";
import { shapeData, status } from "./laser";
import { outlinesShape } from "./paths";

/** "subtractReverse": Subtract with the order swapped (removes the first shape from the second). */
export type BooleanOp = "weld" | "union" | "subtract" | "subtractReverse" | "intersect";

/** The selected shapes in the order they were selected (Subtract removes the second from the first). */
function selectedInOrder(): Shape[] {
  return editor.selectedIds
    .map((id) => editor.shapes.find((shape) => shape.id === id))
    .filter((shape): shape is Shape => !!shape);
}

/** Whether `op` can run on the current selection: Weld needs 2 or more shapes, the others exactly 2. */
export function canCombine(op: BooleanOp): boolean {
  const count = editor.selectedIds.length;
  // An image has no outline to combine (it would become a rectangle).
  if (selectedInOrder().some((shape) => shape.type === "image")) return false;
  return op === "weld" ? count >= 2 : count === 2;
}

/** Runs `op` on the selection; the error, if any, is shown and also returned (for Claude, see mcp.ts). */
export async function combine(op: BooleanOp): Promise<string | null> {
  const shapes = selectedInOrder();
  // Rust knows only Subtract: the reverse one gives it the shapes the other way round.
  const reverse = op === "subtractReverse";
  const sent = reverse ? [...shapes].reverse() : shapes;
  try {
    const outlines = await invoke<Point[][]>("boolean", {
      op: reverse ? "subtract" : op,
      shapes: sent.map(shapeData),
    });
    const result = outlinesShape(outlines);
    if (!result) return "The result is empty";
    const used = new Set(shapes.map((shape) => shape.id));
    editor.shapes = editor.shapes.filter((shape) => !used.has(shape.id));
    // The result is added and selected.
    addShape(result);
    return null;
  } catch (error) {
    // E.g. open lines, or shapes that do not overlap (Intersection).
    status.message = String(error);
    status.error = true;
    return String(error);
  }
}
