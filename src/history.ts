// Undo (Ctrl+Z) and Redo (Ctrl+Shift+Z): the shapes are remembered after every finished change. A change in
// progress (a drag, a text being typed) is remembered only when it ends, so it is undone
// in one step.
import { watch } from "vue";
import { editor } from "./editor";
import { endNodeEdit, nodeEdit, startNodeEdit } from "./nodes";
import { finishText, textEdit } from "./text";

/** How many changes can be undone; older ones are forgotten. */
const UNDO_LEVELS = 100;

/**
 * A remembered state: each shape as JSON text. A shape that did not change reuses the
 * text of the previous state (the same string, not a copy), so a step costs memory only
 * for the shapes it changed.
 */
type State = { id: number; json: string }[];

/** The last finished state, and the ones before it (the newest last). */
let current: State = [];
const undoStack: State[] = [];
/** States undone, to redo them (the next to redo last); a new change clears them. */
const redoStack: State[] = [];

/** The shapes as they are now, reusing the texts of `current` where nothing changed. */
function capture(): State {
  const known = new Map(current.map((entry) => [entry.id, entry.json]));
  return editor.shapes.map((shape) => {
    const json = JSON.stringify(shape);
    const old = known.get(shape.id);
    return { id: shape.id, json: old === json ? old : json };
  });
}

function same(a: State, b: State): boolean {
  return a.length === b.length && a.every((entry, i) => entry.id === b[i].id && entry.json === b[i].json);
}

/** A mouse button is held: a drag may be in progress. */
let pointerHeld = false;

/** Remembers the shapes if they changed and no change is in progress. */
function remember() {
  if (pointerHeld || textEdit.value) return;
  const next = capture();
  if (same(next, current)) return;
  undoStack.push(current);
  if (undoStack.length > UNDO_LEVELS) undoStack.shift();
  redoStack.length = 0;
  current = next;
}

current = capture();
watch(() => editor.shapes, remember, { deep: true });
// Typing a text ends.
watch(textEdit, remember);

// Capture: these run before the app's own handlers, so a drag ending is seen after
// them (setTimeout), once the shapes have their final place.
window.addEventListener("pointerdown", () => (pointerHeld = true), true);
for (const type of ["pointerup", "pointercancel"]) {
  window.addEventListener(
    type,
    () => {
      pointerHeld = false;
      setTimeout(remember);
    },
    true,
  );
}

/**
 * Ctrl+Z: puts the shapes back as they were before the last change and selects the
 * shapes it brings back or changes. A text being typed is finished first.
 */
export function undo() {
  finishText();
  remember();
  step(undoStack, redoStack);
}

/** Ctrl+Shift+Z: does again the last change undone. */
export function redo() {
  finishText();
  remember();
  step(redoStack, undoStack);
}

/** Goes to the last state of `from`, keeping the current one in `to` to come back. */
function step(from: State[], to: State[]) {
  const target = from.pop();
  if (!target) return;
  to.push(current);
  const now = new Set(current.map((entry) => entry.json));
  current = target;
  editor.shapes = target.map((entry) => JSON.parse(entry.json));
  // Select the shapes brought back or changed.
  editor.selectedIds = target.filter((entry) => !now.has(entry.json)).map((entry) => entry.id);

  // Edit Nodes shows the nodes again from the restored shape (or ends if it is gone).
  if (nodeEdit.value) {
    const shape = editor.shapes.find((other) => other.id === nodeEdit.value!.shapeId);
    endNodeEdit();
    if (shape) startNodeEdit(shape);
  }
}
