//! Material Test, as in LightBurn: a grid of cells, each burned with its own power
//! (columns) and speed (rows), with labels showing the values. Rust lays it out; the UI
//! puts it on the canvas, and Start cuts each cell with its own settings (job.rs).

use serde::{Deserialize, Serialize};

use crate::job::Point;
use crate::text;

/// Highest number of rows or columns.
const MAX_COUNT: u32 = 30;
const MAX_PASSES: u32 = 100;

/// A path as (x, y) points in mm; outlines are not closed (the last point is not repeated).
type Contour = Vec<(f64, f64)>;

#[derive(Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Each cell is engraved full, line by line.
    Fill,
    /// Only the cell's outline is burned.
    Line,
}

/// The settings chosen in the Material Test dialog; in JSON with camelCase names.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialTest {
    pub mode: Mode,
    /// Bottom-left corner of the whole test (labels included), in mm.
    pub x: f64,
    pub y: f64,
    /// Columns: power in % of the maximum.
    pub power_count: u32,
    pub power_min: f64,
    pub power_max: f64,
    /// Rows: speed in mm/min.
    pub speed_count: u32,
    pub speed_min: f64,
    pub speed_max: f64,
    pub cell_width: f64,
    pub cell_height: f64,
    /// Gap between cells, in mm.
    pub spacing: f64,
    /// Fill only: distance between the engraved lines, in mm.
    pub interval: f64,
    pub passes: u32,
    pub labels: bool,
    /// Height of the label text, in mm.
    pub label_size: f64,
    pub label_power: f64,
    pub label_speed: f64,
}

/// One thing burned with one power and speed: a cell or a label.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub contours: Vec<Vec<Point>>,
    pub power_percent: f64,
    pub feed: f64,
    /// True: engraved full; false: only the outline.
    pub fill: bool,
    pub label: bool,
}

impl MaterialTest {
    fn check(&self) -> Result<(), String> {
        let count_ok = |count: u32| (1..=MAX_COUNT).contains(&count);
        if !count_ok(self.power_count) || !count_ok(self.speed_count) {
            return Err(format!("Counts must be between 1 and {MAX_COUNT}"));
        }
        let power_ok = |p: f64| (0.0..=100.0).contains(&p);
        if !power_ok(self.power_min) || !power_ok(self.power_max) || !power_ok(self.label_power) {
            return Err("Power must be between 0 and 100%".into());
        }
        if !(self.speed_min > 0.0 && self.speed_max > 0.0 && self.label_speed > 0.0) {
            return Err("Speed must be greater than 0".into());
        }
        if !(self.cell_width > 0.0 && self.cell_height > 0.0 && self.spacing >= 0.0) {
            return Err("Cell size must be greater than 0".into());
        }
        if !(self.interval > 0.0) {
            return Err("Interval must be greater than 0".into());
        }
        if self.labels && !(self.label_size > 0.0) {
            return Err("Label size must be greater than 0".into());
        }
        if !(1..=MAX_PASSES).contains(&self.passes) {
            return Err(format!("Passes must be between 1 and {MAX_PASSES}"));
        }
        Ok(())
    }

    /// The cells and labels at their place on the bed.
    pub fn parts(&self) -> Result<Vec<Part>, String> {
        self.check()?;
        let (w, h, gap) = (self.cell_width, self.cell_height, self.spacing);
        let (columns, rows) = (self.power_count, self.speed_count);
        let grid_width = columns as f64 * w + (columns - 1) as f64 * gap;
        let grid_height = rows as f64 * h + (rows - 1) as f64 * gap;

        // Built with the grid's bottom-left at (0, 0), then moved to (x, y).
        let mut parts: Vec<(Vec<Contour>, f64, f64, bool, bool)> = Vec::new();

        if self.labels {
            let font = label_font()?;
            let size = self.label_size;
            let mut label = |contours: Vec<Contour>| {
                parts.push((contours, self.label_power, self.label_speed, true, true));
            };

            // Power values above the columns, and the title above them.
            for column in 0..columns {
                let text = text_contours(&font, &format_value(value(self.power_min, self.power_max, columns, column)), size)?;
                let center = column as f64 * (w + gap) + w / 2.0;
                label(place(text, Anchor::BottomCenter, center, grid_height + gap));
            }
            let title = text_contours(&font, "Power (%)", size)?;
            label(place(title, Anchor::BottomCenter, grid_width / 2.0, grid_height + 2.0 * gap + size));

            // Speed values left of the rows (top row: lowest speed), and the title turned upright.
            let mut widest = 0.0f64;
            for row in 0..rows {
                let text = text_contours(&font, &format_value(value(self.speed_min, self.speed_max, rows, row)), size)?;
                let (min_x, _, max_x, _) = bounds(&text);
                widest = widest.max(max_x - min_x);
                let middle = (rows - 1 - row) as f64 * (h + gap) + h / 2.0;
                label(place(text, Anchor::MiddleRight, -gap, middle));
            }
            let title = turn_left(text_contours(&font, "Speed (mm/min)", size)?);
            label(place(title, Anchor::MiddleRight, -2.0 * gap - widest, grid_height / 2.0));
        }

        for row in 0..rows {
            let speed = value(self.speed_min, self.speed_max, rows, row);
            for column in 0..columns {
                let power = value(self.power_min, self.power_max, columns, column);
                let (left, bottom) = (column as f64 * (w + gap), (rows - 1 - row) as f64 * (h + gap));
                let cell = vec![(left, bottom), (left + w, bottom), (left + w, bottom + h), (left, bottom + h)];
                parts.push((vec![cell], power, speed, self.mode == Mode::Fill, false));
            }
        }

        // Move everything so the bottom-left of the whole test is at (x, y).
        let all: Vec<Contour> = parts.iter().flat_map(|part| part.0.clone()).collect();
        let (min_x, min_y, _, _) = bounds(&all);
        let (dx, dy) = (self.x - min_x, self.y - min_y);
        Ok(parts
            .into_iter()
            .map(|(contours, power_percent, feed, fill, label)| Part {
                contours: contours
                    .iter()
                    .map(|contour| contour.iter().map(|&(x, y)| Point { x: x + dx, y: y + dy }).collect())
                    .collect(),
                power_percent,
                feed,
                fill,
                label,
            })
            .collect())
    }
}

