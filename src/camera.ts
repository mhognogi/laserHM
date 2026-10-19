// Camera overlay: live video from a camera, shown over the work area under the shapes.
// Uncalibrated, the image is stretched over the whole bed. Calibration: with a cut square
// selected, click 9 of its points in the image (corners, middles of the sides, center).
// Each quarter of the square then gets its own perspective, so its 4 points fall on the
// real ones; this also follows a lens distortion, piece by piece.
import { computed, reactive, ref, shallowRef } from "vue";
import { selectionBox, type Point } from "./editor";
import { call, status } from "./laser";

export const camera = reactive({
  on: false,
  /** The chosen camera; "" = the system's default camera. */
  deviceId: "",
  /** Cameras found; their names are known only after the first permission to use a camera. */
  devices: [] as { deviceId: string; label: string }[],
  /** Opacity of the video over the bed, 0 to 1. */
  opacity: 0.6,
});

/** The running video; shallowRef: Vue should not look inside the browser's stream object. */
export const cameraStream = shallowRef<MediaStream | null>(null);

async function listCameras() {
  const all = await navigator.mediaDevices.enumerateDevices();
  camera.devices = all
    .filter((device) => device.kind === "videoinput")
    .map((device, index) => ({ deviceId: device.deviceId, label: device.label || `Camera ${index + 1}` }));
}

/** Starts the video from the chosen camera (asks Windows for permission the first time). */
export async function startCamera() {
  stopCamera();
  try {
    const stream = await navigator.mediaDevices.getUserMedia({
      video: camera.deviceId ? { deviceId: { exact: camera.deviceId } } : true,
      audio: false,
    });
    cameraStream.value = stream;
    camera.on = true;
    await listCameras();
    // Remember which camera the default turned out to be, for the list.
    camera.deviceId ||= stream.getVideoTracks()[0]?.getSettings().deviceId ?? "";
  } catch (error) {
    camera.on = false;
    status.message = `Camera: ${error instanceof Error ? error.message : String(error)}`;
    status.error = true;
  }
}

/** Stops the video and frees the camera. */
export function stopCamera() {
  cameraStream.value?.getTracks().forEach((track) => track.stop());
  cameraStream.value = null;
  camera.on = false;
  calibrating.value = null;
}

export function toggleCamera() {
  if (camera.on) stopCamera();
  else startCamera();
}

/** Another camera chosen in the list: switch to it if the video is on. */
export function chooseCamera(deviceId: string) {
  camera.deviceId = deviceId;
  if (camera.on) startCamera();
}

// ----- Calibration -----

/**
 * The square's points clicked during calibration, in this order; `x`/`y` place each one
 * in the square (0 = left/bottom, 0.5 = middle, 1 = right/top). The hint names the next one.
 */
export const CALIBRATION_POINTS = [
  { name: "bottom-left corner", x: 0, y: 0 },
  { name: "bottom-right corner", x: 1, y: 0 },
  { name: "top-right corner", x: 1, y: 1 },
  { name: "top-left corner", x: 0, y: 1 },
  { name: "middle of the bottom side", x: 0.5, y: 0 },
  { name: "middle of the right side", x: 1, y: 0.5 },
  { name: "middle of the top side", x: 0.5, y: 1 },
  { name: "middle of the left side", x: 0, y: 0.5 },
  { name: "center", x: 0.5, y: 0.5 },
];

/** The square's quarters, as indexes into CALIBRATION_POINTS (bottom-left, bottom-right, top-right, top-left of each). */
const QUARTERS = [
  [0, 4, 8, 7],
  [4, 1, 5, 8],
  [8, 5, 2, 6],
  [7, 8, 6, 3],
];

/**
 * The calibration in progress, or null: the square's points on the bed (mm, in the
 * order of CALIBRATION_POINTS) and the points clicked so far in the image (0 to 1).
 * Meanwhile the image is shown stretched over the bed, as before calibration.
 */
export const calibrating = ref<{ bed: Point[]; image: Point[] } | null>(null);

/** The instructions popup is shown; it opens with each calibration and can be closed. */
export const calibrationHelp = ref(false);

/** Calibration needs the camera on and the cut square (one shape) selected. */
export function canCalibrate(): boolean {
  return camera.on && !!selectionBox();
}

/** Starts the calibration with the points of the selection's box (the cut square). */
export function startCalibration() {
  const box = selectionBox();
  if (!camera.on || !box) return;
  calibrating.value = {
    bed: CALIBRATION_POINTS.map(({ x, y }) => ({
      x: box.left + x * (box.right - box.left),
      y: box.bottom + y * (box.top - box.bottom),
    })),
    image: [],
  };
  calibrationHelp.value = true;
}

export function cancelCalibration() {
  calibrating.value = null;
}

