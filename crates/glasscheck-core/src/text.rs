use std::io;
use std::path::{Path, PathBuf};

use crate::{compare_images, CompareConfig, CompareResult, Image, Rect, RegionSpec};

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
    /// Optional background color. When absent, the background is sampled.
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
    /// Optional background color. When absent, the background is sampled.
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
/// When no background color is provided, the background is inferred from the
/// border pixels of `actual` before compositing the reference image.
#[must_use]
pub fn compare_rendered_text(
    actual: &Image,
    expected: &Image,
    expectation: &TextExpectation,
    config: &TextAssertionConfig,
) -> CompareResult {
    let background = expectation
        .background
        .unwrap_or_else(|| sample_background_color(actual));
    let composited_expected = composite_over_background(expected, background);
    compare_images(actual, &composited_expected, &config.compare)
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

    let background = expectation
        .background
        .unwrap_or_else(|| sample_background_color(&actual));
    let composited_expected = composite_over_background(&expected, background);
    let result = compare_images(&actual, &composited_expected, &config.compare);
    if result.passed {
        return Ok(());
    }

    std::fs::create_dir_all(artifact_dir)?;
    let actual_path = artifact_dir.join("actual.png");
    crate::save_png(&actual, &actual_path)?;
    let expected_path = artifact_dir.join("expected.png");
    crate::save_png(&composited_expected, &expected_path)?;
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

fn sample_background_color(image: &Image) -> RgbaColor {
    let width = image.width as usize;
    let height = image.height as usize;
    if width == 0 || height == 0 {
        return RgbaColor::new(255, 255, 255, 255);
    }

    let mut sum = [0u64; 4];
    let mut count = 0u64;
    for y in 0..height {
        for x in 0..width {
            if x != 0 && y != 0 && x + 1 != width && y + 1 != height {
                continue;
            }
            if let Some(pixel) = image.pixel_at(x as u32, y as u32) {
                sum[0] += u64::from(pixel[0]);
                sum[1] += u64::from(pixel[1]);
                sum[2] += u64::from(pixel[2]);
                sum[3] += u64::from(pixel[3]);
                count += 1;
            }
        }
    }

    if count == 0 {
        return RgbaColor::new(255, 255, 255, 255);
    }

    RgbaColor::new(
        (sum[0] / count) as u8,
        (sum[1] / count) as u8,
        (sum[2] / count) as u8,
        (sum[3] / count) as u8,
    )
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

fn composite_channel(foreground: u8, background: u8, alpha: f64, inverse: f64) -> u8 {
    (f64::from(foreground) * alpha + f64::from(background) * inverse).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point, Size};
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
        let actual = Image::new(
            3,
            3,
            vec![
                255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0,
                0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                255, 255,
            ],
        );
        let expected = Image::new(
            3,
            3,
            vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        );
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
            TextAssertionError::Mismatch { artifacts, .. } => {
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
}
