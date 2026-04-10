use std::io;
use std::path::{Path, PathBuf};

use crate::{compare_images, CompareConfig, CompareResult, Image, Rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RgbaColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl RgbaColor {
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
pub struct TextExpectation {
    pub content: String,
    pub rect: Rect,
    pub font_family: Option<String>,
    pub font_name: Option<String>,
    pub point_size: f64,
    pub weight: Option<u16>,
    pub italic: bool,
    pub foreground: RgbaColor,
    pub background: Option<RgbaColor>,
}

impl TextExpectation {
    #[must_use]
    pub fn new(content: impl Into<String>, rect: Rect) -> Self {
        Self {
            content: content.into(),
            rect,
            font_family: None,
            font_name: None,
            point_size: 14.0,
            weight: None,
            italic: false,
            foreground: RgbaColor::new(0, 0, 0, 255),
            background: None,
        }
    }

    #[must_use]
    pub fn with_font_family(mut self, family: impl Into<String>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    #[must_use]
    pub fn with_font_name(mut self, name: impl Into<String>) -> Self {
        self.font_name = Some(name.into());
        self
    }

    #[must_use]
    pub fn with_point_size(mut self, point_size: f64) -> Self {
        self.point_size = point_size;
        self
    }

    #[must_use]
    pub fn with_weight(mut self, weight: u16) -> Self {
        self.weight = Some(weight);
        self
    }

    #[must_use]
    pub fn italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }

    #[must_use]
    pub fn with_foreground(mut self, foreground: RgbaColor) -> Self {
        self.foreground = foreground;
        self
    }

    #[must_use]
    pub fn with_background(mut self, background: RgbaColor) -> Self {
        self.background = Some(background);
        self
    }
}

#[derive(Clone, Debug)]
pub struct TextAssertionConfig {
    pub compare: CompareConfig,
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

#[derive(Clone, Debug, Default)]
pub struct TextAssertionArtifacts {
    pub actual_path: PathBuf,
    pub expected_path: PathBuf,
    pub diff_path: Option<PathBuf>,
}

pub trait TextRenderer {
    type Error;

    fn render_text_reference(&self, expectation: &TextExpectation) -> Result<Image, Self::Error>;

    fn capture_text_region(&self, expectation: &TextExpectation) -> Result<Image, Self::Error>;
}

#[derive(Debug)]
pub enum TextAssertionError<E> {
    Io(io::Error),
    Render(E),
    Capture(E),
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
        data.push(composite_channel(pixel[1], background.green, alpha, inverse));
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
        Image::new(2, 1, vec![value, value, value, 255, value, value, value, 255])
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
                255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                255, 255, 255,
            ],
        );
        let expected = Image::new(
            3,
            3,
            vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
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
                assert!(artifacts.diff_path.as_ref().is_some_and(|path| path.exists()));
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
            "functional-ui-text-{}-{}-{}",
            std::process::id(),
            nanos,
            count
        ));
        std::fs::create_dir_all(&path).expect("temporary directory should be creatable");
        path
    }
}
