// Images (File > Import), engraved line by line as in LightBurn: the picture is turned
// into a grid of dots, one every Line Interval, each burned with its own power.
import { ref } from "vue";
import { addShape, editor, type Shape } from "./editor";
import { status } from "./laser";

/** How the gray of each dot becomes a burn, as LightBurn's Image Mode. */
export type ImageMode = "threshold" | "floyd" | "jarvis" | "stucki" | "grayscale";

export const IMAGE_MODES: { id: ImageMode; name: string; title: string }[] = [
  { id: "threshold", name: "Threshold", title: "Each dot black or white: darker than middle gray burns" },
  { id: "floyd", name: "Floyd-Steinberg", title: "Dithering: gray made of more or fewer burned dots; fine, a little noisy" },
  { id: "jarvis", name: "Jarvis", title: "Dithering spread over more neighbours: smoother than Floyd-Steinberg" },
  { id: "stucki", name: "Stucki", title: "Dithering like Jarvis, a little sharper; good for photos" },
  { id: "grayscale", name: "Grayscale", title: "The power follows the gray, from Power min (white) to Power max (black)" },
];

/** An image's settings, kept on its shape (so Undo restores them). */
export interface ImageSettings {
  mode: ImageMode;
  /** Distance between engraved lines and between dots on a line, in mm. */
  interval: number;
  /** Burn the light parts instead of the dark ones. */
  negative: boolean;
  /** -100 to 100. */
  contrast: number;
  brightness: number;
  /** 1: unchanged; above 1 lighter mid-tones, below 1 darker. */
  gamma: number;
  /** mm/min */
  speed: number;
  /** % of the laser's maximum: for a black dot, and (Grayscale only) for a white one. */
  powerMax: number;
  powerMin: number;
}

export const DEFAULT_IMAGE_SETTINGS: ImageSettings = {
  mode: "stucki",
  interval: 0.1,
  negative: false,
  contrast: 0,
  brightness: 0,
  gamma: 1,
  speed: 3000,
  powerMax: 50,
  powerMin: 0,
};

/** An imported picture, loaded and ready to draw; shapes refer to it by id. */
interface ImageSource {
  element: HTMLImageElement;
  /** The whole file as a data URL ("data:image/png;base64,…"), so a project can hold it. */
  url: string;
}

/** The imported pictures; never removed, so Undo can bring a deleted image back. */
const sources = new Map<number, ImageSource>();
let nextImageId = 1;

export function imageUrl(shape: Shape): string {
  return sources.get(shape.imageId ?? 0)?.url ?? "";
}

/** Loads a picture from its data URL and keeps it under a new id, which it returns. */
async function addSource(url: string): Promise<number> {
  const element = new Image();
  element.src = url;
  await element.decode();
  const imageId = nextImageId++;
  sources.set(imageId, { element, url });
  return imageId;
}

/** The pictures the shapes use, by id, as data URLs (for a project file). */
export function imagesOf(shapes: Shape[]): Record<number, string> {
  const images: Record<number, string> = {};
  for (const shape of shapes) {
    const source = sources.get(shape.imageId ?? 0);
    if (source) images[shape.imageId!] = source.url;
  }
  return images;
}

/**
 * Loads the pictures of a project and points its shapes to them. They get new ids, so
 * they never mix with pictures already loaded (and their previews) under the same number.
 */
export async function restoreImages(shapes: Shape[], images: Record<number, string>) {
  const ids = new Map<number, number>();
  for (const [id, url] of Object.entries(images)) ids.set(Number(id), await addSource(url));
  for (const shape of shapes) {
    if (shape.imageId !== undefined) shape.imageId = ids.get(shape.imageId);
  }
}

/** The image whose Adjust Image dialog is open, or null. */
export const adjustImageId = ref<number | null>(null);

/** DPI used when the file does not say: 0.1 mm per pixel, the default Line Interval. */
const DEFAULT_DPI = 254;

/**
 * The DPI written in the file, or null: PNG "pHYs" chunk, JPEG "JFIF" header,
 * BMP header (the last two are pixels per meter).
 */
