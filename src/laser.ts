// Laser state and commands to Rust, shared by all components.
import { reactive, ref, shallowRef } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CutSettings, Point, Shape, ShapeType } from "./editor";
import type { rasterData } from "./images";

/** Same shape as the `WorkArea` struct in src-tauri/src/state.rs (mm). */
export interface WorkArea {
  width: number;
  height: number;
}

/** Same shape as the `Status` struct in src-tauri/src/state.rs. */
export interface Status {
  connected: boolean;
  busy: boolean;
  /** The running task, or null when idle. */
  task: "cut" | "simulate" | "home" | "move" | "fire" | null;
  /** True while the running job is paused. */
  paused: boolean;
  message: string;
  /** True when `message` is an error. */
  error: boolean;
  /** The work area to display: set by the user, else the connected laser's. */
  workArea: WorkArea | null;
  /** The work area last read from a laser, even if it is not connected now. */
  laserWorkArea: WorkArea | null;
  /** Ask for the pre-start checks before cutting (saved between sessions). */
  checkBeforeStart: boolean;
  /** Places the camera image on the bed (saved between sessions); null until calibrated. */
  cameraCalibration: CameraCalibration | null;
  /** The laser panel's last power, speed and passes (saved between sessions); null until first changed. */
  job: JobSettings | null;
  /** The safety warning was accepted; null until Rust first answers (so it does not flash). */
  safetyAccepted: boolean | null;
}

/** Same shape as the `CameraCalibration` struct in src-tauri/src/settings.rs. */
export interface CameraCalibration {
  /** Points in the camera image, as fractions of its width and height (0 to 1, from the top-left):
   *  9 (see CALIBRATION_POINTS in camera.ts) or, from older versions, the 4 corners. */
  image: Point[];
  /** Where they are on the bed, in mm (machine coordinates, Y up), in the same order. */
  bed: Point[];
}

// `reactive`: Vue tracks this object and automatically updates every
// component that displays it whenever one of its fields changes.
export const status = reactive<Status>({
  connected: false,
  busy: false,
  task: null,
  paused: false,
  message: "Starting…",
  error: false,
  workArea: null,
  laserWorkArea: null,
  checkBeforeStart: false,
  cameraCalibration: null,
  job: null,
  safetyAccepted: null,
});

// Rust announces every state change; on startup we ask for the current state.
listen<Status>("laser-status", (event) => Object.assign(status, event.payload));
invoke<Status>("get_status").then((current) => Object.assign(status, current));

/** Same fields as the `CutProgress` struct in src-tauri/src/job.rs. */
export interface CutProgress {
  powerPercent: number;
  feed: number;
  /** The current pass (from 1) and the shape's number of passes. */
  pass: number;
  passes: number;
}

/** What the laser is cutting with right now; null when no cut runs. */
export const cutProgress = ref<CutProgress | null>(null);
listen<CutProgress | null>("cut-progress", (event) => (cutProgress.value = event.payload));

/** Same fields as the `JobProgress` struct in src-tauri/src/job.rs. */
export interface JobProgress {
  /** 0 to 1, by the estimated time. */
  fraction: number;
  /** Estimated duration of the whole job, in seconds. */
  totalSeconds: number;
}

/** How far the last cut or simulation is (it stays full when done); null before the first. */
export const jobProgress = ref<JobProgress | null>(null);
listen<JobProgress>("job-progress", (event) => {
  // A new job (0 %) starts with no path cut yet, and the head's last place is forgotten.
  if (event.payload.fraction === 0) {
    cutTrail.value = [];
    headPosition.value = null;
  }
  jobProgress.value = event.payload;
});

/**
 * The path cut so far by the last job, laser on, as [x1, y1, x2, y2] pieces in mm: the canvas
 * shows it in green. Replaced (not changed) on each addition, so the canvas redraws.
 */
export const cutTrail = shallowRef<number[][]>([]);
listen<number[][]>("cut-trail", (event) => (cutTrail.value = [...cutTrail.value, ...event.payload]));
// Each new pass shows its own progress: the shapes go back to their color first.
listen("cut-trail-reset", () => (cutTrail.value = []));

/** Where the laser head is while a job runs, in mm ([x, y]); null before the first report. */
export const headPosition = ref<[number, number] | null>(null);
listen<[number, number]>("head-position", (event) => (headPosition.value = event.payload));

/** Power of the Fire button (laser on without moving), in % of the maximum; set in the top bar. */
export const fireSettings = reactive({ powerPercent: 10 });

/** Power, speed and passes for the job, chosen in the laser panel. */
export interface JobSettings {
  /** Percent of the laser's maximum power. */
  powerPercent: number;
  /** mm/min */
  feed: number;
  passes: number;
}

/** Calls a Rust command and shows the error, if any. */
type Command =
  | "start"
  | "simulate"
  | "home"
  | "move_to"
  | "fire"
  | "fire_off"
  | "pause"
  | "resume"
  | "stop"
  | "set_check_before_start"
  | "set_job_settings"
  | "accept_safety"
  | "set_camera_calibration";

export async function call(command: Command, args?: object) {
  try {
    await invoke(command, args as Record<string, unknown>);
  } catch (error) {
    status.message = String(error);
    status.error = true;
  }
}

/** A shape in machine coordinates (mm), as the Rust `Shape` struct expects it. */
export interface ShapeData {
  type: ShapeType;
  cx: number;
  cy: number;
  rx: number;
  ry: number;
  /** Text only: the letters' outlines in the box from -1 to 1. */
  contours?: Point[][];
  /** Rotation around the center, in degrees counterclockwise. */
  angle?: number;
  /** Path only: true when it ends where it started. */
  closed?: boolean;
  /** Its own cut settings, used instead of the panel's. */
  cut?: CutSettings;
  /** Image only, for a cut: its dots and how to burn them (see images.ts). */
  raster?: ReturnType<typeof rasterData>;
}

/** A shape as Rust expects it: only the fields Rust uses. */
export function shapeData({ type, cx, cy, rx, ry, contours, angle, closed, cut }: Shape): ShapeData {
  return { type, cx, cy, rx, ry, contours, angle, closed, cut };
}

/** Cuts the shapes at their positions on the work area. */
export function start(settings: JobSettings, shapes: ShapeData[]) {
  return call("start", { ...settings, shapes });
}

/** Follows the shapes once with the laser off. */
export function simulate(feed: number, shapes: ShapeData[]) {
  return call("simulate", { feed, shapes });
}

export function setCheckBeforeStart(enabled: boolean) {
  return call("set_check_before_start", { enabled });
}

/** Saves the work area set by the user. Throws on invalid values. */
export async function setWorkArea(width: number, height: number) {
  await invoke("set_work_area", { width, height });
}
