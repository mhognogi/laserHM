// Tool calls from Claude Desktop (MCP, see src-tauri/src/mcp.rs). Each one changes the
// canvas with the editor's own functions (so Ctrl+Z undoes it) and answers with a short
// text for Claude. Mistakes in the arguments are answered as errors Claude can correct.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { combine, canCombine } from "./boolean";
import { breakApart, canBreakApart } from "./breakApart";
import {
  addShape,
  deleteSelected,
  editor,
  moveGroup,
  rotateSelected,
  snapshotSelected,
  type Point,
  type Shape,
} from "./editor";
import { status } from "./laser";
import { pathShape } from "./paths";
import { createText, fonts, loadFonts, textSettings } from "./text";

type Args = Record<string, unknown>;

/** A mistake in a tool call; its message is sent to Claude. */
class ToolError extends Error {}

/** The number `name` of the arguments; `undefined` only when `optional`. */
function num(args: Args, name: string, optional: true): number | undefined;
function num(args: Args, name: string): number;
function num(args: Args, name: string, optional = false): number | undefined {
  const value = args[name];
  if (value === undefined && optional) return undefined;
  if (typeof value !== "number" || !Number.isFinite(value)) throw new ToolError(`"${name}" must be a number`);
  return value;
}

function positive(args: Args, name: string, optional: true): number | undefined;
function positive(args: Args, name: string): number;
function positive(args: Args, name: string, optional = false): number | undefined {
  const value = optional ? num(args, name, true) : num(args, name);
  if (value !== undefined && value <= 0) throw new ToolError(`"${name}" must be greater than 0`);
  return value;
}

/** The ids in the arguments; each must be a shape on the canvas. */
function shapeIds(args: Args): number[] {
  const ids = args.ids;
  if (!Array.isArray(ids)) throw new ToolError('"ids" must be a list of shape ids');
  for (const id of ids) {
    if (!editor.shapes.some((shape) => shape.id === id)) throw new ToolError(`There is no shape with id ${id}`);
  }
  return ids as number[];
}

function nonEmpty(ids: number[]): number[] {
  if (ids.length === 0) throw new ToolError('"ids" is empty');
  return ids;
}

/** Rounded to 0.001 mm, so the answers stay short. */
const round = (value: number) => Math.round(value * 1000) / 1000;

/** A shape as Claude sees it: center, size and rotation in mm and degrees. */
function describe(shape: Shape) {
  return {
    id: shape.id,
    type: shape.type,
    cx: round(shape.cx),
    cy: round(shape.cy),
    width: round(shape.rx * 2),
    height: round(shape.ry * 2),
    angle: round(shape.angle ?? 0),
    ...(shape.text !== undefined && { text: shape.text, font: shape.font?.name }),
    ...(shape.type === "path" && { closed: !!shape.closed }),
  };
}

