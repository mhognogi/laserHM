//! Generates the G-code for a job.

use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use crate::state::WorkArea;

/// Highest number of passes accepted.
const MAX_PASSES: u32 = 100;
/// The straight segments approximating an ellipse stray from it by at most this much, in mm.
const MAX_DEVIATION_MM: f64 = 0.05;
/// Fewest segments for an ellipse, so even tiny ones stay round.
const MIN_SEGMENTS: usize = 16;

/// Power, speed and passes chosen in the UI.
pub struct JobSettings {
    /// Percent of the laser's maximum power (GRBL setting `$30`).
    pub power_percent: f64,
    pub feed_mm_per_min: f64,
    /// How many times the shape is cut, always over the same path.
    pub passes: u32,
}

impl JobSettings {
    pub fn new(power_percent: f64, feed_mm_per_min: f64, passes: u32) -> Result<JobSettings, String> {
        if !(0.0..=100.0).contains(&power_percent) {
            return Err("Power must be between 0 and 100%".into());
        }
        if !(feed_mm_per_min > 0.0) {
            return Err("Speed must be greater than 0".into());
        }
        if !(1..=MAX_PASSES).contains(&passes) {
            return Err(format!("Passes must be between 1 and {MAX_PASSES}"));
        }
        Ok(JobSettings { power_percent, feed_mm_per_min, passes })
    }
}

/// A shape's own cut settings (e.g. a Material Test cell), used instead of the panel's.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CutSettings {
    pub power_percent: f64,
    pub feed: f64,
    pub passes: u32,
    /// True: engraved full, line by line; false: only the outline.
    pub fill: bool,
    /// Fill only: distance between the engraved lines, in mm.
    pub interval: f64,
}

/// Number of sides of the polygon tool (fixed for now; the UI uses the same).
const POLYGON_SIDES: usize = 6;

/// A point; in JSON { "x": …, "y": … }.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// The kinds of shapes; in JSON "ellipse", "rectangle", "polygon", "text" or "path".
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShapeKind {
    Ellipse,
    Rectangle,
    Polygon,
    Text,
    /// Drawn with Draw Lines: straight and curved segments, open or closed.
    Path,
    /// An imported picture, engraved line by line (`raster`); its outline is its frame.
    Image,
}

/// An image's dots, as the UI computed them (see images.ts), and how to burn them.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Raster {
    pub columns: usize,
    pub rows: usize,
    /// Row by row from the top: 0 = no burn … 255 = full burn.
    pub values: Vec<u8>,
    /// mm/min
    pub speed: f64,
    /// % of the maximum for a full burn (255) and for 0.
    pub power_max: f64,
    pub power_min: f64,
}

/// How far the head runs past an image's line, laser off, so it engraves at full speed.
const OVERSCAN_MM: f64 = 2.5;

/// A shape in machine coordinates (mm): its kind, the center and half the width
/// and height of the box it fills, and how much that box is turned around its center.
/// Received from the UI as JSON, e.g. { "type": "ellipse", "cx": 120, "cy": 300, "rx": 70, "ry": 25, "angle": 30 }.
#[derive(Clone, Deserialize)]
pub struct Shape {
    /// `type` is a reserved word in Rust, so the field has another name here.
    #[serde(rename = "type")]
    pub kind: ShapeKind,
    pub cx: f64,
    pub cy: f64,
    pub rx: f64,
    pub ry: f64,
    /// Text and path only: the outlines in the box from -1 to 1 (for text see text.rs;
    /// a path has one, with its curves already split into short segments).
    /// `default`: the other shapes do not send it.
    #[serde(default)]
    pub contours: Vec<Vec<Point>>,
    /// Path only: true when it ends where it started (the laser goes back to the start).
    #[serde(default)]
    pub closed: bool,
    /// Rotation in degrees, counterclockwise (Y points up); 0 when not sent.
    #[serde(default)]
    pub angle: f64,
    /// Its own power, speed, passes and fill; `None`: the panel's settings.
    #[serde(default)]
    pub cut: Option<CutSettings>,
    /// Image only, and only for a cut (not a simulation): the dots to engrave.
    #[serde(default)]
    pub raster: Option<Raster>,
}

impl Shape {
    /// A point of the unit box (-1…1 on both axes) placed on the shape: scaled to its
    /// box, turned by its angle, then moved to its center.
    fn place(&self, ux: f64, uy: f64) -> (f64, f64) {
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let (x, y) = (self.rx * ux, self.ry * uy);
        (self.cx + x * cos - y * sin, self.cy + x * sin + y * cos)
    }

