use std::io;
use std::path::{Path, PathBuf};

use crate::{compare_images, CompareConfig, CompareResult, Image, Point, Rect, RegionSpec, Size};

/// An RGBA color used for text expectations and compositing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RgbaColor {
    /// Red channel.
    pub red: u8,
    /// Green channel.
    pub green: u8,
    /// Blue channel.
    pub blue: u8,
    /// Alpha channel.
    pub alpha: u8,
}

impl RgbaColor {
    /// Creates a color from RGBA8 components.
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct TextStyleDefaults {
    font_family: Option<String>,
    font_name: Option<String>,
    point_size: f64,
    weight: Option<u16>,
    italic: bool,
    foreground: RgbaColor,
    background: Option<RgbaColor>,
}

impl Default for TextStyleDefaults {
    fn default() -> Self {
        Self {
            font_family: None,
            font_name: None,
            point_size: 14.0,
            weight: None,
            italic: false,
            foreground: RgbaColor::new(0, 0, 0, 255),
            background: None,
        }
    }
}

trait TextStyleBuilder: Sized {
    fn font_family_mut(&mut self) -> &mut Option<String>;
    fn font_name_mut(&mut self) -> &mut Option<String>;
    fn point_size_mut(&mut self) -> &mut f64;
    fn weight_mut(&mut self) -> &mut Option<u16>;
    fn italic_mut(&mut self) -> &mut bool;
    fn foreground_mut(&mut self) -> &mut RgbaColor;
    fn background_mut(&mut self) -> &mut Option<RgbaColor>;

    fn with_font_family(mut self, family: impl Into<String>) -> Self {
        *self.font_family_mut() = Some(family.into());
        self
    }

    fn with_font_name(mut self, name: impl Into<String>) -> Self {
        *self.font_name_mut() = Some(name.into());
        self
    }

    fn with_point_size(mut self, point_size: f64) -> Self {
        *self.point_size_mut() = point_size;
        self
    }

    fn with_weight(mut self, weight: u16) -> Self {
        *self.weight_mut() = Some(weight);
        self
    }

    fn italic(mut self, italic: bool) -> Self {
        *self.italic_mut() = italic;
        self
    }

    fn with_foreground(mut self, foreground: RgbaColor) -> Self {
        *self.foreground_mut() = foreground;
        self
    }

    fn with_background(mut self, background: RgbaColor) -> Self {
        *self.background_mut() = Some(background);
        self
    }
}

/// Declarative specification of text expected to appear in a UI region.
#[derive(Clone, Debug, PartialEq)]
pub struct TextExpectation {
    /// Expected text content.
    pub content: String,
    /// Region that should contain the rendered text.
    pub rect: Rect,
    /// Optional font family name.
    pub font_family: Option<String>,
    /// Optional concrete font face name.
    pub font_name: Option<String>,
    /// Expected point size.
    pub point_size: f64,
    /// Optional CSS-style font weight.
    pub weight: Option<u16>,
    /// Whether italic styling is expected.
    pub italic: bool,
    /// Expected foreground text color.
    pub foreground: RgbaColor,
    /// Optional background color. When absent, the reference is compared
    /// against a background estimated from the live capture.
    pub background: Option<RgbaColor>,
}

impl TextExpectation {
    /// Creates a text expectation with default styling for the given region.
    #[must_use]
    pub fn new(content: impl Into<String>, rect: Rect) -> Self {
        let style = TextStyleDefaults::default();
        Self {
            content: content.into(),
            rect,
            font_family: style.font_family,
            font_name: style.font_name,
            point_size: style.point_size,
            weight: style.weight,
            italic: style.italic,
            foreground: style.foreground,
            background: style.background,
        }
    }
}

impl TextStyleBuilder for TextExpectation {
    fn font_family_mut(&mut self) -> &mut Option<String> {
        &mut self.font_family
    }

    fn font_name_mut(&mut self) -> &mut Option<String> {
        &mut self.font_name
    }

    fn point_size_mut(&mut self) -> &mut f64 {
        &mut self.point_size
    }

    fn weight_mut(&mut self) -> &mut Option<u16> {
        &mut self.weight
    }

    fn italic_mut(&mut self) -> &mut bool {
        &mut self.italic
    }

    fn foreground_mut(&mut self) -> &mut RgbaColor {
        &mut self.foreground
    }

    fn background_mut(&mut self) -> &mut Option<RgbaColor> {
        &mut self.background
    }
}

impl TextExpectation {
    /// Sets the expected font family.
    #[must_use]
    pub fn with_font_family(self, family: impl Into<String>) -> Self {
        TextStyleBuilder::with_font_family(self, family)
    }

    /// Sets the expected concrete font name.
    #[must_use]
    pub fn with_font_name(self, name: impl Into<String>) -> Self {
        TextStyleBuilder::with_font_name(self, name)
    }

    /// Sets the expected point size.
    #[must_use]
    pub fn with_point_size(self, point_size: f64) -> Self {
        TextStyleBuilder::with_point_size(self, point_size)
    }

    /// Sets the expected font weight.
    #[must_use]
    pub fn with_weight(self, weight: u16) -> Self {
        TextStyleBuilder::with_weight(self, weight)
    }

    /// Sets whether italic styling is expected.
    #[must_use]
    pub fn italic(self, italic: bool) -> Self {
        TextStyleBuilder::italic(self, italic)
    }

    /// Sets the expected foreground color.
    #[must_use]
    pub fn with_foreground(self, foreground: RgbaColor) -> Self {
        TextStyleBuilder::with_foreground(self, foreground)
    }

