//! Boolean operations on shapes, as in LightBurn: Weld, Union, Subtract and Intersection.

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::simplify::SimplifyShape;
use i_overlay::float::single::SingleFloatOverlay;
use serde::Deserialize;

use crate::job::{Point, Shape, ShapeKind};

/// The operation; in JSON "weld", "union", "subtract" or "intersect".
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BooleanOp {
    /// Joins all the shapes (2 or more) into one.
    Weld,
    /// Joins two shapes.
    Union,
    /// The first shape without the part covered by the second.
    Subtract,
    /// Only the part the two shapes have in common.
    Intersect,
}

/// The area of one shape, as outlines from i_overlay: each piece is an outline followed
/// by its holes, all turning in a consistent direction. A shape's own overlaps (e.g.
/// letters of a text) are joined first, so shapes turning either way combine correctly.
type Area = Vec<Vec<Vec<[f64; 2]>>>;

fn area(shape: &Shape) -> Area {
    let contours: Vec<Vec<[f64; 2]>> = shape
        .paths()
        .into_iter()
        .map(|mut path| {
            // The paths end where they start; i_overlay wants each corner once.
            path.pop();
            path.into_iter().map(|(x, y)| [x, y]).collect()
        })
        .collect();
    contours.simplify_shape(FillRule::NonZero)
}

/// Applies `op` to the shapes (in the order they were selected) and returns the outlines
/// of the result, in machine coordinates (mm). Open lines have no inside and are refused.
pub fn combine(op: BooleanOp, shapes: &[Shape]) -> Result<Vec<Vec<Point>>, String> {
    match op {
        BooleanOp::Weld if shapes.len() < 2 => return Err("Weld needs at least 2 selected shapes".into()),
        BooleanOp::Union | BooleanOp::Subtract | BooleanOp::Intersect if shapes.len() != 2 => {
            return Err("This operation needs exactly 2 selected shapes".into())
        }
        _ => {}
    }
    if shapes.iter().any(|shape| shape.kind == ShapeKind::Path && !shape.closed) {
        return Err("Open lines have no inside: close them first".into());
    }

    let areas: Vec<Area> = shapes.iter().map(area).collect();
    let result: Area = match op {
        BooleanOp::Weld | BooleanOp::Union => {
            let all: Vec<Vec<[f64; 2]>> = areas.into_iter().flatten().flatten().collect();
            all.simplify_shape(FillRule::NonZero)
        }
        BooleanOp::Subtract => areas[0].overlay(&areas[1], OverlayRule::Difference, FillRule::NonZero),
        BooleanOp::Intersect => areas[0].overlay(&areas[1], OverlayRule::Intersect, FillRule::NonZero),
    };

    let contours: Vec<Vec<Point>> = result
        .into_iter()
        .flatten()
        .filter(|contour| contour.len() >= 3)
        .map(|contour| contour.into_iter().map(|[x, y]| Point { x, y }).collect())
        .collect();
    if contours.is_empty() {
        return Err("The result is empty: the shapes do not overlap".into());
    }
    Ok(contours)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(cx: f64, cy: f64, r: f64) -> Shape {
        Shape { kind: ShapeKind::Rectangle, cx, cy, rx: r, ry: r, contours: Vec::new(), closed: false, angle: 0.0, cut: None, raster: None }
    }

    /// The area enclosed by a contour (shoelace formula), always positive.
    fn area_of(contour: &[Point]) -> f64 {
        let n = contour.len();
        let twice: f64 = (0..n)
            .map(|i| {
                let (a, b) = (contour[i], contour[(i + 1) % n]);
                a.x * b.y - b.x * a.y
            })
            .sum();
        twice.abs() / 2.0
    }

    // Two 20 × 20 mm squares overlapping in a 10 × 20 mm strip.
    fn pair() -> [Shape; 2] {
        [square(50.0, 50.0, 10.0), square(60.0, 50.0, 10.0)]
    }

    #[test]
    fn union_is_one_outline_around_both() {
        let result = combine(BooleanOp::Union, &pair()).unwrap();
        assert_eq!(result.len(), 1);
        assert!((area_of(&result[0]) - 600.0).abs() < 0.01);
    }

    #[test]
    fn subtract_keeps_the_first_without_the_second() {
        let result = combine(BooleanOp::Subtract, &pair()).unwrap();
        assert_eq!(result.len(), 1);
        assert!((area_of(&result[0]) - 200.0).abs() < 0.01);
    }

    #[test]
    fn intersection_keeps_the_common_part() {
        let result = combine(BooleanOp::Intersect, &pair()).unwrap();
        assert!((area_of(&result[0]) - 200.0).abs() < 0.01);
    }

    #[test]
    fn a_small_shape_subtracted_from_inside_a_big_one_leaves_a_hole() {
        let result = combine(BooleanOp::Subtract, &[square(50.0, 50.0, 20.0), square(50.0, 50.0, 5.0)]).unwrap();
        // The outline and the hole.
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn refuses_wrong_counts_open_lines_and_empty_results() {
        assert!(combine(BooleanOp::Weld, &[square(0.0, 0.0, 1.0)]).is_err());
        let [a, b] = pair();
        assert!(combine(BooleanOp::Subtract, &[a.clone(), b.clone(), a.clone()]).is_err());
        let mut line = a.clone();
        line.kind = ShapeKind::Path;
        line.contours = vec![vec![Point { x: -1.0, y: 0.0 }, Point { x: 1.0, y: 0.0 }]];
        assert!(combine(BooleanOp::Union, &[line, b]).is_err());
        let far = square(200.0, 200.0, 5.0);
        assert!(combine(BooleanOp::Intersect, &[a, far]).is_err());
    }
}