    /// Refuses shapes that are not entirely inside the machine's travel. A turned
    /// shape no longer fills an upright box, so every point of its paths is checked.
    pub fn check_inside(&self, area: WorkArea) -> Result<(), String> {
        let fits = |&(x, y): &(f64, f64)| x >= 0.0 && x <= area.width && y >= 0.0 && y <= area.height;
        // A straight horizontal or vertical line has no height or no width, but not both.
        let has_size = if self.kind == ShapeKind::Path {
            self.rx > 0.0 || self.ry > 0.0
        } else {
            self.rx > 0.0 && self.ry > 0.0
        };
        let inside = has_size && self.paths().iter().flatten().all(fits);
        if inside {
            Ok(())
        } else {
            Err(format!(
                "The shape must be entirely inside the work area (0–{} × 0–{} mm)",
                area.width, area.height
            ))
        }
    }

    /// The paths the laser follows, closed (ending where they start) except an open path.
    /// Text has one path per contour (letters and the holes in them); the other shapes one.
    pub(crate) fn paths(&self) -> Vec<Vec<(f64, f64)>> {
        if matches!(self.kind, ShapeKind::Text | ShapeKind::Path) {
            let close = self.kind == ShapeKind::Text || self.closed;
            return self
                .contours
                .iter()
                .filter(|contour| !contour.is_empty())
                .map(|contour| {
                    let mut path: Vec<(f64, f64)> = contour.iter().map(|p| self.place(p.x, p.y)).collect();
                    if close {
                        path.push(path[0]);
                    }
                    path
                })
                .collect();
        }
        vec![self.points()]
    }

    /// The outline of an ellipse, rectangle or polygon as a closed path.
    fn points(&self) -> Vec<(f64, f64)> {
        let at = |ux: f64, uy: f64| self.place(ux, uy);
        let mut points: Vec<(f64, f64)> = match self.kind {
            ShapeKind::Ellipse => {
                // A segment spanning angle `step` on a circle of radius r strays from it by
                // r × (1 − cos(step/2)); the largest radius gives the strictest step.
                let radius = self.rx.max(self.ry);
                let step = 2.0 * (1.0 - (MAX_DEVIATION_MM / radius).min(1.0)).acos();
                let segments = ((TAU / step).ceil() as usize).max(MIN_SEGMENTS);
                // Starts at the rightmost point.
                (0..segments)
                    .map(|i| {
                        let angle = TAU * i as f64 / segments as f64;
                        at(angle.cos(), angle.sin())
                    })
                    .collect()
            }
            // Starts at the bottom-right corner.
            // An image: its frame.
            ShapeKind::Rectangle | ShapeKind::Image => {
                vec![at(1.0, -1.0), at(1.0, 1.0), at(-1.0, 1.0), at(-1.0, -1.0)]
            }
            ShapeKind::Polygon => unit_polygon().into_iter().map(|(x, y)| at(x, y)).collect(),
            // Text and paths are handled by `paths`.
            ShapeKind::Text | ShapeKind::Path => vec![at(0.0, 0.0)],
        };
        // Close the path.
        points.push(points[0]);
        points
    }
}

/// The polygon's corners for a box from -1 to 1 on both axes: a regular polygon
/// with one corner at the top, stretched to fill the box exactly. The UI draws the same.
fn unit_polygon() -> Vec<(f64, f64)> {
    let corners: Vec<(f64, f64)> = (0..POLYGON_SIDES)
        .map(|i| {
            let angle = std::f64::consts::FRAC_PI_2 + TAU * i as f64 / POLYGON_SIDES as f64;
            (angle.cos(), angle.sin())
        })
        .collect();
    let min_x = corners.iter().map(|c| c.0).fold(f64::INFINITY, f64::min);
    let max_x = corners.iter().map(|c| c.0).fold(f64::NEG_INFINITY, f64::max);
    let min_y = corners.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
    let max_y = corners.iter().map(|c| c.1).fold(f64::NEG_INFINITY, f64::max);
    corners
        .iter()
        .map(|(x, y)| ((x - min_x) / (max_x - min_x) * 2.0 - 1.0, (y - min_y) / (max_y - min_y) * 2.0 - 1.0))
        .collect()
}

