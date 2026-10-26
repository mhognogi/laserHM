//! Text: the fonts installed in Windows, and the outlines of a text in a font.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use i_overlay::core::fill_rule::FillRule;
use i_overlay::float::simplify::SimplifyShape;
use serde::Serialize;
use ttf_parser::{name_id, Face, OutlineBuilder};

use crate::job::Point;

/// Straight segments used for one curve of a letter.
const QUAD_SEGMENTS: usize = 8;
const CUBIC_SEGMENTS: usize = 12;

/// An installed font face, e.g. "Arial Bold".
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontInfo {
    /// Full name, as shown in the font list.
    pub name: String,
    pub path: String,
    /// Face number inside the file (font collections, .ttc, hold several).
    pub index: u32,
}

/// A text turned into outlines. The unit is the height of a capital letter (the
/// font size, in mm like LightBurn, is the capitals' height); the origin is where
/// the first line starts, on its baseline; Y points up, like the machine.
#[derive(Serialize)]
pub struct TextOutline {
    /// Empty when the text has no visible characters (e.g. only spaces).
    pub contours: Vec<Vec<Point>>,
    /// Where the next character would go: the end of the last line, on its baseline.
    pub caret: Point,
}

/// The installed fonts, sorted by name; read from disk once, then kept in memory.
pub fn installed_fonts() -> Vec<FontInfo> {
    static FONTS: OnceLock<Vec<FontInfo>> = OnceLock::new();
    FONTS.get_or_init(scan_fonts).clone()
}

/// The folders with installed fonts: for all users and for this user only.
/// Windows\Fonts and the user's Fonts folder.
fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(windows) = std::env::var("WINDIR") {
        dirs.push(PathBuf::from(windows).join("Fonts"));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("Microsoft").join("Windows").join("Fonts"));
    }
    dirs
}

/// The font files in `dir`.
fn font_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for path in entries.flatten().map(|entry| entry.path()) {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ["ttf", "otf", "ttc", "otc"].contains(&extension.as_str()) {
            files.push(path);
        }
    }
}

fn scan_fonts() -> Vec<FontInfo> {
    let mut files = Vec::new();
    for dir in font_dirs() {
        font_files(&dir, &mut files);
    }
    // A BTreeMap keeps the names sorted and drops duplicates.
    let mut fonts = BTreeMap::new();
    for path in files {
        let Ok(data) = fs::read(&path) else { continue };
        let faces = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
        for index in 0..faces {
            let Ok(face) = Face::parse(&data, index) else { continue };
            if let Some(name) = full_name(&face) {
                let path = path.to_string_lossy().into_owned();
                fonts.entry(name.clone()).or_insert(FontInfo { name, path, index });
            }
        }
    }
    fonts.into_values().collect()
}

/// Fonts used when nothing else is chosen (new texts, Material Test labels): the first one
/// installed. Arial is on every Windows.
pub const DEFAULT_FONTS: [&str; 1] = ["Arial"];

/// The first of `DEFAULT_FONTS` that is installed, otherwise the first font found.
pub fn default_font() -> Option<FontInfo> {
    let fonts = installed_fonts();
    DEFAULT_FONTS
        .iter()
        .find_map(|name| fonts.iter().find(|font| font.name == *name))
        .or_else(|| fonts.first())
        .cloned()
}

/// The face's full name (e.g. "Arial Bold"), in a form we can read.
fn full_name(face: &Face) -> Option<String> {
    face.names()
        .into_iter()
        .filter(|name| name.name_id == name_id::FULL_NAME)
        .find_map(|name| name.to_string())
}

/// Lays out `text` in the font, one line per "\n", and returns the outlines of its letters.
/// `welded` (like LightBurn's "Welded"): letters that overlap become a single outline.
pub fn outline(text: &str, path: &str, index: u32, welded: bool) -> Result<TextOutline, String> {
    let data = fs::read(path).map_err(|e| format!("Cannot read the font: {e}"))?;
    let face = Face::parse(&data, index).map_err(|e| format!("Cannot use the font: {e}"))?;
    // Distance between baselines: ascender − descender + the font's extra line gap.
    let line_height = f64::from(face.height()) + f64::from(face.line_gap());

    let mut flattener = Flattener::default();
    for (number, line) in text.split('\n').enumerate() {
        flattener.offset_x = 0.0;
        flattener.offset_y = -(number as f64) * line_height;
        for character in line.chars().filter(|c| !c.is_control()) {
            // Characters missing from the font use the font's "missing glyph" (usually a box).
            let glyph = face.glyph_index(character).unwrap_or_default();
            face.outline_glyph(glyph, &mut flattener);
            flattener.offset_x += f64::from(face.glyph_hor_advance(glyph).unwrap_or(0));
        }
    }

    // Font units → capital heights.
    let unit = capital_height(&face);
    let scale = |p: &Point| Point { x: p.x / unit, y: p.y / unit };
    let contours: Vec<Vec<Point>> =
        flattener.contours.iter().map(|contour| contour.iter().map(scale).collect()).collect();
    Ok(TextOutline {
        contours: if welded { weld(&contours) } else { contours },
        caret: scale(&Point { x: flattener.offset_x, y: flattener.offset_y }),
    })
}

/// The union of the letters: the outline around everything that is inside at least
/// one letter. A letter's holes turn the other way round than its outline, so with
/// the "non-zero" rule a hole stays a hole, unless another letter covers it.
fn weld(contours: &[Vec<Point>]) -> Vec<Vec<Point>> {
    let paths: Vec<Vec<[f64; 2]>> =
        contours.iter().map(|contour| contour.iter().map(|p| [p.x, p.y]).collect()).collect();
    // The result is a list of shapes, each an outline followed by its holes.
    paths
        .simplify_shape(FillRule::NonZero)
        .into_iter()
        .flatten()
        .map(|contour| contour.into_iter().map(|[x, y]| Point { x, y }).collect())
        .collect()
}