function fileDpi(bytes: Uint8Array): number | null {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const text = (at: number, length: number) => String.fromCharCode(...bytes.subarray(at, at + length));
  // PNG: chunks of [length, type, data, CRC] after the 8-byte signature.
  if (text(1, 3) === "PNG") {
    for (let at = 8; at + 8 <= bytes.length; ) {
      const length = view.getUint32(at);
      if (text(at + 4, 4) === "pHYs") {
        // Pixels per unit (X), and the unit: 1 = meter.
        return bytes[at + 16] === 1 ? view.getUint32(at + 8) * 0.0254 : null;
      }
      if (text(at + 4, 4) === "IDAT") return null;
      at += 12 + length;
    }
    return null;
  }
  // JPEG: the JFIF segment (APP0) right after the start marker.
  if (bytes[0] === 0xff && bytes[1] === 0xd8 && text(6, 4) === "JFIF") {
    const units = bytes[13];
    const density = view.getUint16(14);
    if (units === 1) return density;
    if (units === 2) return density * 2.54;
    return null;
  }
  // BMP: horizontal pixels per meter at byte 38.
  if (text(0, 2) === "BM" && bytes.length > 42) {
    const perMeter = view.getInt32(38, true);
    return perMeter > 0 ? perMeter * 0.0254 : null;
  }
  return null;
}

/**
 * Adds a picture file to the canvas, centered on the work area and selected, at the size
 * its DPI gives (254 DPI when the file does not say). Returns the new shape.
 */
export async function importImage(file: File): Promise<Shape> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  const url = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
  const imageId = await addSource(url);
  const element = sources.get(imageId)!.element;

  const dpi = fileDpi(bytes) ?? DEFAULT_DPI;
  const area = status.workArea ?? { width: 200, height: 200 };
  const mmPerPixel = 25.4 / dpi;
  addShape({
    type: "image",
    cx: area.width / 2,
    cy: area.height / 2,
    rx: (element.naturalWidth * mmPerPixel) / 2,
    ry: (element.naturalHeight * mmPerPixel) / 2,
    angle: 0,
    imageId,
    imageSettings: { ...DEFAULT_IMAGE_SETTINGS },
  });
  return editor.shapes[editor.shapes.length - 1];
}

/** The dots to burn: `values` row by row from the top, 0 = no burn, 255 = full burn. */
export interface Raster {
  columns: number;
  rows: number;
  values: Uint8Array;
}

/** Error diffusion: [dx, dy, weight] of each neighbour and the weights' sum. */
const DIFFUSION: Record<"floyd" | "jarvis" | "stucki", { spread: [number, number, number][]; total: number }> = {
  floyd: {
    spread: [[1, 0, 7], [-1, 1, 3], [0, 1, 5], [1, 1, 1]],
    total: 16,
  },
  jarvis: {
    spread: [
      [1, 0, 7], [2, 0, 5],
      [-2, 1, 3], [-1, 1, 5], [0, 1, 7], [1, 1, 5], [2, 1, 3],
      [-2, 2, 1], [-1, 2, 3], [0, 2, 5], [1, 2, 3], [2, 2, 1],
    ],
    total: 48,
  },
  stucki: {
    spread: [
      [1, 0, 8], [2, 0, 4],
      [-2, 1, 2], [-1, 1, 4], [0, 1, 8], [1, 1, 4], [2, 1, 2],
      [-2, 2, 1], [-1, 2, 2], [0, 2, 4], [1, 2, 2], [2, 2, 1],
    ],
    total: 42,
  },
};

/**
 * The image at the shape's size as a grid of dots `interval` apart, with the settings
 * applied: brightness, contrast, gamma, negative, then the Image Mode.
 */