/** The tools, by name; each returns the text answered to Claude. */
const tools: Record<string, (args: Args) => string | Promise<string>> = {
  get_canvas() {
    return JSON.stringify({
      workArea: status.workArea,
      shapes: editor.shapes.map(describe),
      selectedIds: editor.selectedIds,
    });
  },

  add_shape(args) {
    const type = args.type;
    if (type !== "rectangle" && type !== "ellipse" && type !== "polygon") {
      throw new ToolError('"type" must be rectangle, ellipse or polygon');
    }
    const id = addShape({
      type,
      cx: num(args, "cx"),
      cy: num(args, "cy"),
      rx: positive(args, "width") / 2,
      ry: positive(args, "height") / 2,
      angle: num(args, "angle", true) ?? 0,
    });
    return `Added ${type} id ${id}`;
  },

  async add_text(args) {
    if (typeof args.text !== "string") throw new ToolError('"text" must be a string');
    await loadFonts();
    let font = textSettings.font;
    if (args.font !== undefined) {
      const wanted = String(args.font).toLowerCase();
      font = fonts.value.find((other) => other.name.toLowerCase() === wanted) ?? null;
      if (!font) throw new ToolError(`Font "${args.font}" is not installed (see list_fonts)`);
    }
    if (!font) throw new ToolError("No font is available");
    const welded = typeof args.welded === "boolean" ? args.welded : textSettings.welded;
    const size = positive(args, "height", true) ?? textSettings.size;
    const id = await createText(args.text, { x: num(args, "x"), y: num(args, "y") }, font, size, welded);
    if (id === null) throw new ToolError("The text has no visible characters, or the font cannot be read");
    const shape = editor.shapes.find((other) => other.id === id)!;
    return `Added text id ${id}: ${JSON.stringify(describe(shape))}`;
  },

  async list_fonts() {
    await loadFonts();
    return [...new Set(fonts.value.map((font) => font.name))].join("\n");
  },

  add_path(args) {
    const points = args.points;
    if (!Array.isArray(points) || points.length < 2) throw new ToolError('"points" needs at least 2 points');
    const nodes = points.map((point: Args) => ({ p: { x: num(point, "x"), y: num(point, "y") } as Point }));
    const shape = pathShape(nodes, args.closed === true);
    if (!shape) throw new ToolError("All points are in the same place");
    return `Added path id ${addShape(shape)}`;
  },

  select(args) {
    editor.selectedIds = shapeIds(args);
    return `Selected: ${JSON.stringify(editor.selectedIds)}`;
  },

  move(args) {
    const ids = nonEmpty(shapeIds(args));
    const [dx, dy] = [num(args, "dx"), num(args, "dy")];
    editor.selectedIds = ids;
    moveGroup(snapshotSelected(), dx, dy);
    return `Moved ${ids.length} shape(s)`;
  },

  set_geometry(args) {
    const shape = editor.shapes.find((other) => other.id === args.id);
    if (!shape) throw new ToolError(`There is no shape with id ${args.id}`);
    const cx = num(args, "cx", true);
    const cy = num(args, "cy", true);
    const width = positive(args, "width", true);
    const height = positive(args, "height", true);
    const angle = num(args, "angle", true);
    if (cx !== undefined) shape.cx = cx;
    if (cy !== undefined) shape.cy = cy;
    if (width !== undefined) shape.rx = width / 2;
    if (height !== undefined) shape.ry = height / 2;
    if (angle !== undefined) shape.angle = angle;
    editor.selectedIds = [shape.id];
    return JSON.stringify(describe(shape));
  },

  rotate(args) {
    const ids = nonEmpty(shapeIds(args));
    const degrees = num(args, "degrees");
    editor.selectedIds = ids;
    rotateSelected(degrees);
    return `Rotated ${ids.length} shape(s) by ${degrees}°`;
  },

  duplicate(args) {
    const ids = nonEmpty(shapeIds(args));
    const [dx, dy] = [num(args, "dx"), num(args, "dy")];
    const copies = ids.map((id) => {
      const { id: _id, ...shape } = editor.shapes.find((other) => other.id === id)!;
      // A deep copy: the copy must not share outlines or nodes with the original.
      const copy = JSON.parse(JSON.stringify(shape)) as Omit<Shape, "id">;
      return addShape({ ...copy, cx: copy.cx + dx, cy: copy.cy + dy });
    });
    editor.selectedIds = copies;
    return `New ids: ${JSON.stringify(copies)}`;
  },

  delete(args) {
    const ids = nonEmpty(shapeIds(args));
    editor.selectedIds = ids;
    deleteSelected();
    return `Deleted ${ids.length} shape(s)`;
  },

  async boolean(args) {
    const op = args.op;
    if (op !== "weld" && op !== "subtract" && op !== "intersect") {
      throw new ToolError('"op" must be weld, subtract or intersect');
    }
    // The order matters for subtract: the second is removed from the first.
    editor.selectedIds = shapeIds(args);
    if (!canCombine(op)) throw new ToolError(op === "weld" ? "Weld needs 2 or more shapes" : `${op} needs exactly 2 shapes`);
    const error = await combine(op);
    if (error) throw new ToolError(error);
    return `Result id ${editor.selectedIds[0]}`;
  },

  break_apart(args) {
    editor.selectedIds = nonEmpty(shapeIds(args));
    if (!canBreakApart()) throw new ToolError("None of these shapes is made of separate pieces");
    breakApart();
    return `New ids: ${JSON.stringify(editor.selectedIds)}`;
  },
};

/** Runs one tool call and sends the answer back to Rust. */
async function run(id: number, tool: string, args: Args) {
  let ok = true;
  let text: string;
  try {
    const handler = tools[tool];
    if (!handler) throw new ToolError(`Unknown tool "${tool}"`);
    text = await handler(args ?? {});
  } catch (error) {
    ok = false;
    text = error instanceof ToolError ? error.message : `laserHM error: ${String(error)}`;
  }
  await invoke("mcp_respond", { id, ok, text });
}

listen<{ id: number; tool: string; arguments: Args }>("mcp-request", ({ payload }) => {
  run(payload.id, payload.tool, payload.arguments);
});