/// Height of a capital letter in font units: from the font's metrics if it has
/// them, otherwise measured on "H", otherwise the ascender.
fn capital_height(face: &Face) -> f64 {
    let measured_h = || {
        let glyph = face.glyph_index('H')?;
        face.glyph_bounding_box(glyph).map(|bounds| bounds.y_max)
    };
    let height = face
        .capital_height()
        .filter(|&height| height > 0)
        .or_else(measured_h)
        .filter(|&height| height > 0)
        .unwrap_or(face.ascender());
    f64::from(height)
}

/// Receives a letter's outline from ttf-parser (it "draws" it by calling these
/// methods) and turns the curves into short straight segments.
#[derive(Default)]
struct Flattener {
    contours: Vec<Vec<Point>>,
    current: Vec<Point>,
    /// Where the current letter starts, in font units: along the line, and the line's baseline.
    offset_x: f64,
    offset_y: f64,
}

impl Flattener {
    fn point(&self, x: f32, y: f32) -> Point {
        Point { x: f64::from(x) + self.offset_x, y: f64::from(y) + self.offset_y }
    }

    fn last(&self) -> Point {
        self.current.last().copied().unwrap_or(Point { x: self.offset_x, y: self.offset_y })
    }
}

impl OutlineBuilder for Flattener {
    fn move_to(&mut self, x: f32, y: f32) {
        self.close();
        let point = self.point(x, y);
        self.current.push(point);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let point = self.point(x, y);
        self.current.push(point);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (p0, p1, p2) = (self.last(), self.point(x1, y1), self.point(x, y));
        for i in 1..=QUAD_SEGMENTS {
            let t = i as f64 / QUAD_SEGMENTS as f64;
            let u = 1.0 - t;
            self.current.push(Point {
                x: u * u * p0.x + 2.0 * u * t * p1.x + t * t * p2.x,
                y: u * u * p0.y + 2.0 * u * t * p1.y + t * t * p2.y,
            });
        }
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (p0, p1, p2, p3) = (self.last(), self.point(x1, y1), self.point(x2, y2), self.point(x, y));
        for i in 1..=CUBIC_SEGMENTS {
            let t = i as f64 / CUBIC_SEGMENTS as f64;
            let u = 1.0 - t;
            self.current.push(Point {
                x: u * u * u * p0.x + 3.0 * u * u * t * p1.x + 3.0 * u * t * t * p2.x + t * t * t * p3.x,
                y: u * u * u * p0.y + 3.0 * u * u * t * p1.y + 3.0 * u * t * t * p2.y + t * t * t * p3.y,
            });
        }
    }

    fn close(&mut self) {
        if self.current.len() > 1 {
            self.contours.push(std::mem::take(&mut self.current));
        } else {
            self.current.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Arial ships with every Windows installation.
    fn arial() -> FontInfo {
        installed_fonts().into_iter().find(|font| font.name == "Arial").expect("Arial is installed")
    }

    #[test]
    fn finds_installed_fonts() {
        assert!(!installed_fonts().is_empty());
        arial();
    }

    #[test]
    fn capitals_sit_on_the_baseline_one_unit_tall() {
        let font = arial();
        let outline = outline("HM", &font.path, font.index, false).unwrap();
        // "H" has one contour, "M" one.
        assert_eq!(outline.contours.len(), 2);
        let ys = outline.contours.iter().flatten().map(|p| p.y);
        let (low, high) = ys.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), y| (lo.min(y), hi.max(y)));
        assert!(low.abs() < 0.01);
        assert!((high - 1.0).abs() < 0.01);
        // The caret is after "M", on the baseline.
        assert!(outline.caret.x > 1.0);
        assert_eq!(outline.caret.y, 0.0);
    }

    #[test]
    fn each_line_starts_below_the_previous_one() {
        let font = arial();
        let outline = outline("A\nB", &font.path, font.index, false).unwrap();
        // The lowest point of the text is the bottom of "B" ("A" sits on y = 0).
        let b_bottom = outline.contours.iter().flatten().map(|p| p.y).fold(f64::INFINITY, f64::min);
        // "B" sits on the second baseline, more than a capital's height lower.
        assert!(b_bottom < -1.0);
        assert!((outline.caret.y - b_bottom).abs() < 0.01);
    }

    #[test]
    fn letters_with_holes_have_several_contours() {
        let font = arial();
        // "o": outer and inner contour.
        assert_eq!(outline("o", &font.path, font.index, false).unwrap().contours.len(), 2);
    }

    #[test]
    fn welding_joins_overlapping_letters_and_keeps_holes() {
        let font = arial();
        // A letter alone: welded, "o" still has its outline and its hole.
        assert_eq!(outline("o", &font.path, font.index, true).unwrap().contours.len(), 2);
        // Two overlapping squares, the second turning the same way: one outline.
        let square = |x0: f64| {
            vec![
                Point { x: x0, y: 0.0 },
                Point { x: x0 + 2.0, y: 0.0 },
                Point { x: x0 + 2.0, y: 2.0 },
                Point { x: x0, y: 2.0 },
            ]
        };
        assert_eq!(weld(&[square(0.0), square(1.0)]).len(), 1);
    }

    #[test]
    fn spaces_have_no_outlines_but_move_the_caret() {
        let font = arial();
        let outline = outline("  ", &font.path, font.index, false).unwrap();
        assert!(outline.contours.is_empty());
        assert!(outline.caret.x > 0.0);
    }
}