/// Refuses an empty job and shapes that are not entirely inside the machine's travel.
pub fn check_shapes(shapes: &[Shape], area: WorkArea) -> Result<(), String> {
    if shapes.is_empty() {
        return Err("No object selected".into());
    }
    for shape in shapes {
        shape.check_inside(area)?;
        if let Some(cut) = &shape.cut {
            JobSettings::new(cut.power_percent, cut.feed, cut.passes)?;
            if cut.fill && !(cut.interval > 0.0) {
                return Err("Interval must be greater than 0".into());
            }
        }
    }
    Ok(())
}

/// What the laser is cutting with, announced to the UI while the job runs.
#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CutProgress {
    pub power_percent: f64,
    pub feed: f64,
    /// The current pass (from 1) and the shape's number of passes.
    pub pass: u32,
    pub passes: u32,
}

/// Only the G-code of `cut_job` (the tests check it), on a large work area.
#[cfg(test)]
pub fn cut_gcode(shapes: &[Shape], settings: &JobSettings, max_power: f64) -> Vec<String> {
    cut_job(shapes, settings, max_power, WorkArea { width: 1000.0, height: 1000.0 }).0
}

/// Refuses images whose dots are missing or wrong, and invalid image settings.
pub fn check_rasters(shapes: &[Shape]) -> Result<(), String> {
    for shape in shapes.iter().filter(|shape| shape.kind == ShapeKind::Image) {
        let raster = shape.raster.as_ref().ok_or("The image has no dots to engrave")?;
        if raster.columns == 0 || raster.rows == 0 || raster.values.len() != raster.columns * raster.rows {
            return Err("The image's dots do not match its size".into());
        }
        JobSettings::new(raster.power_max, raster.speed, 1)?;
        JobSettings::new(raster.power_min, raster.speed, 1)?;
    }
    Ok(())
}

/// The cut's G-code, and where its settings change: (index of the line, the new settings).
/// `max_power` is the `$30` value read from the laser. A shape with its own settings
/// (`cut`, an image's `raster`) uses them; the others use the panel's `settings`.
/// `area` keeps the images' overscan inside the machine's travel.
pub fn cut_job(
    shapes: &[Shape],
    settings: &JobSettings,
    max_power: f64,
    area: WorkArea,
) -> (Vec<String>, Vec<(usize, CutProgress)>) {
    // Each shape: the paths the laser follows, its power (%), speed and passes,
    // or (an image) the shape whose dots are engraved, once.
    let jobs: Vec<(Vec<Vec<(f64, f64)>>, f64, f64, u32, Option<&Shape>)> = shapes
        .iter()
        .map(|shape| {
            if let (ShapeKind::Image, Some(raster)) = (shape.kind, &shape.raster) {
                return (Vec::new(), raster.power_max, raster.speed, 1, Some(shape));
            }
            let (power_percent, feed, passes) = match &shape.cut {
                Some(cut) => (cut.power_percent, cut.feed, cut.passes),
                None => (settings.power_percent, settings.feed_mm_per_min, settings.passes),
            };
            let paths = match &shape.cut {
                // Fill: each engraved line is a path of two points.
                Some(cut) if cut.fill => {
                    hatch(&shape.paths(), cut.interval).into_iter().map(|(a, b)| vec![a, b]).collect()
                }
                _ => shape.paths(),
            };
            (paths, power_percent, feed, passes, None)
        })
        .collect();

    // M4 = dynamic power, as in LightBurn: GRBL lowers the power when
    // the machine slows down, so the burn stays even.
    let mut gcode = vec!["G21 G90".to_string(), "M4 S0".to_string()];
    let mut changes: Vec<(usize, CutProgress)> = Vec::new();
    // Each pass follows every shape that still has passes left, as LightBurn repeats a layer.
    let most = jobs.iter().map(|job| job.3).max().unwrap_or(1);
    for pass in 0..most {
        for (paths, power_percent, feed, passes, image) in &jobs {
            if pass >= *passes {
                continue;
            }
            let progress = CutProgress { power_percent: *power_percent, feed: *feed, pass: pass + 1, passes: *passes };
            if changes.last().map(|(_, last)| last) != Some(&progress) {
                changes.push((gcode.len(), progress));
            }
            match image {
                Some(shape) => push_raster(&mut gcode, shape, max_power, area),
                None => push_paths(&mut gcode, paths, (max_power * power_percent / 100.0).round(), *feed),
            }
        }
    }
    gcode.extend(job_end());
    (gcode, changes)
}

