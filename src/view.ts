// The work area view (zoom and pan), shared by the work area and the zoom buttons.
import { computed, ref } from "vue";
import { status } from "./laser";
import type { Box } from "./editor";

/**
 * Room around the bed, in screen pixels: only what the axis labels need.
 * Left and bottom: the same, one label's height (the Y labels are turned upright);
 * top and right: half of a label ("400") centered on the top or right edge.
 */
export const MARGIN_PX = { left: 18, right: 10, top: 10, bottom: 18 };

/** Zoom limits, relative to "fit": from half the fit size to 50 times closer. */
const MIN_ZOOM = 0.5;
const MAX_ZOOM = 50;
/** How much one click on a zoom button zooms in or out. */
const BUTTON_ZOOM_FACTOR = 1.25;
/** Space left around a shape when zooming to it, in screen pixels. */
const SELECTION_PADDING_PX = 24;

/** Size of the work area <svg> on screen, in pixels; set by the work area component. */
export const svgSize = ref({ width: 0, height: 0 });

/**
 * The user's view: `zoom` relative to "fit" (1 = whole bed visible), and
 * `panX`/`panY`, how far the bed was moved from its fitted position, in screen pixels.
 */
export const view = ref({ zoom: 1, panX: 0, panY: 0 });

/** Screen pixels per mm when the whole bed fits in the space left after the label margins. */
export const fitScale = computed(() => {
  const area = status.workArea;
  const { width, height } = svgSize.value;
  if (!area || width === 0) return 1;
  const availableWidth = width - MARGIN_PX.left - MARGIN_PX.right;
  const availableHeight = height - MARGIN_PX.top - MARGIN_PX.bottom;
  return Math.max(Math.min(availableWidth / area.width, availableHeight / area.height), 0.01);
});

/** Screen pixels per mm, including the zoom. */
export const scale = computed(() => fitScale.value * view.value.zoom);

/** Where the bed's top-left corner would be on screen (px, inside the <svg>) without panning. */
const fitOrigin = computed(() => {
  const area = status.workArea;
  const { width, height } = svgSize.value;
  if (!area) return { x: 0, y: 0 };
  const s = fitScale.value;
  return {
    x: MARGIN_PX.left + (width - MARGIN_PX.left - MARGIN_PX.right - area.width * s) / 2,
    y: MARGIN_PX.top + (height - MARGIN_PX.top - MARGIN_PX.bottom - area.height * s) / 2,
  };
});

/** Where the bed's top-left corner is on screen (px, inside the <svg>). */
export const origin = computed(() => ({
  x: fitOrigin.value.x + view.value.panX,
  y: fitOrigin.value.y + view.value.panY,
}));

function clampZoom(zoom: number) {
  return Math.min(Math.max(zoom, MIN_ZOOM), MAX_ZOOM);
}

/** Zooms by `factor`, keeping the screen point (px, inside the <svg>) in place. */
export function zoomAt(px: number, py: number, factor: number) {
  // The SVG point (mm) at that screen position before zooming...
  const x = (px - origin.value.x) / scale.value;
  const y = (py - origin.value.y) / scale.value;
  const zoom = clampZoom(view.value.zoom * factor);
  const newScale = fitScale.value * zoom;
  // ...must still be there after zooming: solve the pan for that.
  view.value = {
    zoom,
    panX: px - x * newScale - fitOrigin.value.x,
    panY: py - y * newScale - fitOrigin.value.y,
  };
}

/** Zoom buttons: zoom around the center of the work area. */
export function zoomIn() {
  zoomAt(svgSize.value.width / 2, svgSize.value.height / 2, BUTTON_ZOOM_FACTOR);
}

export function zoomOut() {
  zoomAt(svgSize.value.width / 2, svgSize.value.height / 2, 1 / BUTTON_ZOOM_FACTOR);
}

/** The whole bed visible. */
export function zoomToFit() {
  view.value = { zoom: 1, panX: 0, panY: 0 };
}

/** Zooms so `box` (machine coordinates, mm) fills the view, with a little space around it. */
export function zoomToBox(box: Box) {
  const area = status.workArea;
  if (!area) return;
  const { width, height } = svgSize.value;
  const availableWidth = width - MARGIN_PX.left - MARGIN_PX.right - 2 * SELECTION_PADDING_PX;
  const availableHeight = height - MARGIN_PX.top - MARGIN_PX.bottom - 2 * SELECTION_PADDING_PX;
  const boxWidth = Math.max(box.right - box.left, 0.1);
  const boxHeight = Math.max(box.top - box.bottom, 0.1);
  const zoom = clampZoom(
    Math.min(availableWidth / boxWidth, availableHeight / boxHeight) / fitScale.value,
  );
  const newScale = fitScale.value * zoom;
  // The box's top-left corner in SVG coordinates (SVG's Y grows downwards).
  const svgLeft = box.left;
  const svgTop = area.height - box.top;
  // Center the box in the available space.
  const originX = MARGIN_PX.left + SELECTION_PADDING_PX + (availableWidth - boxWidth * newScale) / 2 - svgLeft * newScale;
  const originY = MARGIN_PX.top + SELECTION_PADDING_PX + (availableHeight - boxHeight * newScale) / 2 - svgTop * newScale;
  view.value = { zoom, panX: originX - fitOrigin.value.x, panY: originY - fitOrigin.value.y };
}