/// The `index`-th of `count` values evenly spread from `min` to `max`.
fn value(min: f64, max: f64, count: u32, index: u32) -> f64 {
    if count == 1 {
        min
    } else {
        min + (max - min) * index as f64 / (count - 1) as f64
    }
}

/// "10" for whole numbers, "12.5" otherwise.
fn format_value(value: f64) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.1}")
    }
}

/// Arial (on every Windows), otherwise the first installed font.
fn label_font() -> Result<text::FontInfo, String> {
    let fonts = text::installed_fonts();
    fonts
        .iter()
        .find(|font| font.name == "Arial")
        .or_else(|| fonts.first())
        .cloned()
        .ok_or_else(|| "No font found for the labels".to_string())
}

/// The outlines of `content` with capitals `size` mm tall.
fn text_contours(font: &text::FontInfo, content: &str, size: f64) -> Result<Vec<Contour>, String> {
    let outline = text::outline(content, &font.path, font.index, true)?;
    Ok(outline
        .contours
        .iter()
        .map(|contour| contour.iter().map(|p| (p.x * size, p.y * size)).collect())
        .collect())
}

/// Turns the outlines 90° counterclockwise (text reading upward).
fn turn_left(contours: Vec<Contour>) -> Vec<Contour> {
    contours.into_iter().map(|contour| contour.into_iter().map(|(x, y)| (-y, x)).collect()).collect()
}

/// (min x, min y, max x, max y) of all points.
fn bounds(contours: &[Contour]) -> (f64, f64, f64, f64) {
    contours.iter().flatten().fold(
        (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
        |(x0, y0, x1, y1), &(x, y)| (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
    )
}

/// Which point of a text's box goes to the given position.
enum Anchor {
    BottomCenter,
    MiddleRight,
}

fn place(contours: Vec<Contour>, anchor: Anchor, x: f64, y: f64) -> Vec<Contour> {
    let (min_x, min_y, max_x, max_y) = bounds(&contours);
    let (dx, dy) = match anchor {
        Anchor::BottomCenter => (x - (min_x + max_x) / 2.0, y - min_y),
        Anchor::MiddleRight => (x - max_x, y - (min_y + max_y) / 2.0),
    };
    contours.into_iter().map(|contour| contour.into_iter().map(|(px, py)| (px + dx, py + dy)).collect()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test(labels: bool) -> MaterialTest {
        MaterialTest {
            mode: Mode::Fill,
            x: 10.0,
            y: 20.0,
            power_count: 3,
            power_min: 10.0,
            power_max: 30.0,
            speed_count: 2,
            speed_min: 1000.0,
            speed_max: 2000.0,
            cell_width: 5.0,
            cell_height: 5.0,
            spacing: 2.0,
            interval: 1.0,
            passes: 1,
            labels,
            label_size: 3.0,
            label_power: 20.0,
            label_speed: 1500.0,
        }
    }

    #[test]
    fn spreads_values_evenly() {
        assert_eq!(value(10.0, 100.0, 10, 0), 10.0);
        assert_eq!(value(10.0, 100.0, 10, 9), 100.0);
        assert_eq!(value(10.0, 100.0, 1, 0), 10.0);
        assert_eq!(format_value(12.0), "12");
        assert_eq!(format_value(12.25), "12.3");
    }

    #[test]
    fn grid_starts_at_the_chosen_corner() {
        let parts = test(false).parts().unwrap();
        assert_eq!(parts.len(), 6);
        let first = &parts[0].contours[0][0];
        // Top row, first column: its bottom-left is one row (5 + 2 mm) above (10, 20).
        assert_eq!((first.x, first.y), (10.0, 27.0));
        assert_eq!((parts[0].power_percent, parts[0].feed), (10.0, 1000.0));
        assert_eq!((parts[5].power_percent, parts[5].feed), (30.0, 2000.0));
    }

    #[test]
    fn labels_sit_left_and_above_the_grid() {
        let parts = test(true).parts().unwrap();
        // 3 power values + title, 2 speed values + title, 6 cells.
        assert_eq!(parts.len(), 13);
        let min_x = parts.iter().flat_map(|p| p.contours.iter().flatten()).map(|p| p.x).fold(f64::INFINITY, f64::min);
        assert!((min_x - 10.0).abs() < 1e-9);
    }
}