    /// Sets the expected background color.
    #[must_use]
    pub fn with_background(self, background: RgbaColor) -> Self {
        TextStyleBuilder::with_background(self, background)
    }
}

/// Declarative text expectation whose target region is resolved semantically.
#[derive(Clone, Debug, PartialEq)]
pub struct AnchoredTextExpectation {
    /// Expected text content.
    pub content: String,
    /// Region that should contain the rendered text.
    pub region: RegionSpec,
    /// Optional font family name.
    pub font_family: Option<String>,
    /// Optional concrete font face name.
    pub font_name: Option<String>,
    /// Expected point size.
    pub point_size: f64,
    /// Optional CSS-style font weight.
    pub weight: Option<u16>,
    /// Whether italic styling is expected.
    pub italic: bool,
    /// Expected foreground text color.
    pub foreground: RgbaColor,
    /// Optional background color. When absent, the reference is compared
    /// against a background estimated from the live capture.
    pub background: Option<RgbaColor>,
}

impl AnchoredTextExpectation {
    /// Creates an anchored text expectation with default styling.
    #[must_use]
    pub fn new(content: impl Into<String>, region: RegionSpec) -> Self {
        let style = TextStyleDefaults::default();
        Self {
            content: content.into(),
            region,
            font_family: style.font_family,
            font_name: style.font_name,
            point_size: style.point_size,
            weight: style.weight,
            italic: style.italic,
            foreground: style.foreground,
            background: style.background,
        }
    }

    /// Resolves to a concrete text expectation using `rect`.
    #[must_use]
    pub fn resolve(&self, rect: Rect) -> TextExpectation {
        TextExpectation {
            content: self.content.clone(),
            rect,
            font_family: self.font_family.clone(),
            font_name: self.font_name.clone(),
            point_size: self.point_size,
            weight: self.weight,
            italic: self.italic,
            foreground: self.foreground,
            background: self.background,
        }
    }
}

impl TextStyleBuilder for AnchoredTextExpectation {
    fn font_family_mut(&mut self) -> &mut Option<String> {
        &mut self.font_family
    }

    fn font_name_mut(&mut self) -> &mut Option<String> {
        &mut self.font_name
    }

    fn point_size_mut(&mut self) -> &mut f64 {
        &mut self.point_size
    }

    fn weight_mut(&mut self) -> &mut Option<u16> {
        &mut self.weight
    }

    fn italic_mut(&mut self) -> &mut bool {
        &mut self.italic
    }

    fn foreground_mut(&mut self) -> &mut RgbaColor {
        &mut self.foreground
    }

    fn background_mut(&mut self) -> &mut Option<RgbaColor> {
        &mut self.background
    }
}

impl AnchoredTextExpectation {
    /// Sets the expected font family.
    #[must_use]
    pub fn with_font_family(self, family: impl Into<String>) -> Self {
        TextStyleBuilder::with_font_family(self, family)
    }

    /// Sets the expected concrete font name.
    #[must_use]
    pub fn with_font_name(self, name: impl Into<String>) -> Self {
        TextStyleBuilder::with_font_name(self, name)
    }

    /// Sets the expected point size.
    #[must_use]
    pub fn with_point_size(self, point_size: f64) -> Self {
        TextStyleBuilder::with_point_size(self, point_size)
    }

    /// Sets the expected font weight.
    #[must_use]
    pub fn with_weight(self, weight: u16) -> Self {
        TextStyleBuilder::with_weight(self, weight)
    }

    /// Sets whether italic styling is expected.
    #[must_use]
    pub fn italic(self, italic: bool) -> Self {
        TextStyleBuilder::italic(self, italic)
    }

    /// Sets the expected foreground color.
    #[must_use]
    pub fn with_foreground(self, foreground: RgbaColor) -> Self {
        TextStyleBuilder::with_foreground(self, foreground)
    }

    /// Sets the expected background color.
    #[must_use]
    pub fn with_background(self, background: RgbaColor) -> Self {
        TextStyleBuilder::with_background(self, background)
    }
}

/// Configuration for rendered-text assertions.
#[derive(Clone, Debug)]
pub struct TextAssertionConfig {
    /// Pixel comparison settings.
    pub compare: CompareConfig,
    /// Whether to write a diff artifact on failure.
    pub write_diff: bool,
}

impl Default for TextAssertionConfig {
    fn default() -> Self {
        Self {
            compare: CompareConfig {
                channel_tolerance: 12,
                match_threshold: 0.98,
                generate_diff: true,
            },
            write_diff: true,
        }
    }
}

/// Paths to artifacts emitted by a failed text assertion.
#[derive(Clone, Debug, Default)]
pub struct TextAssertionArtifacts {
    /// Path to the captured UI region.
    pub actual_path: PathBuf,
    /// Path to the rendered reference image.
    pub expected_path: PathBuf,
    /// Path to the generated diff image, when available.
    pub diff_path: Option<PathBuf>,
}

/// Backend capable of rendering a text reference image and capturing a live UI region.
pub trait TextRenderer {
    /// Backend-specific error type.
    type Error;

    /// Renders the expected text into a reference image.
    fn render_text_reference(&self, expectation: &TextExpectation) -> Result<Image, Self::Error>;

    /// Captures the live UI pixels for the target text region.
    fn capture_text_region(&self, expectation: &TextExpectation) -> Result<Image, Self::Error>;
}

/// Errors returned by rendered-text assertions.
#[derive(Debug)]
pub enum TextAssertionError<E> {
    /// Filesystem I/O failed while writing artifacts.
    Io(io::Error),
    /// The backend could not render the reference image.
    Render(E),
    /// The backend could not capture the live UI region.
    Capture(E),
    /// The live UI region did not match the rendered reference.
    Mismatch {
        expectation: TextExpectation,
        artifacts: TextAssertionArtifacts,
        result: CompareResult,
    },
}

impl<E> std::fmt::Display for TextAssertionError<E>
where
    E: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Render(error) => write!(f, "text reference render failed: {error}"),
            Self::Capture(error) => write!(f, "text region capture failed: {error}"),
            Self::Mismatch {
                expectation,
                artifacts,
                result,
            } => write!(
                f,
                "rendered text mismatch for {:?}: {:.2}% match, expected at {}, actual at {}",
                expectation.content,
                result.matched_ratio * 100.0,
                artifacts.expected_path.display(),
                artifacts.actual_path.display()
            ),
        }
    }
}

