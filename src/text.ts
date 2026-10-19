// Text shapes: the installed fonts, the font and size settings, and typing a text
// directly on the canvas (its outlines come from Rust, see src-tauri/src/text.rs).
import { reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
  addShape,
  editor,
  placeOn,
  rotateVector,
  type FontInfo,
  type Geometry,
  type Point,
  type Shape,
} from "./editor";
import { status } from "./laser";

/** Size of a new text: the height of its capitals, in mm (like LightBurn). */
const DEFAULT_SIZE = 15;
/** Font picked first, if it is installed. */
const DEFAULT_FONT = "Arial";

/** The installed fonts, loaded once from Rust. */
export const fonts = ref<FontInfo[]>([]);

/** The font, size and welding used for the next text, shown in the top bar. */
export const textSettings = reactive({ font: null as FontInfo | null, size: DEFAULT_SIZE, welded: true });

/** Loads the font list; the first call takes a moment while Rust reads the font files. */
export async function loadFonts() {
  if (fonts.value.length > 0) return;
  fonts.value = await invoke<FontInfo[]>("list_fonts");
  textSettings.font ??= fonts.value.find((font) => font.name === DEFAULT_FONT) ?? fonts.value[0] ?? null;
}

/** Same as the Rust `TextOutline`: the unit is a capital's height, the origin the first line's start. */
interface TextOutline {
  contours: Point[][];
  /** The end of the last line, on its baseline. */
  caret: Point;
}

function outlineOf(content: string, font: FontInfo, welded: boolean) {
  return invoke<TextOutline>("text_outline", { text: content, path: font.path, index: font.index, welded });
}

/** Shows an error from Rust (e.g. an unreadable font) in the laser panel's status. */
function showError(error: unknown) {
  status.message = String(error);
  status.error = true;
}

/** A text's size: the height of its capitals, in mm. It follows resizing on the canvas. */
export function textSize(shape: Shape): number {
  return 2 * shape.ry * (shape.capHeight ?? 1);
}

/** Where a text's first line starts (on its baseline), in mm. */
function anchorOf(shape: Shape): Point {
  const origin = shape.origin ?? { x: -1, y: -1 };
  return placeOn(shape, origin.x, origin.y);
}

/** A point given in capital heights from the first line's start, placed in mm. */
function fromAnchor(anchor: Point, size: number, angle: number, p: Point): Point {
  const turned = rotateVector(p.x * size, p.y * size, angle);
  return { x: anchor.x + turned.x, y: anchor.y + turned.y };
}

/**
 * Places an outline with its first line starting at `anchor`, capitals `size` mm tall,
 * turned by `angle` around the anchor: the box around the letters, and the letters
 * and origin in that box (-1 to 1). Null when the text has no visible characters.
 */
function layout(outline: TextOutline, anchor: Point, size: number, angle: number) {
  const points = outline.contours.flat();
  if (points.length === 0) return null;
  const xs = points.map((p) => p.x);
  const ys = points.map((p) => p.y);
  const [minX, maxX, minY, maxY] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  if (maxX <= minX || maxY <= minY) return null;
  const toBox = (p: Point) => ({
    x: ((p.x - minX) / (maxX - minX)) * 2 - 1,
    y: ((p.y - minY) / (maxY - minY)) * 2 - 1,
  });
  const center = fromAnchor(anchor, size, angle, { x: (minX + maxX) / 2, y: (minY + maxY) / 2 });
  const geometry: Geometry = {
    cx: center.x,
    cy: center.y,
    rx: ((maxX - minX) / 2) * size,
    ry: ((maxY - minY) / 2) * size,
    angle,
  };
  return {
    ...geometry,
    contours: outline.contours.map((contour) => contour.map(toBox)),
    capHeight: 1 / (maxY - minY),
    origin: toBox({ x: 0, y: 0 }),
  };
}

// ----- Typing on the canvas -----

/** The text being typed, or null. `shapeId` is null until the text has a visible character. */
interface TextEdit {
  shapeId: number | null;
  /** Where the first line starts, in mm. */
  anchor: Point;
  content: string;
  font: FontInfo;
  size: number;
  /** Overlapping letters are joined into one outline. */
  welded: boolean;
  /** Rotation around the anchor, in degrees (from a turned text being edited). */
  angle: number;
  /** Where the next character goes, in mm. */
  caret: Point;
  /** Counts the requests to Rust, so only the latest answer is used. */
  request: number;
}

