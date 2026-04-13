use crate::Rect;

/// Tolerance for geometry and layout assertions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutTolerance {
    /// Maximum delta allowed on each compared axis.
    pub position: f64,
    /// Maximum delta allowed for size comparisons.
    pub size: f64,
}

impl Default for LayoutTolerance {
    fn default() -> Self {
        Self {
            position: 1.0,
            size: 1.0,
        }
    }
}

/// Errors returned by direct layout assertions.
#[derive(Clone, Debug, PartialEq)]
pub enum LayoutError {
    /// The expected relationship did not hold.
    Relationship {
        expected: &'static str,
        left: Rect,
        right: Rect,
        tolerance: LayoutTolerance,
    },
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Relationship { expected, .. } => {
                write!(f, "layout assertion failed: expected {expected}")
            }
        }
    }
}

impl std::error::Error for LayoutError {}

/// Asserts that `left` is above `right` within `tolerance`.
pub fn assert_above(
    left: Rect,
    right: Rect,
    tolerance: LayoutTolerance,
) -> Result<(), LayoutError> {
    ((left.origin.y + left.size.height) <= (right.origin.y + tolerance.position))
        .then_some(())
        .ok_or(LayoutError::Relationship {
            expected: "left rect above right rect",
            left,
            right,
            tolerance,
        })
}

/// Asserts that `left` is left of `right` within `tolerance`.
pub fn assert_left_of(
    left: Rect,
    right: Rect,
    tolerance: LayoutTolerance,
) -> Result<(), LayoutError> {
    ((left.origin.x + left.size.width) <= (right.origin.x + tolerance.position))
        .then_some(())
        .ok_or(LayoutError::Relationship {
            expected: "left rect left of right rect",
            left,
            right,
            tolerance,
        })
}

/// Asserts that `inner` is contained within `outer` within `tolerance`.
pub fn assert_contained_within(
    inner: Rect,
    outer: Rect,
    tolerance: LayoutTolerance,
) -> Result<(), LayoutError> {
    let within = inner.origin.x >= outer.origin.x - tolerance.position
        && inner.origin.y >= outer.origin.y - tolerance.position
        && inner.origin.x + inner.size.width
            <= outer.origin.x + outer.size.width + tolerance.position
        && inner.origin.y + inner.size.height
            <= outer.origin.y + outer.size.height + tolerance.position;
    within.then_some(()).ok_or(LayoutError::Relationship {
        expected: "inner rect contained within outer rect",
        left: inner,
        right: outer,
        tolerance,
    })
}

/// Asserts that `left` and `right` do not overlap beyond `tolerance`.
pub fn assert_non_overlapping(
    left: Rect,
    right: Rect,
    tolerance: LayoutTolerance,
) -> Result<(), LayoutError> {
    let overlap_x = (left.origin.x < right.origin.x + right.size.width - tolerance.position)
        && (right.origin.x < left.origin.x + left.size.width - tolerance.position);
    let overlap_y = (left.origin.y < right.origin.y + right.size.height - tolerance.position)
        && (right.origin.y < left.origin.y + left.size.height - tolerance.position);
    (!(overlap_x && overlap_y))
        .then_some(())
        .ok_or(LayoutError::Relationship {
            expected: "rectangles not overlapping",
            left,
            right,
            tolerance,
        })
}

/// Asserts that `left` and `right` are vertically centered within `tolerance`.
pub fn assert_vertical_alignment(
    left: Rect,
    right: Rect,
    tolerance: LayoutTolerance,
) -> Result<(), LayoutError> {
    let left_mid = left.origin.y + left.size.height / 2.0;
    let right_mid = right.origin.y + right.size.height / 2.0;
    ((left_mid - right_mid).abs() <= tolerance.position)
        .then_some(())
        .ok_or(LayoutError::Relationship {
            expected: "rectangles vertically aligned",
            left,
            right,
            tolerance,
        })
}