/** A click during calibration, at `point` (mm) on the stretched image; the last one saves it. */
export function addCalibrationPoint(point: Point) {
  const current = calibrating.value;
  const area = status.workArea;
  if (!current || !area) return;
  // Stretched over the bed: the image's top-left is at the bed's top-left (Y up in mm).
  current.image.push({ x: point.x / area.width, y: (area.height - point.y) / area.height });
  if (current.image.length < CALIBRATION_POINTS.length) return;
  call("set_camera_calibration", { calibration: { image: current.image, bed: current.bed } });
  calibrating.value = null;
}

/**
 * The perspective (homography) that takes each `from` point to the `to` point in the
 * same place in the list (4 each): the 8 numbers h of
 *   x' = (h0 x + h1 y + h2) / (h6 x + h7 y + 1),  y' = (h3 x + h4 y + h5) / (h6 x + h7 y + 1).
 * Each pair of points gives 2 linear equations; the 8 are solved by Gaussian elimination.
 * Null when 3 of the points are on one line.
 */
function homography(from: Point[], to: Point[]): number[] | null {
  const rows: number[][] = [];
  from.forEach(({ x, y }, i) => {
    const { x: u, y: v } = to[i];
    rows.push([x, y, 1, 0, 0, 0, -x * u, -y * u, u]);
    rows.push([0, 0, 0, x, y, 1, -x * v, -y * v, v]);
  });
  for (let col = 0; col < 8; col++) {
    // The row with the largest value in this column goes on top (keeps the result accurate).
    let pivot = col;
    for (let row = col + 1; row < 8; row++) {
      if (Math.abs(rows[row][col]) > Math.abs(rows[pivot][col])) pivot = row;
    }
    if (Math.abs(rows[pivot][col]) < 1e-12) return null;
    [rows[col], rows[pivot]] = [rows[pivot], rows[col]];
    for (let row = 0; row < 8; row++) {
      if (row === col) continue;
      const factor = rows[row][col] / rows[col][col];
      for (let k = col; k < 9; k++) rows[row][k] -= factor * rows[col][k];
    }
  }
  return rows.map((row, i) => row[8] / row[i]);
}

/** How far each quarter's cut-out reaches past its edges, in mm, so no thin gap shows between quarters. */
const SEAM_OVERLAP = 1;

/** One copy of the video: its CSS transform and the part of it shown (CSS clip-path). */
export interface CameraPiece {
  transform: string;
  clip: string;
}

/**
 * The CSS transform of a video (sized to the bed, top-left at the bed's top-left, in mm
 * with Y down) that puts the image points `from` on the bed points `to` (4 each).
 */
function perspective(from: Point[], to: Point[]): string {
  const h = homography(from, to);
  if (!h) return "none";
  // The 3×3 homography as a 4×4 matrix (z unused), listed column by column as CSS wants.
  return `matrix3d(${[h[0], h[3], 0, h[6], h[1], h[4], 0, h[7], 0, 0, 1, 0, h[2], h[5], 0, 1].join(",")})`;
}

/** The polygon as a CSS clip-path, pushed `SEAM_OVERLAP` away from its center. */
function clipPath(points: Point[]): string {
  const cx = points.reduce((sum, p) => sum + p.x, 0) / points.length;
  const cy = points.reduce((sum, p) => sum + p.y, 0) / points.length;
  const corners = points.map((p) => {
    const d = Math.hypot(p.x - cx, p.y - cy) || 1;
    return `${p.x + ((p.x - cx) / d) * SEAM_OVERLAP}px ${p.y + ((p.y - cy) / d) * SEAM_OVERLAP}px`;
  });
  return `polygon(${corners.join(",")})`;
}

/**
 * The copies of the video to show, bottom first. Not calibrated or while calibrating:
 * one, stretched over the bed. Calibrated: the whole image in the perspective of the
 * square's 4 corners (for outside the square), then each quarter in its own perspective,
 * cut out to that quarter.
 */
export const cameraPieces = computed<CameraPiece[]>(() => {
  const calibration = status.cameraCalibration;
  const area = status.workArea;
  if (!calibration || !area || calibrating.value) return [{ transform: "none", clip: "none" }];
  // In the video's own space (mm, Y down) and on the bed in SVG space (Y down).
  const image = calibration.image.map((p) => ({ x: p.x * area.width, y: p.y * area.height }));
  const bed = calibration.bed.map((p) => ({ x: p.x, y: area.height - p.y }));
  const corners = [0, 1, 2, 3];
  const pick = (points: Point[], indexes: number[]) => indexes.map((i) => points[i]);
  const pieces = [{ transform: perspective(pick(image, corners), pick(bed, corners)), clip: "none" }];
  // Older calibrations have only the 4 corners.
  if (image.length < CALIBRATION_POINTS.length) return pieces;
  for (const quarter of QUARTERS) {
    pieces.push({
      transform: perspective(pick(image, quarter), pick(bed, quarter)),
      clip: clipPath(pick(image, quarter)),
    });
  }
  return pieces;
});