export const textEdit = ref<TextEdit | null>(null);

/** Text tool: a click starts a new, empty text there (its first line starts at the click). */
export function startText(point: Point) {
  finishText();
  const font = textSettings.font;
  if (!font) return;
  textEdit.value = {
    shapeId: null,
    anchor: point,
    content: "",
    font,
    size: textSettings.size,
    welded: textSettings.welded,
    angle: 0,
    caret: point,
    request: 0,
  };
}

/** Double-click on a text: continue typing at its end. */
export function editText(shape: Shape) {
  finishText();
  if (!shape.font) return;
  textEdit.value = {
    shapeId: shape.id,
    anchor: anchorOf(shape),
    content: shape.text ?? "",
    font: shape.font,
    size: textSize(shape),
    welded: shape.welded ?? false,
    angle: shape.angle ?? 0,
    caret: anchorOf(shape),
    request: 0,
  };
  refresh(textEdit.value);
}

/** Escape, a click elsewhere or another tool ends the typing. */
export function finishText() {
  textEdit.value = null;
}

// Choosing another tool ends the typing.
watch(() => editor.tool, finishText);

/** A typed key: a character, Enter (new line) or Backspace (removes the last character). */
export function typeKey(key: string) {
  const edit = textEdit.value;
  if (!edit) return;
  if (key === "Backspace") edit.content = Array.from(edit.content).slice(0, -1).join("");
  else if (key === "Enter") edit.content += "\n";
  else edit.content += key;
  refresh(edit);
}

/** Asks Rust for the new outlines and updates the shape: created by the first visible
 *  character, removed when none is left. Applies even if the typing ended meanwhile. */
async function refresh(edit: TextEdit) {
  const request = ++edit.request;
  let outline: TextOutline;
  try {
    outline = await outlineOf(edit.content, edit.font, edit.welded);
  } catch (error) {
    showError(error);
    return;
  }
  // A newer key was typed while waiting: its answer will do the update.
  if (request !== edit.request) return;

  edit.caret = fromAnchor(edit.anchor, edit.size, edit.angle, outline.caret);
  const placed = layout(outline, edit.anchor, edit.size, edit.angle);
  const index = editor.shapes.findIndex((shape) => shape.id === edit.shapeId);
  if (!placed) {
    if (index >= 0) editor.shapes.splice(index, 1);
    editor.selectedIds = editor.selectedIds.filter((id) => id !== edit.shapeId);
    edit.shapeId = null;
  } else if (index >= 0) {
    Object.assign(editor.shapes[index], { text: edit.content, font: edit.font, welded: edit.welded, ...placed });
  } else {
    edit.shapeId = addShape({ type: "text", text: edit.content, font: edit.font, welded: edit.welded, ...placed });
  }
}

/**
 * Creates a text whose first line starts at `anchor` (e.g. asked by Claude, see mcp.ts).
 * Its id, or null when it has no visible characters or the font cannot be read.
 */
export async function createText(
  content: string,
  anchor: Point,
  font: FontInfo,
  size: number,
  welded: boolean,
): Promise<number | null> {
  const edit: TextEdit = { shapeId: null, anchor, content, font, size, welded, angle: 0, caret: anchor, request: 0 };
  await refresh(edit);
  return edit.shapeId;
}

/**
 * The font, size or welding changed in the top bar: applies to the text being typed,
 * otherwise to the selected text (its first line keeps its place), and to the next text.
 */
export function applyTextSettings(font: FontInfo, size: number, welded: boolean, selected: Shape | null) {
  Object.assign(textSettings, { font, size, welded });
  const edit = textEdit.value;
  if (edit) {
    Object.assign(edit, { font, size, welded });
    refresh(edit);
  } else if (selected) {
    refresh({
      shapeId: selected.id,
      anchor: anchorOf(selected),
      content: selected.text ?? "",
      font,
      size,
      welded,
      angle: selected.angle ?? 0,
      caret: anchorOf(selected),
      request: 0,
    });
  }
}