/// Asserts that `left` and `right` have the same width within `tolerance`.
pub fn assert_same_width(
    left: Rect,
    right: Rect,
    tolerance: LayoutTolerance,
) -> Result<(), LayoutError> {
    ((left.size.width - right.size.width).abs() <= tolerance.size)
        .then_some(())
        .ok_or(LayoutError::Relationship {
            expected: "rectangles same width",
            left,
            right,
            tolerance,
        })
}

/// Asserts that `left` and `right` have the same height within `tolerance`.
pub fn assert_same_height(
    left: Rect,
    right: Rect,
    tolerance: LayoutTolerance,
) -> Result<(), LayoutError> {
    ((left.size.height - right.size.height).abs() <= tolerance.size)
        .then_some(())
        .ok_or(LayoutError::Relationship {
            expected: "rectangles same height",
            left,
            right,
            tolerance,
        })
}

/// Asserts that `left` and `right` are horizontally centered within `tolerance`.
pub fn assert_horizontal_alignment(
    left: Rect,
    right: Rect,
    tolerance: LayoutTolerance,
) -> Result<(), LayoutError> {
    let left_mid = left.origin.x + left.size.width / 2.0;
    let right_mid = right.origin.x + right.size.width / 2.0;
    ((left_mid - right_mid).abs() <= tolerance.position)
        .then_some(())
        .ok_or(LayoutError::Relationship {
            expected: "rectangles horizontally aligned",
            left,
            right,
            tolerance,
        })
}

/// Asserts that `rect` contains `point` within `tolerance`.
pub fn assert_contains_point(
    rect: Rect,
    point: crate::Point,
    tolerance: LayoutTolerance,
) -> Result<(), LayoutError> {
    let min_x = rect.origin.x - tolerance.position;
    let min_y = rect.origin.y - tolerance.position;
    let max_x = rect.origin.x + rect.size.width + tolerance.position;
    let max_y = rect.origin.y + rect.size.height + tolerance.position;
    (point.x >= min_x && point.y >= min_y && point.x <= max_x && point.y <= max_y)
        .then_some(())
        .ok_or(LayoutError::Relationship {
            expected: "rectangle contains point",
            left: rect,
            right: Rect::new(point, crate::Size::new(0.0, 0.0)),
            tolerance,
        })
}

/// Asserts that `left` and `right` are horizontally adjacent within `tolerance`.
pub fn assert_adjacent_horizontally(
    left: Rect,
    right: Rect,
    tolerance: LayoutTolerance,
) -> Result<(), LayoutError> {
    let horizontal_gap_ok =
        (left.origin.x + left.size.width - right.origin.x).abs() <= tolerance.position;
    let vertical_overlap = spans_overlap(
        left.origin.y,
        left.origin.y + left.size.height,
        right.origin.y,
        right.origin.y + right.size.height,
        tolerance.position,
    );

    (horizontal_gap_ok && vertical_overlap)
        .then_some(())
        .ok_or(LayoutError::Relationship {
            expected: "rectangles horizontally adjacent",
            left,
            right,
            tolerance,
        })
}

/// Asserts that `upper` and `lower` are vertically adjacent within `tolerance`.
pub fn assert_adjacent_vertically(
    upper: Rect,
    lower: Rect,
    tolerance: LayoutTolerance,
) -> Result<(), LayoutError> {
    let vertical_gap_ok =
        (upper.origin.y + upper.size.height - lower.origin.y).abs() <= tolerance.position;
    let horizontal_overlap = spans_overlap(
        upper.origin.x,
        upper.origin.x + upper.size.width,
        lower.origin.x,
        lower.origin.x + lower.size.width,
        tolerance.position,
    );

    (vertical_gap_ok && horizontal_overlap)
        .then_some(())
        .ok_or(LayoutError::Relationship {
            expected: "rectangles vertically adjacent",
            left: upper,
            right: lower,
            tolerance,
        })
}

