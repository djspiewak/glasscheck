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
}