/// G-code that engraves an image line by line, like a printer: horizontal machine lines one
/// dot apart, every other one backwards. Neighbouring dots with the same power become one
/// move; blank ends of a line are skipped. A turned image is sampled through its rotation.
fn push_raster(gcode: &mut Vec<String>, shape: &Shape, max_power: f64, area: WorkArea) {
    let Some(raster) = &shape.raster else { return };
    let step_x = 2.0 * shape.rx / raster.columns as f64;
    let step_y = 2.0 * shape.ry / raster.rows as f64;
    let (sin, cos) = shape.angle.to_radians().sin_cos();

    // The S value at a machine point: the dot under it, or 0 outside the image.
    let level = |x: f64, y: f64| -> f64 {
        // Back into the image's own (unturned) frame.
        let (dx, dy) = (x - shape.cx, y - shape.cy);
        let (u, v) = (dx * cos + dy * sin, -dx * sin + dy * cos);
        let column = ((u + shape.rx) / step_x).floor();
        let row = ((shape.ry - v) / step_y).floor();
        if column < 0.0 || row < 0.0 || column >= raster.columns as f64 || row >= raster.rows as f64 {
            return 0.0;
        }
        let value = f64::from(raster.values[row as usize * raster.columns + column as usize]);
        let percent = raster.power_min + (raster.power_max - raster.power_min) * value / 255.0;
        (max_power * percent / 100.0).round()
    };

    // The machine area the (turned) image covers.
    let corners = shape.points();
    let min_x = corners.iter().map(|c| c.0).fold(f64::INFINITY, f64::min);
    let max_x = corners.iter().map(|c| c.0).fold(f64::NEG_INFINITY, f64::max);
    let min_y = corners.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
    let max_y = corners.iter().map(|c| c.1).fold(f64::NEG_INFINITY, f64::max);
    // A little slack, so rounding does not add an empty dot or line.
    let dots = ((max_x - min_x) / step_x - 1e-6).ceil() as usize;
    let lines = ((max_y - min_y) / step_y - 1e-6).ceil() as usize;
    // The left edge of dot k on a line.
    let edge = |k: usize| min_x + k as f64 * step_x;
    let inside = |x: f64| x.clamp(0.0, area.width);

    let mut forward = true;
    // From the top line down.
    for line in 0..lines {
        let y = max_y - (line as f64 + 0.5) * step_y;
        let levels: Vec<f64> = (0..dots).map(|k| level(edge(k) + step_x / 2.0, y)).collect();
        let (Some(first), Some(last)) =
            (levels.iter().position(|&s| s > 0.0), levels.iter().rposition(|&s| s > 0.0))
        else {
            continue;
        };
        // Runs of equal power: (first dot, last dot, S).
        let mut runs: Vec<(usize, usize, f64)> = Vec::new();
        for (k, &power) in levels.iter().enumerate().take(last + 1).skip(first) {
            match runs.last_mut() {
                Some(run) if run.2 == power => run.1 = k,
                _ => runs.push((k, k, power)),
            }
        }

        let (start, end) = if forward { (edge(first), edge(last + 1)) } else { (edge(last + 1), edge(first)) };
        let direction = if forward { 1.0 } else { -1.0 };
        // Overscan: start a little before the line, laser off, so the head is at speed.
        gcode.push(format!("G0 X{:.3} Y{y:.3}", inside(start - direction * OVERSCAN_MM)));
        gcode.push(format!("G1 X{start:.3} S0 F{}", raster.speed));
        if forward {
            for &(_, last_dot, power) in &runs {
                gcode.push(format!("G1 X{:.3} S{power}", edge(last_dot + 1)));
            }
        } else {
            for &(first_dot, _, power) in runs.iter().rev() {
                gcode.push(format!("G1 X{:.3} S{power}", edge(first_dot)));
            }
        }
        gcode.push(format!("G1 X{:.3} S0", inside(end + direction * OVERSCAN_MM)));
        forward = !forward;
    }
}

/// After each line: when it ends, in seconds from the start (every move at its speed,
/// G0 at `rapid` mm/min, acceleration ignored, so a little optimistic, mostly for images),
/// and where the head then is.
pub fn trace(gcode: &[String], rapid: f64) -> Vec<(f64, (f64, f64))> {
    let (mut x, mut y, mut feed, mut time) = (0.0f64, 0.0f64, 1000.0, 0.0);
    gcode
        .iter()
        .map(|line| {
            let mut words = line.split_whitespace();
            let code = words.next().unwrap_or("");
            if code == "G0" || code == "G1" {
                let (mut to_x, mut to_y) = (x, y);
                // Words like "X12.5", "Y3", "F1000".
                for word in words {
                    let (letter, value) = word.split_at(1);
                    let Ok(value) = value.parse::<f64>() else { continue };
                    match letter {
                        "X" => to_x = value,
                        "Y" => to_y = value,
                        "F" => feed = value,
                        _ => {}
                    }
                }
                let speed = if code == "G0" { rapid } else { feed };
                if speed > 0.0 {
                    time += (to_x - x).hypot(to_y - y) / speed * 60.0;
                }
                (x, y) = (to_x, to_y);
            }
            (time, (x, y))
        })
        .collect()
}