fn spans_overlap(start_a: f64, end_a: f64, start_b: f64, end_b: f64, tolerance: f64) -> bool {
    start_a <= end_b + tolerance && start_b <= end_a + tolerance
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point, Size};

    fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
        Rect::new(Point::new(x, y), Size::new(width, height))
    }

    #[test]
    fn above_passes_for_non_overlapping_rects() {
        assert!(assert_above(
            rect(0.0, 0.0, 10.0, 10.0),
            rect(0.0, 12.0, 10.0, 10.0),
            LayoutTolerance::default()
        )
        .is_ok());
    }

    #[test]
    fn above_fails_when_rectangles_overlap() {
        assert!(assert_above(
            rect(0.0, 4.0, 10.0, 10.0),
            rect(0.0, 12.0, 10.0, 10.0),
            LayoutTolerance::default()
        )
        .is_err());
    }

    #[test]
    fn vertical_alignment_passes_with_small_offset() {
        assert!(assert_vertical_alignment(
            rect(0.0, 0.0, 10.0, 20.0),
            rect(20.0, 0.5, 10.0, 19.0),
            LayoutTolerance::default()
        )
        .is_ok());
    }

    #[test]
    fn vertical_alignment_fails_with_large_offset() {
        assert!(assert_vertical_alignment(
            rect(0.0, 0.0, 10.0, 20.0),
            rect(20.0, 4.0, 10.0, 20.0),
            LayoutTolerance::default()
        )
        .is_err());
    }

    #[test]
    fn same_size_and_alignment_helpers_cover_positive_and_negative_cases() {
        assert!(assert_same_width(
            rect(0.0, 0.0, 10.0, 20.0),
            rect(20.0, 0.0, 10.5, 20.0),
            LayoutTolerance::default()
        )
        .is_ok());
        assert!(assert_same_width(
            rect(0.0, 0.0, 10.0, 20.0),
            rect(20.0, 0.0, 14.0, 20.0),
            LayoutTolerance::default()
        )
        .is_err());
        assert!(assert_same_height(
            rect(0.0, 0.0, 10.0, 20.0),
            rect(20.0, 0.0, 10.0, 20.5),
            LayoutTolerance::default()
        )
        .is_ok());
        assert!(assert_horizontal_alignment(
            rect(0.0, 0.0, 10.0, 20.0),
            rect(0.5, 20.0, 9.0, 20.0),
            LayoutTolerance::default()
        )
        .is_ok());
        assert!(assert_horizontal_alignment(
            rect(0.0, 0.0, 10.0, 20.0),
            rect(8.0, 20.0, 9.0, 20.0),
            LayoutTolerance::default()
        )
        .is_err());
    }

    #[test]
    fn point_and_adjacency_helpers_cover_positive_and_negative_cases() {
        assert!(assert_contains_point(
            rect(0.0, 0.0, 10.0, 10.0),
            Point::new(10.5, 10.5),
            LayoutTolerance::default()
        )
        .is_ok());
        assert!(assert_contains_point(
            rect(0.0, 0.0, 10.0, 10.0),
            Point::new(12.0, 12.0),
            LayoutTolerance::default()
        )
        .is_err());
        assert!(assert_adjacent_horizontally(
            rect(0.0, 0.0, 10.0, 10.0),
            rect(10.5, 0.0, 10.0, 10.0),
            LayoutTolerance::default()
        )
        .is_ok());
        assert!(assert_adjacent_horizontally(
            rect(0.0, 0.0, 10.0, 10.0),
            rect(13.0, 0.0, 10.0, 10.0),
            LayoutTolerance::default()
        )
        .is_err());
        assert!(assert_adjacent_horizontally(
            rect(0.0, 0.0, 10.0, 10.0),
            rect(10.5, 25.0, 10.0, 10.0),
            LayoutTolerance::default()
        )
        .is_err());
        assert!(assert_adjacent_vertically(
            rect(0.0, 0.0, 10.0, 10.0),
            rect(0.0, 11.0, 10.0, 10.0),
            LayoutTolerance::default()
        )
        .is_ok());
        assert!(assert_adjacent_vertically(
            rect(0.0, 0.0, 10.0, 10.0),
            rect(0.0, 14.0, 10.0, 10.0),
            LayoutTolerance::default()
        )
        .is_err());
        assert!(assert_adjacent_vertically(
            rect(0.0, 0.0, 10.0, 10.0),
            rect(25.0, 11.0, 10.0, 10.0),
            LayoutTolerance::default()
        )
        .is_err());
    }
}
