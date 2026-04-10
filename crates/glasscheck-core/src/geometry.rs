/// A point in two-dimensional view coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    /// Horizontal position.
    pub x: f64,
    /// Vertical position.
    pub y: f64,
}

impl Point {
    /// Creates a point from `x` and `y` coordinates.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A width and height pair in two-dimensional view coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    /// Width component.
    pub width: f64,
    /// Height component.
    pub height: f64,
}

impl Size {
    /// Creates a size from `width` and `height`.
    #[must_use]
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

/// An axis-aligned rectangle in two-dimensional view coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    /// Top-left origin of the rectangle.
    pub origin: Point,
    /// Size of the rectangle.
    pub size: Size,
}

impl Rect {
    /// Creates a rectangle from an origin and size.
    #[must_use]
    pub const fn new(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    /// Returns `true` when the point lies inside or on the rectangle edges.
    #[must_use]
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.origin.x
            && point.y >= self.origin.y
            && point.x <= self.origin.x + self.size.width
            && point.y <= self.origin.y + self.size.height
    }
}