/// How far the job is, announced to the UI while it runs.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgress {
    /// 0 to 1, by the estimated time of the lines sent.
    pub fraction: f64,
    /// Estimated duration of the whole job, in seconds.
    pub total_seconds: f64,
}

/// G-code that follows the shapes once with the laser off, to check their positions.
pub fn simulation_gcode(shapes: &[Shape], feed_mm_per_min: f64) -> Vec<String> {
    // Without M4 the laser stays off, and S0 makes sure of it.
    let mut gcode = vec!["G21 G90".to_string()];
    let paths: Vec<_> = shapes.iter().flat_map(Shape::paths).collect();
    push_paths(&mut gcode, &paths, 0.0, feed_mm_per_min);
    gcode.extend(job_end());
    gcode
}

/// G-code that moves the head to (x, y) with the laser off. Refuses points outside
/// the machine's travel.
pub fn move_gcode(x: f64, y: f64, area: WorkArea) -> Result<Vec<String>, String> {
    if !(x >= 0.0 && x <= area.width && y >= 0.0 && y <= area.height) {
        return Err(format!(
            "The point must be inside the work area (0–{} × 0–{} mm)",
            area.width, area.height
        ));
    }
    Ok(vec![
        // Millimeters, absolute coordinates; laser off.
        "G21 G90".to_string(),
        "M5 S0".to_string(),
        format!("G0 X{x:.3} Y{y:.3}"),
        // Answers only when the head has arrived.
        "G4 P0".to_string(),
    ])
}

/// G-code that turns the laser on at S `power` without moving (to see where it points).
/// M3 = constant power: in laser mode ($32=1) GRBL keeps the laser on in G1 mode even
/// while standing still; G1 without coordinates only selects that mode (no motion).
pub fn fire_gcode(power: f64) -> Vec<String> {
    vec![format!("M3 S{power}"), "G1 F1000".to_string()]
}

/// G-code that follows the paths with power S `power` (0: laser off) at `feed` mm/min.
fn push_paths(gcode: &mut Vec<String>, paths: &[Vec<(f64, f64)>], power: f64, feed: f64) {
    for points in paths {
        let (start_x, start_y) = points[0];
        // Rapid move to the path's start; in laser mode GRBL keeps the laser off during G0.
        gcode.push(format!("G0 X{start_x:.3} Y{start_y:.3}"));
        for (index, (x, y)) in points.iter().enumerate().skip(1) {
            if index == 1 {
                // Power and speed are set at the start of each path;
                // GRBL keeps them for the following moves.
                gcode.push(format!("G1 X{x:.3} Y{y:.3} S{power} F{feed}"));
            } else {
                gcode.push(format!("G1 X{x:.3} Y{y:.3}"));
            }
        }
    }
}

/// The end of every job.
fn job_end() -> [String; 2] {
    [
        // Laser off.
        "M5 S0".to_string(),
        // 0 s dwell: GRBL replies "ok" only after the motion has finished,
        // so we know when the job is really done.
        "G4 P0".to_string(),
    ]
}