impl<E> std::error::Error for TextAssertionError<E> where E: std::error::Error + 'static {}

impl<E> From<io::Error> for TextAssertionError<E> {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// Compares captured text pixels against a rendered reference for `expectation`.
///
/// When no background color is provided, transparent reference pixels are
/// treated as valid over any opaque concrete background that could produce the
/// captured result after alpha compositing.
#[must_use]
pub fn compare_rendered_text(
    actual: &Image,
    expected: &Image,
    expectation: &TextExpectation,
    config: &TextAssertionConfig,
) -> CompareResult {
    prepare_rendered_text_comparison(actual, expected, expectation, config).result
}

/// Asserts that a backend renders and captures text consistently for `expectation`.
///
/// On failure, writes actual, expected, and optional diff artifacts into
/// `artifact_dir`.
pub fn assert_text_renders<R>(
    renderer: &R,
    expectation: &TextExpectation,
    artifact_dir: &Path,
    config: &TextAssertionConfig,
) -> Result<(), TextAssertionError<R::Error>>
where
    R: TextRenderer,
    R::Error: std::fmt::Display,
{
    let expected = renderer
        .render_text_reference(expectation)
        .map_err(TextAssertionError::Render)?;
    let actual = renderer
        .capture_text_region(expectation)
        .map_err(TextAssertionError::Capture)?;

    let comparison = prepare_rendered_text_comparison(&actual, &expected, expectation, config);
    let result = comparison.result;
    if result.passed {
        return Ok(());
    }

    std::fs::create_dir_all(artifact_dir)?;
    let actual_for_artifact = comparison
        .comparison_bounds
        .map(|bounds| actual.crop(bounds))
        .unwrap_or_else(|| actual.clone());
    let expected_for_artifact = comparison
        .comparison_bounds
        .map(|bounds| comparison.composited_expected.crop(bounds))
        .unwrap_or_else(|| comparison.composited_expected.clone());
    let actual_path = artifact_dir.join("actual.png");
    crate::save_png(&actual_for_artifact, &actual_path)?;
    let expected_path = artifact_dir.join("expected.png");
    crate::save_png(&expected_for_artifact, &expected_path)?;
    let diff_path = if config.write_diff {
        let path = artifact_dir.join("diff.png");
        if let Some(image) = result.diff_image.as_ref() {
            crate::save_png(image, &path)?;
            Some(path)
        } else {
            None
        }
    } else {
        None
    };

    Err(TextAssertionError::Mismatch {
        expectation: expectation.clone(),
        artifacts: TextAssertionArtifacts {
            actual_path,
            expected_path,
            diff_path,
        },
        result,
    })
}

struct RenderedTextComparison {
    composited_expected: Image,
    comparison_bounds: Option<Rect>,
    result: CompareResult,
}

fn prepare_rendered_text_comparison(
    actual: &Image,
    expected: &Image,
    expectation: &TextExpectation,
    config: &TextAssertionConfig,
) -> RenderedTextComparison {
    if expectation.background.is_none() {
        if !actual.is_valid_rgba() || !expected.is_valid_rgba() {
            let result = CompareResult {
                matched_ratio: 0.0,
                mismatched_pixels: u32::MAX,
                passed: false,
                diff_image: None,
            };
            return RenderedTextComparison {
                composited_expected: expected.clone(),
                comparison_bounds: None,
                result,
            };
        }

        if actual.width != expected.width || actual.height != expected.height {
            let result = CompareResult {
                matched_ratio: 0.0,
                mismatched_pixels: u32::MAX,
                passed: false,
                diff_image: None,
            };
            return RenderedTextComparison {
                composited_expected: expected.clone(),
                comparison_bounds: None,
                result,
            };
        }

        if actual.width == 0 || actual.height == 0 {
            let result = CompareResult {
                matched_ratio: 1.0,
                mismatched_pixels: 0,
                passed: true,
                diff_image: None,
            };
            return RenderedTextComparison {
                composited_expected: expected.clone(),
                comparison_bounds: None,
                result,
            };
        }

        let result = compare_text_against_transparent_reference(actual, expected, &config.compare);
        return RenderedTextComparison {
            composited_expected: expected.clone(),
            comparison_bounds: None,
            result,
        };
    }

    let background = expectation
        .background
        .expect("background should be present for flat-background text comparison");
    let composited_expected = composite_over_background(expected, background);
    if actual.width != composited_expected.width || actual.height != composited_expected.height {
        let result = compare_images(actual, &composited_expected, &config.compare);
        return RenderedTextComparison {
            composited_expected,
            comparison_bounds: None,
            result,
        };
    }
    let comparison_bounds = comparison_bounds(
        actual,
        &composited_expected,
        background,
        config.compare.channel_tolerance,
    );
    let focused_actual = comparison_bounds
        .map(|bounds| actual.crop(bounds))
        .unwrap_or_else(|| actual.clone());
    let focused_expected = comparison_bounds
        .map(|bounds| composited_expected.crop(bounds))
        .unwrap_or_else(|| composited_expected.clone());
    let result = compare_images(&focused_actual, &focused_expected, &config.compare);

    RenderedTextComparison {
        composited_expected,
        comparison_bounds,
        result,
    }
}

fn composite_over_background(image: &Image, background: RgbaColor) -> Image {
    let mut data = Vec::with_capacity(image.data.len());
    for pixel in image.data.chunks_exact(4) {
        let alpha = f64::from(pixel[3]) / 255.0;
        let inverse = 1.0 - alpha;
        data.push(composite_channel(pixel[0], background.red, alpha, inverse));
        data.push(composite_channel(
            pixel[1],
            background.green,
            alpha,
            inverse,
        ));
        data.push(composite_channel(pixel[2], background.blue, alpha, inverse));
        data.push(255);
    }
    Image::new(image.width, image.height, data)
}

#[cfg(test)]
fn composite_over_background_image(image: &Image, background: &Image) -> Image {
    let mut data = Vec::with_capacity(image.data.len());
    for (pixel, background) in image
        .data
        .chunks_exact(4)
        .zip(background.data.chunks_exact(4))
    {
        let alpha = f64::from(pixel[3]) / 255.0;
        let inverse = 1.0 - alpha;
        data.push(composite_channel(pixel[0], background[0], alpha, inverse));
        data.push(composite_channel(pixel[1], background[1], alpha, inverse));
        data.push(composite_channel(pixel[2], background[2], alpha, inverse));
        data.push(255);
    }
    Image::new(image.width, image.height, data)
}

fn compare_text_against_transparent_reference(
    actual: &Image,
    expected: &Image,
    config: &CompareConfig,
) -> CompareResult {
    let total_pixels = u64::from(actual.width) * u64::from(actual.height);
    if total_pixels == 0 {
        return CompareResult {
            matched_ratio: 1.0,
            mismatched_pixels: 0,
            passed: true,
            diff_image: None,
        };
    }

    let tolerance = i16::from(config.channel_tolerance);
    let mut mismatched = 0u64;
    let mut diff = config
        .generate_diff
        .then(|| Vec::with_capacity(actual.data.len()));

    for (actual_pixel, expected_pixel) in actual
        .data
        .chunks_exact(4)
        .zip(expected.data.chunks_exact(4))
    {
        let is_match = transparent_reference_pixel_matches(actual_pixel, expected_pixel, tolerance);

        if !is_match {
            mismatched += 1;
        }

        if let Some(buffer) = diff.as_mut() {
            if is_match {
                buffer.extend_from_slice(&[0, 255, 0, 255]);
            } else {
                buffer.extend_from_slice(&[255, 0, 0, 255]);
            }
        }
    }

    let matched_ratio = (total_pixels - mismatched) as f64 / total_pixels as f64;
    CompareResult {
        matched_ratio,
        mismatched_pixels: mismatched.min(u64::from(u32::MAX)) as u32,
        passed: matched_ratio >= config.match_threshold,
        diff_image: if mismatched == 0 {
            None
        } else {
            diff.map(|data| Image::new(actual.width, actual.height, data))
        },
    }
}

fn transparent_reference_pixel_matches(actual: &[u8], expected: &[u8], tolerance: i16) -> bool {
    if (i16::from(actual[3]) - 255).abs() > tolerance {
        return false;
    }

    let alpha = expected[3];
    channel_matches_transparent_reference(actual[0], expected[0], alpha, tolerance)
        && channel_matches_transparent_reference(actual[1], expected[1], alpha, tolerance)
        && channel_matches_transparent_reference(actual[2], expected[2], alpha, tolerance)
}

fn channel_matches_transparent_reference(
    actual: u8,
    foreground: u8,
    alpha: u8,
    tolerance: i16,
) -> bool {
    if alpha == 0 {
        return true;
    }

    let alpha = f64::from(alpha) / 255.0;
    let inverse = 1.0 - alpha;
    let minimum = i16::from(composite_channel(foreground, 0, alpha, inverse));
    let maximum = i16::from(composite_channel(foreground, 255, alpha, inverse));
    let actual = i16::from(actual);
    actual >= minimum - tolerance && actual <= maximum + tolerance
}

#[cfg(test)]
fn interpolate_color(start: RgbaColor, end: RgbaColor, t: f64) -> RgbaColor {
    let lerp = |a: u8, b: u8| -> u8 {
        (f64::from(a) + (f64::from(b) - f64::from(a)) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    RgbaColor::new(
        lerp(start.red, end.red),
        lerp(start.green, end.green),
        lerp(start.blue, end.blue),
        lerp(start.alpha, end.alpha),
    )
}

fn content_bounds(image: &Image, background: RgbaColor, tolerance: u8) -> Option<Rect> {
    let mut min_x = image.width;
    let mut min_y = image.height;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut found = false;

    for y in 0..image.height {
        for x in 0..image.width {
            let pixel = image
                .pixel_at(x, y)
                .expect("iterated coordinates should stay within image bounds");

            if is_background_pixel(pixel, background, tolerance) {
                continue;
            }

            found = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }

    found.then(|| {
        Rect::new(
            Point::new(f64::from(min_x), f64::from(min_y)),
            Size::new(f64::from(max_x - min_x + 1), f64::from(max_y - min_y + 1)),
        )
    })
}

fn comparison_bounds(
    actual: &Image,
    expected: &Image,
    background: RgbaColor,
    tolerance: u8,
) -> Option<Rect> {
    union_bounds(
        content_bounds(actual, background, tolerance),
        content_bounds(expected, background, tolerance),
    )
}

fn is_background_pixel(pixel: [u8; 4], background: RgbaColor, tolerance: u8) -> bool {
    let tolerance = u16::from(tolerance);
    [background.red, background.green, background.blue, 255]
        .iter()
        .zip(pixel.iter())
        .all(|(expected, actual)| u16::from(*expected).abs_diff(u16::from(*actual)) <= tolerance)
}

fn union_bounds(left: Option<Rect>, right: Option<Rect>) -> Option<Rect> {
    match (left, right) {
        (Some(left), Some(right)) => {
            let min_x = left.origin.x.min(right.origin.x);
            let min_y = left.origin.y.min(right.origin.y);
            let max_x = (left.origin.x + left.size.width).max(right.origin.x + right.size.width);
            let max_y = (left.origin.y + left.size.height).max(right.origin.y + right.size.height);
            Some(Rect::new(
                Point::new(min_x, min_y),
                Size::new(max_x - min_x, max_y - min_y),
            ))
        }
        (Some(bounds), None) | (None, Some(bounds)) => Some(bounds),
        (None, None) => None,
    }
}

fn composite_channel(foreground: u8, background: u8, alpha: f64, inverse: f64) -> u8 {
    (f64::from(foreground) * alpha + f64::from(background) * inverse).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{load_png, Point, Size};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Debug)]
    struct StubError(&'static str);

    impl std::fmt::Display for StubError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::error::Error for StubError {}

    struct StubRenderer {
        actual: Image,
        expected: Image,
    }

    impl TextRenderer for StubRenderer {
        type Error = StubError;

        fn render_text_reference(&self, _: &TextExpectation) -> Result<Image, Self::Error> {
            Ok(self.expected.clone())
        }

        fn capture_text_region(&self, _: &TextExpectation) -> Result<Image, Self::Error> {
            Ok(self.actual.clone())
        }
    }

    fn rect() -> Rect {
        Rect::new(Point::new(2.0, 3.0), Size::new(20.0, 8.0))
    }

    fn image(value: u8) -> Image {
        Image::new(
            2,
            1,
            vec![value, value, value, 255, value, value, value, 255],
        )
    }

    fn gradient_background(width: u32, height: u32) -> Image {
        let top_left = RgbaColor::new(10, 20, 30, 255);
        let top_right = RgbaColor::new(40, 50, 60, 255);
        let bottom_left = RgbaColor::new(70, 80, 90, 255);
        let bottom_right = RgbaColor::new(100, 110, 120, 255);
        let width_denom = f64::from(width.saturating_sub(1).max(1));
        let height_denom = f64::from(height.saturating_sub(1).max(1));
        let mut data = Vec::with_capacity(width as usize * height as usize * 4);

        for y in 0..height {
            let vertical = f64::from(y) / height_denom;
            let left = interpolate_color(top_left, bottom_left, vertical);
            let right = interpolate_color(top_right, bottom_right, vertical);
            for x in 0..width {
                let horizontal = f64::from(x) / width_denom;
                let pixel = interpolate_color(left, right, horizontal);
                data.push(pixel.red);
                data.push(pixel.green);
                data.push(pixel.blue);
                data.push(255);
            }
        }

        Image::new(width, height, data)
    }

    fn transparent_reference(width: u32, height: u32) -> Image {
        Image::new(width, height, vec![0; width as usize * height as usize * 4])
    }

    #[test]
    fn expectation_builder_overrides_defaults() {
        let expectation = TextExpectation::new("Hello", rect())
            .with_font_family("SF Pro")
            .with_font_name("SFProText-Regular")
            .with_point_size(18.0)
            .with_weight(600)
            .italic(true)
            .with_foreground(RgbaColor::new(1, 2, 3, 255))
            .with_background(RgbaColor::new(4, 5, 6, 255));

        assert_eq!(expectation.content, "Hello");
        assert_eq!(expectation.font_family.as_deref(), Some("SF Pro"));
        assert_eq!(expectation.font_name.as_deref(), Some("SFProText-Regular"));
        assert_eq!(expectation.point_size, 18.0);
        assert_eq!(expectation.weight, Some(600));
        assert!(expectation.italic);
        assert_eq!(expectation.foreground, RgbaColor::new(1, 2, 3, 255));
        assert_eq!(expectation.background, Some(RgbaColor::new(4, 5, 6, 255)));
    }

    #[test]
    fn anchored_expectation_resolves_to_absolute_expectation() {
        let anchored = AnchoredTextExpectation::new(
            "Hello",
            crate::RegionSpec::root().subregion(crate::RelativeBounds::new(0.25, 0.5, 0.5, 0.25)),
        )
        .with_font_family("SF Pro")
        .with_point_size(18.0)
        .with_foreground(RgbaColor::new(1, 2, 3, 255));

        let resolved = anchored.resolve(rect());
        assert_eq!(resolved.content, "Hello");
        assert_eq!(resolved.rect, rect());
        assert_eq!(resolved.font_family.as_deref(), Some("SF Pro"));
        assert_eq!(resolved.point_size, 18.0);
        assert_eq!(resolved.foreground, RgbaColor::new(1, 2, 3, 255));
    }

    #[test]
    fn compare_rendered_text_uses_text_defaults() {
        let expectation = TextExpectation::new("Hello", rect());
        let result = compare_rendered_text(
            &image(0),
            &image(5),
            &expectation,
            &TextAssertionConfig::default(),
        );
        assert!(result.passed);
    }

    #[test]
    fn compare_rendered_text_composites_transparent_reference_with_sampled_background() {
        let actual = gradient_background(2, 2);
        let expected = transparent_reference(2, 2);
        let expectation = TextExpectation::new("Hi", rect());

        let result = compare_rendered_text(
            &actual,
            &expected,
            &expectation,
            &TextAssertionConfig::default(),
        );
        assert!(result.passed);
    }

    #[test]
    fn compare_rendered_text_allows_transparent_pixels_over_variable_background() {
        let actual = gradient_background(2, 2);
        let expected = transparent_reference(2, 2);
        let expectation = TextExpectation::new("Hi", rect());

        let result = compare_rendered_text(
            &actual,
            &expected,
            &expectation,
            &TextAssertionConfig::default(),
        );
        assert!(result.passed);
    }

    #[test]
    fn compare_rendered_text_allows_partial_alpha_edges_over_variable_background() {
        let background = gradient_background(3, 3);
        let mut expected = transparent_reference(3, 3);
        paint_rect(&mut expected.data, 3, 1, 1, 1, 1, [255, 255, 255, 128]);
        let actual = composite_over_background_image(&expected, &background);
        let expectation = TextExpectation::new("Hi", rect());

        let result = compare_rendered_text(
            &actual,
            &expected,
            &expectation,
            &TextAssertionConfig::default(),
        );
        assert!(result.passed);
    }

    #[test]
    fn compare_rendered_text_rejects_alpha_reference_when_pixel_is_too_dark() {
        let actual = Image::new(1, 1, vec![40, 40, 40, 255]);
        let expected = Image::new(1, 1, vec![255, 255, 255, 128]);
        let expectation = TextExpectation::new("Hi", rect());

        let result = compare_rendered_text(
            &actual,
            &expected,
            &expectation,
            &TextAssertionConfig {
                compare: CompareConfig {
                    channel_tolerance: 0,
                    match_threshold: 1.0,
                    generate_diff: true,
                },
                write_diff: true,
            },
        );
        assert!(!result.passed);
    }

    #[test]
    fn compare_rendered_text_allows_non_white_transparent_foreground_on_valid_backgrounds() {
        let background = gradient_background(3, 3);
        let mut expected = transparent_reference(3, 3);
        paint_rect(&mut expected.data, 3, 1, 1, 1, 1, [200, 60, 80, 128]);
        let actual = composite_over_background_image(&expected, &background);
        let expectation =
            TextExpectation::new("Hi", rect()).with_foreground(RgbaColor::new(200, 60, 80, 255));

        let result = compare_rendered_text(
            &actual,
            &expected,
            &expectation,
            &TextAssertionConfig::default(),
        );
        assert!(result.passed);
    }

    #[test]
    fn compare_rendered_text_rejects_alpha_reference_when_non_white_pixel_is_too_bright() {
        let background = gradient_background(3, 3);
        let mut expected = transparent_reference(3, 3);
        paint_rect(&mut expected.data, 3, 1, 1, 1, 1, [200, 60, 80, 128]);
        let mut actual = composite_over_background_image(&expected, &background);
        paint_rect(&mut actual.data, 3, 1, 1, 1, 1, [220, 200, 210, 255]);
        let expectation =
            TextExpectation::new("Hi", rect()).with_foreground(RgbaColor::new(200, 60, 80, 255));

        let result = compare_rendered_text(
            &actual,
            &expected,
            &expectation,
            &TextAssertionConfig {
                compare: CompareConfig {
                    channel_tolerance: 0,
                    match_threshold: 1.0,
                    generate_diff: true,
                },
                write_diff: true,
            },
        );
        assert!(!result.passed);
    }

    #[test]
    fn compare_rendered_text_rejects_alpha_reference_when_channel_mix_is_invalid() {
        let background = gradient_background(3, 3);
        let mut expected = transparent_reference(3, 3);
        paint_rect(&mut expected.data, 3, 1, 1, 1, 1, [255, 40, 40, 128]);
        let mut actual = composite_over_background_image(&expected, &background);
        paint_rect(&mut actual.data, 3, 1, 1, 1, 1, [120, 180, 120, 255]);
        let expectation =
            TextExpectation::new("Hi", rect()).with_foreground(RgbaColor::new(255, 40, 40, 255));

        let result = compare_rendered_text(
            &actual,
            &expected,
            &expectation,
            &TextAssertionConfig {
                compare: CompareConfig {
                    channel_tolerance: 0,
                    match_threshold: 1.0,
                    generate_diff: true,
                },
                write_diff: true,
            },
        );
        assert!(!result.passed);
    }

    #[test]
    fn compare_rendered_text_rejects_transparent_actual_alpha_even_when_rgb_matches() {
        let actual = Image::new(1, 1, vec![255, 255, 255, 0]);
        let expected = Image::new(1, 1, vec![255, 255, 255, 128]);
        let expectation = TextExpectation::new("Hi", rect());

        let result = compare_rendered_text(
            &actual,
            &expected,
            &expectation,
            &TextAssertionConfig::default(),
        );
        assert!(!result.passed);
    }

    #[test]
    fn compare_rendered_text_rejects_alpha_reference_with_invalid_rgba_buffers() {
        let actual = Image {
            width: 1,
            height: 1,
            data: vec![255, 255, 255],
        };
        let expected = Image::new(1, 1, vec![255, 255, 255, 0]);
        let expectation = TextExpectation::new("Hi", rect());

        let result = compare_rendered_text(
            &actual,
            &expected,
            &expectation,
            &TextAssertionConfig::default(),
        );
        assert!(!result.passed);
        assert_eq!(result.mismatched_pixels, u32::MAX);
    }

    #[test]
    fn compare_rendered_text_rejects_alpha_reference_region_size_changes() {
        let actual = Image::new(1, 1, vec![255, 255, 255, 255]);
        let expected = Image {
            width: 2,
            height: 1,
            data: vec![255, 255, 255, 0, 255, 255, 255, 255],
        };
        let expectation = TextExpectation::new("Hi", rect());

        let result = compare_rendered_text(
            &actual,
            &expected,
            &expectation,
            &TextAssertionConfig::default(),
        );
        assert!(!result.passed);
        assert_eq!(result.mismatched_pixels, u32::MAX);
    }

    #[test]
    fn compare_rendered_text_rejects_transparent_reference_content_displacement() {
        let actual = Image::new(
            4,
            1,
            vec![
                255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            ],
        );
        let expected = Image::new(
            4,
            1,
            vec![
                0, 0, 0, 255, 255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 255, 0,
            ],
        );
        let expectation = TextExpectation::new("Hi", rect());

        let result = compare_rendered_text(
            &actual,
            &expected,
            &expectation,
            &TextAssertionConfig {
                compare: CompareConfig {
                    channel_tolerance: 0,
                    match_threshold: 1.0,
                    generate_diff: true,
                },
                write_diff: true,
            },
        );
        assert!(!result.passed);
    }

    #[test]
    fn compare_rendered_text_allows_extra_content_in_fully_transparent_region() {
        let background = gradient_background(3, 3);
        let mut expected = transparent_reference(3, 3);
        paint_rect(&mut expected.data, 3, 1, 1, 1, 1, [255, 255, 255, 128]);
        let mut actual = composite_over_background_image(&expected, &background);
        paint_rect(&mut actual.data, 3, 0, 1, 1, 1, [0, 0, 0, 255]);
        let expectation = TextExpectation::new("Hi", rect());

        let result = compare_rendered_text(
            &actual,
            &expected,
            &expectation,
            &TextAssertionConfig {
                compare: CompareConfig {
                    channel_tolerance: 4,
                    match_threshold: 1.0,
                    generate_diff: true,
                },
                write_diff: true,
            },
        );
        assert!(result.passed);
    }

    #[test]
    fn compare_rendered_text_accepts_alpha_reference_at_tolerance_boundary() {
        let background = gradient_background(3, 3);
        let mut expected = transparent_reference(3, 3);
        paint_rect(&mut expected.data, 3, 1, 1, 1, 1, [0, 0, 0, 128]);
        let mut actual = composite_over_background_image(&expected, &background);
        let base = actual.pixel_at(1, 1).expect("center pixel should exist");
        paint_rect(
            &mut actual.data,
            3,
            1,
            1,
            1,
            1,
            [
                base[0].saturating_add(4),
                base[1].saturating_add(4),
                base[2].saturating_add(4),
                255,
            ],
        );
        let expectation =
            TextExpectation::new("Hi", rect()).with_foreground(RgbaColor::new(0, 0, 0, 255));

        let result = compare_rendered_text(
            &actual,
            &expected,
            &expectation,
            &TextAssertionConfig {
                compare: CompareConfig {
                    channel_tolerance: 4,
                    match_threshold: 1.0,
                    generate_diff: true,
                },
                write_diff: true,
            },
        );
        assert!(result.passed);
    }

    #[test]
    fn compare_rendered_text_rejects_alpha_reference_just_outside_tolerance_boundary() {
        let background = gradient_background(3, 3);
        let mut expected = transparent_reference(3, 3);
        paint_rect(&mut expected.data, 3, 1, 1, 1, 1, [0, 0, 0, 128]);
        let mut actual = composite_over_background_image(&expected, &background);
        paint_rect(&mut actual.data, 3, 1, 1, 1, 1, [140, 140, 140, 255]);
        let expectation =
            TextExpectation::new("Hi", rect()).with_foreground(RgbaColor::new(0, 0, 0, 255));

        let result = compare_rendered_text(
            &actual,
            &expected,
            &expectation,
            &TextAssertionConfig {
                compare: CompareConfig {
                    channel_tolerance: 4,
                    match_threshold: 1.0,
                    generate_diff: true,
                },
                write_diff: true,
            },
        );
        assert!(!result.passed);
    }

    #[test]
    fn compare_rendered_text_rejects_content_displacement() {
        let mut actual = vec![255; 20 * 20 * 4];
        let mut expected = vec![255; 20 * 20 * 4];
        for alpha in actual.iter_mut().skip(3).step_by(4) {
            *alpha = 255;
        }
        for alpha in expected.iter_mut().skip(3).step_by(4) {
            *alpha = 255;
        }

        paint_rect(&mut actual, 20, 10, 9, 2, 2, [0, 0, 0, 255]);
        paint_rect(&mut expected, 20, 9, 9, 2, 2, [0, 0, 0, 255]);

        let expectation = TextExpectation::new("Shifted", rect())
            .with_background(RgbaColor::new(255, 255, 255, 255));
        let result = compare_rendered_text(
            &Image::new(20, 20, actual),
            &Image::new(20, 20, expected),
            &expectation,
            &TextAssertionConfig::default(),
        );
        assert!(!result.passed);
        assert_eq!(result.mismatched_pixels, 4);
    }

    #[test]
    fn compare_rendered_text_ignores_blank_padding_around_matching_content() {
        let mut actual = vec![255; 20 * 20 * 4];
        let mut expected = vec![255; 20 * 20 * 4];
        for alpha in actual.iter_mut().skip(3).step_by(4) {
            *alpha = 255;
        }
        for alpha in expected.iter_mut().skip(3).step_by(4) {
            *alpha = 255;
        }

        paint_rect(&mut actual, 20, 10, 9, 2, 2, [0, 0, 0, 255]);
        paint_rect(&mut expected, 20, 10, 9, 2, 2, [0, 0, 0, 255]);

        let expectation = TextExpectation::new("Shifted", rect())
            .with_background(RgbaColor::new(255, 255, 255, 255));
        let result = compare_rendered_text(
            &Image::new(20, 20, actual),
            &Image::new(20, 20, expected),
            &expectation,
            &TextAssertionConfig::default(),
        );
        assert!(result.passed);
    }

    #[test]
    fn compare_rendered_text_rejects_region_size_changes_even_when_content_matches() {
        let actual = Image::new(
            4,
            4,
            vec![
                255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255,
                255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            ],
        );
        let expected = Image::new(
            6,
            6,
            vec![
                255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255,
                255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255, 0,
                0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            ],
        );
        let expectation = TextExpectation::new("Sized", rect())
            .with_background(RgbaColor::new(255, 255, 255, 255));

        let result = compare_rendered_text(
            &actual,
            &expected,
            &expectation,
            &TextAssertionConfig::default(),
        );
        assert!(!result.passed);
        assert_eq!(result.mismatched_pixels, u32::MAX);
    }

    #[test]
    fn assert_text_renders_passes_when_images_match() {
        let artifact_dir = unique_temp_dir();
        let expectation = TextExpectation::new("Hello", rect());
        assert_text_renders(
            &StubRenderer {
                actual: image(7),
                expected: image(7),
            },
            &expectation,
            &artifact_dir,
            &TextAssertionConfig::default(),
        )
        .expect("matching text render should pass");

        assert!(!artifact_dir.join("actual.png").exists());
        assert!(!artifact_dir.join("expected.png").exists());
        assert!(!artifact_dir.join("diff.png").exists());

        let _ = std::fs::remove_dir_all(artifact_dir);
    }

    #[test]
    fn assert_text_renders_passes_for_transparent_reference_on_variable_background() {
        let artifact_dir = unique_temp_dir();
        let expectation = TextExpectation::new("Hello", rect())
            .with_foreground(RgbaColor::new(255, 255, 255, 255));
        assert_text_renders(
            &StubRenderer {
                actual: Image::new(
                    2,
                    2,
                    vec![
                        10, 20, 30, 255, 120, 140, 160, 255, 255, 255, 255, 255, 40, 50, 60, 255,
                    ],
                ),
                expected: Image::new(
                    2,
                    2,
                    vec![
                        255, 255, 255, 0, 255, 255, 255, 96, 255, 255, 255, 255, 255, 255, 255, 0,
                    ],
                ),
            },
            &expectation,
            &artifact_dir,
            &TextAssertionConfig::default(),
        )
        .expect("transparent text render should pass over variable background");

        assert!(!artifact_dir.join("actual.png").exists());
        assert!(!artifact_dir.join("expected.png").exists());
        assert!(!artifact_dir.join("diff.png").exists());

        let _ = std::fs::remove_dir_all(artifact_dir);
    }

    #[test]
    fn assert_text_renders_reports_transparent_reference_mismatch() {
        let artifact_dir = unique_temp_dir();
        let expectation = TextExpectation::new("Hello", rect())
            .with_foreground(RgbaColor::new(255, 255, 255, 255));
        let error = assert_text_renders(
            &StubRenderer {
                actual: Image::new(1, 1, vec![20, 20, 20, 255]),
                expected: Image::new(1, 1, vec![255, 255, 255, 128]),
            },
            &expectation,
            &artifact_dir,
            &TextAssertionConfig {
                compare: CompareConfig {
                    channel_tolerance: 0,
                    match_threshold: 1.0,
                    generate_diff: true,
                },
                write_diff: true,
            },
        )
        .unwrap_err();

        match error {
            TextAssertionError::Mismatch {
                expectation: failed_expectation,
                artifacts,
                result,
            } => {
                assert_eq!(failed_expectation.content, "Hello");
                assert!(!result.passed);
                assert!(result.mismatched_pixels > 0);
                assert!(artifacts.actual_path.exists());
                assert!(artifacts.expected_path.exists());
                assert!(artifacts
                    .diff_path
                    .as_ref()
                    .is_some_and(|path| path.exists()));
            }
            other => panic!("expected mismatch error, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(artifact_dir);
    }

    #[test]
    fn assert_text_renders_omits_diff_when_disabled_for_transparent_reference_mismatch() {
        let artifact_dir = unique_temp_dir();
        let expectation = TextExpectation::new("Hello", rect())
            .with_foreground(RgbaColor::new(255, 255, 255, 255));
        let error = assert_text_renders(
            &StubRenderer {
                actual: Image::new(1, 1, vec![20, 20, 20, 255]),
                expected: Image::new(1, 1, vec![255, 255, 255, 128]),
            },
            &expectation,
            &artifact_dir,
            &TextAssertionConfig {
                compare: CompareConfig {
                    channel_tolerance: 0,
                    match_threshold: 1.0,
                    generate_diff: true,
                },
                write_diff: false,
            },
        )
        .unwrap_err();

        match error {
            TextAssertionError::Mismatch { artifacts, .. } => {
                assert!(artifacts.actual_path.exists());
                assert!(artifacts.expected_path.exists());
                assert!(artifacts.diff_path.is_none());
                assert!(!artifact_dir.join("diff.png").exists());
            }
            other => panic!("expected mismatch error, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(artifact_dir);
    }

    #[test]
    fn assert_text_renders_writes_expected_actual_and_diff_artifacts() {
        let artifact_dir = unique_temp_dir();
        let expectation = TextExpectation::new("Hello", rect());
        let error = assert_text_renders(
            &StubRenderer {
                actual: image(0),
                expected: image(255),
            },
            &expectation,
            &artifact_dir,
            &TextAssertionConfig::default(),
        )
        .unwrap_err();

        match error {
            TextAssertionError::Mismatch {
                expectation: failed_expectation,
                artifacts,
                result,
            } => {
                assert_eq!(failed_expectation.content, "Hello");
                assert!(!result.passed);
                assert_eq!(result.mismatched_pixels, 2);
                assert_eq!(result.matched_ratio, 0.0);
                let actual = load_png(&artifacts.actual_path).expect("actual artifact should load");
                let expected =
                    load_png(&artifacts.expected_path).expect("expected artifact should load");
                assert_eq!(actual.width, expected.width);
                assert_eq!(actual.height, expected.height);
                assert!(artifacts
                    .diff_path
                    .as_ref()
                    .is_some_and(|path| path.exists()));
                let diff_path = artifacts
                    .diff_path
                    .as_ref()
                    .expect("diff path should exist");
                let diff = load_png(diff_path).expect("diff artifact should load");
                assert_eq!(actual.width, diff.width);
                assert_eq!(actual.height, diff.height);
            }
            other => panic!("expected mismatch error, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(artifact_dir);
    }

    fn unique_temp_dir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "glasscheck-text-{}-{}-{}",
            std::process::id(),
            nanos,
            count
        ));
        std::fs::create_dir_all(&path).expect("temporary directory should be creatable");
        path
    }

    fn paint_rect(
        image: &mut [u8],
        width: usize,
        origin_x: usize,
        origin_y: usize,
        rect_width: usize,
        rect_height: usize,
        pixel: [u8; 4],
    ) {
        for y in origin_y..origin_y + rect_height {
            for x in origin_x..origin_x + rect_width {
                let base = (y * width + x) * 4;
                image[base..base + 4].copy_from_slice(&pixel);
            }
        }
    }
}
