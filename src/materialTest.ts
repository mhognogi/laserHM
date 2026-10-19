// Material Test (Tools menu), as in LightBurn: a grid of cells, power per column, speed per row.
import { reactive, ref } from "vue";
import type { Point } from "./editor";

/** Same fields as the `MaterialTest` struct in src-tauri/src/material_test.rs. */
export interface MaterialTest {
  mode: "fill" | "line";
  /** Bottom-left corner of the whole test (labels included), in mm. */
  x: number;
  y: number;
  powerCount: number;
  powerMin: number;
  powerMax: number;
  speedCount: number;
  speedMin: number;
  speedMax: number;
  cellWidth: number;
  cellHeight: number;
  spacing: number;
  /** Fill only: distance between the engraved lines, in mm. */
  interval: number;
  passes: number;
  labels: boolean;
  labelSize: number;
  labelPower: number;
  labelSpeed: number;
}

/** Same fields as the `Part` struct in material_test.rs: a cell or a label. */
export interface Part {
  contours: Point[][];
  powerPercent: number;
  feed: number;
  fill: boolean;
  label: boolean;
}

/** The dialog's values; kept while the app runs, so reopening it shows the last ones. */
export const materialTest = reactive<MaterialTest>({
  mode: "fill",
  x: 10,
  y: 10,
  powerCount: 10,
  powerMin: 10,
  powerMax: 100,
  speedCount: 5,
  speedMin: 1000,
  speedMax: 5000,
  cellWidth: 5,
  cellHeight: 5,
  spacing: 2,
  interval: 0.1,
  passes: 1,
  labels: true,
  labelSize: 2.5,
  labelPower: 30,
  labelSpeed: 1500,
});

/** The Material Test dialog is open. */
export const materialTestOpen = ref(false);