export function rasterize(shape: Shape, settings: ImageSettings): Raster | null {
  const source = sources.get(shape.imageId ?? 0);
  if (!source || !(settings.interval > 0)) return null;
  const columns = Math.max(1, Math.round((2 * shape.rx) / settings.interval));
  const rows = Math.max(1, Math.round((2 * shape.ry) / settings.interval));

  // The browser scales the picture to one pixel per dot; transparent parts count as white.
  const canvas = document.createElement("canvas");
  canvas.width = columns;
  canvas.height = rows;
  const context = canvas.getContext("2d", { willReadFrequently: true })!;
  context.fillStyle = "#fff";
  context.fillRect(0, 0, columns, rows);
  context.imageSmoothingQuality = "high";
  context.drawImage(source.element, 0, 0, columns, rows);
  const pixels = context.getImageData(0, 0, columns, rows).data;

  // Contrast as in common image editors: the factor stretches the grays around the middle.
  const c = settings.contrast * 2.55;
  const contrast = (259 * (c + 255)) / (255 * (259 - c));
  const gray = new Float32Array(columns * rows);
  for (let i = 0; i < gray.length; i++) {
    let value = 0.299 * pixels[4 * i] + 0.587 * pixels[4 * i + 1] + 0.114 * pixels[4 * i + 2];
    value += settings.brightness * 2.55;
    value = contrast * (value - 128) + 128;
    value = 255 * Math.pow(Math.min(Math.max(value, 0), 255) / 255, 1 / settings.gamma);
    gray[i] = settings.negative ? 255 - value : value;
  }

  const values = new Uint8Array(columns * rows);
  if (settings.mode === "grayscale") {
    for (let i = 0; i < gray.length; i++) values[i] = Math.round(255 - gray[i]);
  } else if (settings.mode === "threshold") {
    for (let i = 0; i < gray.length; i++) values[i] = gray[i] < 128 ? 255 : 0;
  } else {
    // Each dot becomes black or white; the difference is passed on to the next dots.
    const { spread, total } = DIFFUSION[settings.mode];
    for (let y = 0; y < rows; y++) {
      for (let x = 0; x < columns; x++) {
        const i = y * columns + x;
        const black = gray[i] < 128;
        values[i] = black ? 255 : 0;
        const error = gray[i] - (black ? 0 : 255);
        for (const [dx, dy, weight] of spread) {
          const nx = x + dx;
          const ny = y + dy;
          if (nx >= 0 && nx < columns && ny < rows) gray[ny * columns + nx] += (error * weight) / total;
        }
      }
    }
  }
  return { columns, rows, values };
}

/** The raster as a picture (burned dots dark), for the previews. */
export function rasterUrl(raster: Raster): string {
  const canvas = document.createElement("canvas");
  canvas.width = raster.columns;
  canvas.height = raster.rows;
  const context = canvas.getContext("2d")!;
  const picture = context.createImageData(raster.columns, raster.rows);
  raster.values.forEach((value, i) => {
    const level = 255 - value;
    picture.data.set([level, level, level, 255], 4 * i);
  });
  context.putImageData(picture, 0, 0);
  return canvas.toDataURL();
}

/** Canvas previews already made, by image and settings (resizing only stretches them). */
const previews = new Map<string, string>();

/** How the shape will be engraved, as shown on the canvas. */
export function previewUrl(shape: Shape): string {
  const settings = shape.imageSettings ?? DEFAULT_IMAGE_SETTINGS;
  const key = `${shape.imageId}|${JSON.stringify(settings)}`;
  let url = previews.get(key);
  if (!url) {
    const raster = rasterize(shape, settings);
    url = raster ? rasterUrl(raster) : imageUrl(shape);
    previews.set(key, url);
  }
  return url;
}

/** The image as Rust engraves it (see `Raster` in src-tauri/src/job.rs). */
export function rasterData(shape: Shape) {
  const settings = shape.imageSettings ?? DEFAULT_IMAGE_SETTINGS;
  const raster = rasterize(shape, settings);
  if (!raster) return undefined;
  return {
    columns: raster.columns,
    rows: raster.rows,
    values: Array.from(raster.values),
    speed: settings.speed,
    powerMax: settings.powerMax,
    // Only Grayscale burns white dots at a minimum power; the other modes leave them.
    powerMin: settings.mode === "grayscale" ? settings.powerMin : 0,
  };
}