/// Horizontal lines `interval` apart filling the outlines (even-odd: holes stay empty),
/// every other line reversed so the head does not travel back each time.
pub(crate) fn hatch(contours: &[Vec<(f64, f64)>], interval: f64) -> Vec<((f64, f64), (f64, f64))> {
    let ys = contours.iter().flatten().map(|p| p.1);
    let (min_y, max_y) = ys.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), y| (lo.min(y), hi.max(y)));
    let mut lines = Vec::new();
    let mut reverse = false;
    let mut y = min_y + interval / 2.0;
    while y < max_y {
        // Where the line crosses the outlines' edges.
        let mut xs: Vec<f64> = Vec::new();
        for contour in contours {
            for (i, &(ax, ay)) in contour.iter().enumerate() {
                let (bx, by) = contour[(i + 1) % contour.len()];
                if (ay <= y) != (by <= y) {
                    xs.push(ax + (y - ay) * (bx - ax) / (by - ay));
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        // Inside between the 1st and 2nd crossing, the 3rd and 4th, and so on.
        let mut segments: Vec<_> = xs.chunks_exact(2).map(|pair| ((pair[0], y), (pair[1], y))).collect();
        if reverse {
            segments = segments.into_iter().rev().map(|(a, b)| (b, a)).collect();
        }
        lines.extend(segments);
        reverse = !reverse;
        y += interval;
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: WorkArea = WorkArea { width: 400.0, height: 400.0 };

    fn settings(passes: u32) -> JobSettings {
        JobSettings::new(60.0, 2000.0, passes).unwrap()
    }

    #[test]
    fn rejects_invalid_settings() {
        assert!(JobSettings::new(101.0, 2000.0, 1).is_err());
        assert!(JobSettings::new(50.0, 0.0, 1).is_err());
        assert!(JobSettings::new(50.0, 2000.0, 0).is_err());
    }

    fn shape(kind: ShapeKind, cx: f64, cy: f64, rx: f64, ry: f64) -> Shape {
        Shape { kind, cx, cy, rx, ry, contours: Vec::new(), closed: false, angle: 0.0, cut: None, raster: None }
    }

    #[test]
    fn moves_the_head_only_inside_the_work_area() {
        let gcode = move_gcode(120.0, 35.5, AREA).unwrap();
        assert_eq!(gcode, ["G21 G90", "M5 S0", "G0 X120.000 Y35.500", "G4 P0"]);
        assert!(move_gcode(-1.0, 10.0, AREA).is_err());
        assert!(move_gcode(10.0, 401.0, AREA).is_err());
    }

    #[test]
    fn an_open_path_does_not_go_back_to_its_start() {
        // A horizontal line from (60, 50) to (140, 50): no height, still valid.
        let mut line = shape(ShapeKind::Path, 100.0, 50.0, 40.0, 0.0);
        line.contours = vec![vec![Point { x: -1.0, y: 0.0 }, Point { x: 1.0, y: 0.0 }]];
        assert!(line.check_inside(AREA).is_ok());
        assert_eq!(line.paths(), [vec![(60.0, 50.0), (140.0, 50.0)]]);
        // Closed, it goes back to the start.
        line.closed = true;
        assert_eq!(line.paths()[0].len(), 3);
    }

    #[test]
    fn turned_shapes_turn_around_their_center() {
        let mut rectangle = shape(ShapeKind::Rectangle, 100.0, 50.0, 40.0, 20.0);
        rectangle.angle = 90.0;
        // A quarter turn counterclockwise: the bottom-right corner goes to the top-right.
        let (x, y) = rectangle.points()[0];
        assert!((x - 120.0).abs() < 1e-9 && (y - 90.0).abs() < 1e-9);
    }

    #[test]
    fn a_turned_shape_must_fit_the_work_area() {
        // A 420 × 20 mm bar at the center of the 400 × 400 mm area: too long upright,
        // but it fits turned by 45° (along the diagonal).
        let mut bar = shape(ShapeKind::Rectangle, 200.0, 200.0, 210.0, 10.0);
        assert!(bar.check_inside(AREA).is_err());
        bar.angle = 45.0;
        assert!(bar.check_inside(AREA).is_ok());
        // A 300 mm square fits upright, but its corners stick out turned by 45°.
        let mut square = shape(ShapeKind::Rectangle, 200.0, 200.0, 150.0, 150.0);
        assert!(square.check_inside(AREA).is_ok());
        square.angle = 45.0;
        assert!(square.check_inside(AREA).is_err());
    }

    fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Shape {
        shape(ShapeKind::Ellipse, cx, cy, rx, ry)
    }

    #[test]
    fn rejects_shapes_outside_the_work_area() {
        assert!(ellipse(100.0, 100.0, 50.0, 20.0).check_inside(AREA).is_ok());
        assert!(ellipse(10.0, 100.0, 50.0, 20.0).check_inside(AREA).is_err());
        assert!(shape(ShapeKind::Rectangle, 390.0, 100.0, 20.0, 20.0).check_inside(AREA).is_err());
    }

    #[test]
    fn closed_path_that_stays_close_to_the_ellipse() {
        let ellipse = ellipse(100.0, 50.0, 40.0, 20.0);
        let points = ellipse.points();
        assert!(points.len() > MIN_SEGMENTS);
        // Starts and ends at the rightmost point.
        let (first, last) = (points[0], points[points.len() - 1]);
        assert!((first.0 - 140.0).abs() < 1e-9 && (first.1 - 50.0).abs() < 1e-9);
        assert!((last.0 - first.0).abs() < 1e-9 && (last.1 - first.1).abs() < 1e-9);
    }

    #[test]
    fn rejects_an_empty_job() {
        assert!(check_shapes(&[], AREA).is_err());
    }

    #[test]
    fn rectangle_goes_around_its_four_corners() {
        let rectangle = shape(ShapeKind::Rectangle, 100.0, 50.0, 40.0, 20.0);
        assert_eq!(
            rectangle.points(),
            [(140.0, 30.0), (140.0, 70.0), (60.0, 70.0), (60.0, 30.0), (140.0, 30.0)]
        );
    }

    #[test]
    fn polygon_is_a_closed_hexagon_filling_its_box() {
        let polygon = shape(ShapeKind::Polygon, 100.0, 50.0, 40.0, 20.0);
        let points = polygon.points();
        // 6 corners plus the closing point.
        assert_eq!(points.len(), POLYGON_SIDES + 1);
        assert_eq!(points[0], points[POLYGON_SIDES]);
        // First corner at the top center; the corners touch every side of the box.
        assert!((points[0].0 - 100.0).abs() < 1e-9 && (points[0].1 - 70.0).abs() < 1e-9);
        let xs: Vec<f64> = points.iter().map(|p| p.0).collect();
        let ys: Vec<f64> = points.iter().map(|p| p.1).collect();
        assert!((xs.iter().cloned().fold(f64::INFINITY, f64::min) - 60.0).abs() < 1e-9);
        assert!((xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - 140.0).abs() < 1e-9);
        assert!((ys.iter().cloned().fold(f64::INFINITY, f64::min) - 30.0).abs() < 1e-9);
    }

    #[test]
    fn text_cuts_each_contour_placed_on_its_box() {
        let mut text = shape(ShapeKind::Text, 100.0, 50.0, 40.0, 20.0);
        // Two square "letters", in the unit box.
        let square = |x0: f64, x1: f64| {
            vec![Point { x: x0, y: -1.0 }, Point { x: x1, y: -1.0 }, Point { x: x1, y: 1.0 }, Point { x: x0, y: 1.0 }]
        };
        text.contours = vec![square(-1.0, -0.2), square(0.2, 1.0)];
        let paths = text.paths();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], [(60.0, 30.0), (92.0, 30.0), (92.0, 70.0), (60.0, 70.0), (60.0, 30.0)]);
        // Each contour gets its own G0 (move with the laser off) in the G-code.
        let gcode = cut_gcode(&[text], &settings(1), 1000.0);
        assert_eq!(gcode.iter().filter(|line| line.starts_with("G0")).count(), 2);
    }

    #[test]
    fn reads_the_shape_type_from_json() {
        let json = r#"{ "type": "polygon", "cx": 1, "cy": 2, "rx": 3, "ry": 4 }"#;
        let shape: Shape = serde_json::from_str(json).unwrap();
        assert_eq!(shape.kind, ShapeKind::Polygon);
    }

    #[test]
    fn gcode_moves_to_start_then_cuts_each_pass() {
        let ellipse = ellipse(100.0, 50.0, 40.0, 20.0);
        let segments = ellipse.points().len() - 1;
        let gcode = cut_gcode(&[ellipse], &settings(2), 1000.0);
        assert_eq!(gcode[..4], ["G21 G90", "M4 S0", "G0 X140.000 Y50.000", gcode[3].as_str()]);
        assert!(gcode[3].ends_with("S600 F2000"));
        // Header (2) + per pass: one G0 and one G1 per segment + footer (2).
        assert_eq!(gcode.len(), 2 + 2 * (1 + segments) + 2);
        assert_eq!(gcode[gcode.len() - 2..], ["M5 S0", "G4 P0"]);
    }

    #[test]
    fn each_pass_cuts_every_shape_in_turn() {
        let first = ellipse(100.0, 50.0, 40.0, 20.0);
        let second = ellipse(200.0, 50.0, 10.0, 10.0);
        let gcode = cut_gcode(&[first, second], &settings(2), 1000.0);
        let starts: Vec<&str> = gcode.iter().filter(|line| line.starts_with("G0")).map(String::as_str).collect();
        assert_eq!(
            starts,
            ["G0 X140.000 Y50.000", "G0 X210.000 Y50.000", "G0 X140.000 Y50.000", "G0 X210.000 Y50.000"]
        );
    }

    #[test]
    fn simulation_never_turns_the_laser_on() {
        let ellipse = ellipse(100.0, 50.0, 40.0, 20.0);
        let segments = ellipse.points().len() - 1;
        let gcode = simulation_gcode(&[ellipse], 3000.0);
        assert!(!gcode.iter().any(|line| line.starts_with("M3") || line.starts_with("M4")));
        assert!(gcode[2].ends_with("S0 F3000"));
        // Header (1) + one pass (G0 + segments) + footer (2).
        assert_eq!(gcode.len(), 1 + 1 + segments + 2);
    }

    #[test]
    fn hatches_a_square_line_by_line() {
        let square = vec![(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        let lines = hatch(&[square], 1.0);
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], ((0.0, 0.5), (4.0, 0.5)));
        // The second line goes back.
        assert_eq!(lines[1], ((4.0, 1.5), (0.0, 1.5)));
    }

    #[test]
    fn a_shape_with_its_own_settings_uses_them() {
        let mut cell = shape(ShapeKind::Rectangle, 100.0, 100.0, 2.0, 2.0);
        cell.cut = Some(CutSettings { power_percent: 20.0, feed: 1500.0, passes: 2, fill: true, interval: 1.0 });
        let plain = shape(ShapeKind::Rectangle, 200.0, 100.0, 2.0, 2.0);
        // Panel: 60 %, 2000 mm/min, 1 pass.
        let gcode = cut_gcode(&[cell, plain], &settings(1), 1000.0);
        // The cell: 4 fill lines per pass, 2 passes, at S200 F1500.
        assert_eq!(gcode.iter().filter(|line| line.ends_with("S200 F1500")).count(), 8);
        // The other shape: its outline once, with the panel's settings.
        assert_eq!(gcode.iter().filter(|line| line.ends_with("S600 F2000")).count(), 1);
    }

    #[test]
    fn announces_where_the_settings_change() {
        let mut cell = shape(ShapeKind::Rectangle, 100.0, 100.0, 2.0, 2.0);
        cell.cut = Some(CutSettings { power_percent: 20.0, feed: 1500.0, passes: 2, fill: false, interval: 1.0 });
        let plain = shape(ShapeKind::Rectangle, 200.0, 100.0, 2.0, 2.0);
        let (gcode, changes) = cut_job(&[cell, plain], &settings(1), 1000.0, AREA);
        // Pass 1: the cell, then the other shape; pass 2: only the cell.
        let seen: Vec<_> = changes.iter().map(|(_, c)| (c.power_percent, c.pass, c.passes)).collect();
        assert_eq!(seen, [(20.0, 1, 2), (60.0, 1, 1), (20.0, 2, 2)]);
        // Each change starts at its shape's first move.
        assert!(changes.iter().all(|(index, _)| gcode[*index].starts_with("G0")));
    }

    /// A 2 × 1 mm image of 2 × 1 dots, centered on (10, 10).
    fn image(values: Vec<u8>) -> Shape {
        let mut image = shape(ShapeKind::Image, 10.0, 10.0, 1.0, 0.5);
        image.raster =
            Some(Raster { columns: 2, rows: 1, values, speed: 1000.0, power_max: 50.0, power_min: 0.0 });
        image
    }

    #[test]
    fn engraves_an_image_line_with_overscan() {
        // Left dot black, right dot white: only the left one burns.
        let gcode = cut_gcode(&[image(vec![255, 0])], &settings(1), 1000.0);
        assert_eq!(
            gcode[2..6],
            ["G0 X6.500 Y10.000", "G1 X9.000 S0 F1000", "G1 X10.000 S500", "G1 X12.500 S0"]
        );
    }

    #[test]
    fn skips_blank_image_lines_and_checks_the_dots() {
        let gcode = cut_gcode(&[image(vec![0, 0])], &settings(1), 1000.0);
        assert!(!gcode.iter().any(|line| line.starts_with("G1")));
        assert!(check_rasters(&[image(vec![255])]).is_err());
        assert!(check_rasters(&[image(vec![255, 0])]).is_ok());
    }

    #[test]
    fn estimates_the_time_of_each_line() {
        let gcode: Vec<String> = ["G21 G90", "G0 X60 Y0", "G1 X60 Y10 F600", "G1 X60 Y20"].map(String::from).into();
        // 60 mm at 3600 mm/min = 1 s; then 10 mm at 600 mm/min = 1 s, twice.
        let times: Vec<f64> = trace(&gcode, 3600.0).iter().map(|line| line.0).collect();
        assert_eq!(times, [0.0, 1.0, 2.0, 3.0]);
        assert_eq!(trace(&gcode, 3600.0)[3].1, (60.0, 20.0));
    }
}
