use std::io;
use std::path::{Path, PathBuf};

use crate::Image;

/// Configuration for pixel-by-pixel image comparison.
#[derive(Clone, Debug)]
pub struct CompareConfig {
    /// Maximum allowed absolute per-channel difference for a pixel to match.
    pub channel_tolerance: u8,
    /// Minimum fraction of matching pixels required to pass.
    pub match_threshold: f64,
    /// Whether to generate a red/green diff image in the result.
    pub generate_diff: bool,
}

impl Default for CompareConfig {
    fn default() -> Self {
        Self {
            channel_tolerance: 4,
            match_threshold: 0.99,
            generate_diff: true,
        }
    }
}

/// Result of comparing two images.
#[derive(Clone, Debug)]
pub struct CompareResult {
    /// Fraction of pixels that matched within tolerance.
    pub matched_ratio: f64,
    /// Number of pixels that did not match.
    pub mismatched_pixels: u32,
    /// Whether the comparison satisfied the configured threshold.
    pub passed: bool,
    /// Optional red/green diff image for mismatches.
    pub diff_image: Option<Image>,
}

/// Configuration for asserting an image against a stored baseline.
#[derive(Clone, Debug)]
pub struct SnapshotConfig {
    /// Pixel comparison settings.
    pub compare: CompareConfig,
    /// Whether to write a diff artifact when the assertion fails.
    pub write_diff: bool,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            compare: CompareConfig::default(),
            write_diff: true,
        }
    }
}

/// Paths to artifacts emitted by a failed snapshot assertion.
#[derive(Clone, Debug, Default)]
pub struct SnapshotArtifacts {
    /// Path to the captured actual image.
    pub actual_path: PathBuf,
    /// Path to the generated diff image, when available.
    pub diff_path: Option<PathBuf>,
}

/// Errors returned by snapshot assertions.
#[derive(Debug)]
pub enum SnapshotError {
    /// Filesystem I/O failed.
    Io(io::Error),
    /// The requested baseline image does not exist.
    MissingBaseline(PathBuf),
    /// The actual image did not match the baseline.
    Mismatch {
        baseline: PathBuf,
        artifacts: SnapshotArtifacts,
        result: CompareResult,
    },
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::MissingBaseline(path) => write!(f, "missing baseline at {}", path.display()),
            Self::Mismatch {
                baseline,
                artifacts,
                result,
            } => write!(
                f,
                "snapshot mismatch against {}: {:.2}% match, actual at {}",
                baseline.display(),
                result.matched_ratio * 100.0,
                artifacts.actual_path.display()
            ),
        }
    }
}

impl std::error::Error for SnapshotError {}

impl From<io::Error> for SnapshotError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// Compares two RGBA images using the supplied tolerance and threshold settings.
#[must_use]
pub fn compare_images(actual: &Image, expected: &Image, config: &CompareConfig) -> CompareResult {
    if !actual.is_valid_rgba() || !expected.is_valid_rgba() {
        return CompareResult {
            matched_ratio: 0.0,
            mismatched_pixels: u32::MAX,
            passed: false,
            diff_image: None,
        };
    }

    if actual.width != expected.width || actual.height != expected.height {
        return CompareResult {
            matched_ratio: 0.0,
            mismatched_pixels: u32::MAX,
            passed: false,
            diff_image: None,
        };
    }

    let total_pixels = u64::from(actual.width) * u64::from(actual.height);
    if total_pixels == 0 {
        return CompareResult {
            matched_ratio: 1.0,
            mismatched_pixels: 0,
            passed: true,
            diff_image: None,
        };
    }

    let tolerance = u16::from(config.channel_tolerance);
    let mut mismatched = 0u64;
    let mut diff = config
        .generate_diff
        .then(|| Vec::with_capacity(actual.data.len()));

    for (actual, expected) in actual
        .data
        .chunks_exact(4)
        .zip(expected.data.chunks_exact(4))
    {
        let is_match = actual
            .iter()
            .zip(expected.iter())
            .all(|(left, right)| u16::from(*left).abs_diff(u16::from(*right)) <= tolerance);

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

/// Writes an image to `path` as a PNG file.
pub fn save_png(image: &Image, path: &Path) -> io::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(io::BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(io::Error::other)?;
    writer
        .write_image_data(&image.data)
        .map_err(io::Error::other)
}

/// Loads a PNG file and normalizes it to RGBA8 pixel data.
pub fn load_png(path: &Path) -> io::Result<Image> {
    let file = std::fs::File::open(path)?;
    let decoder = png::Decoder::new(io::BufReader::new(file));
    let mut decoder = decoder;
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut data = vec![0u8; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut data)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    data.truncate(info.buffer_size());
    Ok(decode_png_to_rgba(info, data)?)
}

/// Asserts that `actual` matches the PNG baseline at `baseline_path`.
///
/// On failure, writes artifacts into `artifact_dir` and returns the mismatch
/// details together with the artifact paths.
pub fn assert_snapshot_matches(
    actual: &Image,
    baseline_path: &Path,
    artifact_dir: &Path,
    config: &SnapshotConfig,
) -> Result<(), SnapshotError> {
    if !baseline_path.exists() {
        return Err(SnapshotError::MissingBaseline(baseline_path.to_path_buf()));
    }

    std::fs::create_dir_all(artifact_dir)?;
    let baseline = load_png(baseline_path)?;
    let result = compare_images(actual, &baseline, &config.compare);
    if result.passed {
        return Ok(());
    }

    let actual_path = artifact_dir.join("actual.png");
    save_png(actual, &actual_path)?;

    let diff_path = if config.write_diff {
        let path = artifact_dir.join("diff.png");
        if let Some(image) = result.diff_image.as_ref() {
            save_png(image, &path)?;
            Some(path)
        } else {
            None
        }
    } else {
        None
    };

    Err(SnapshotError::Mismatch {
        baseline: baseline_path.to_path_buf(),
        artifacts: SnapshotArtifacts {
            actual_path,
            diff_path,
        },
        result,
    })
}

fn decode_png_to_rgba(info: png::OutputInfo, data: Vec<u8>) -> io::Result<Image> {
    let rgba = match info.color_type {
        png::ColorType::Rgba => data,
        png::ColorType::Rgb => {
            let mut rgba = Vec::with_capacity(info.width as usize * info.height as usize * 4);
            for chunk in data.chunks_exact(3) {
                rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
            rgba
        }
        png::ColorType::Grayscale => {
            let mut rgba = Vec::with_capacity(info.width as usize * info.height as usize * 4);
            for &value in &data {
                rgba.extend_from_slice(&[value, value, value, 255]);
            }
            rgba
        }
        png::ColorType::GrayscaleAlpha => {
            let mut rgba = Vec::with_capacity(info.width as usize * info.height as usize * 4);
            for chunk in data.chunks_exact(2) {
                rgba.extend_from_slice(&[chunk[0], chunk[0], chunk[0], chunk[1]]);
            }
            rgba
        }
        png::ColorType::Indexed => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "PNG palette data was not expanded to an RGB or RGBA image",
            ));
        }
    };

    if rgba.len() != info.width as usize * info.height as usize * 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "decoded PNG did not normalize to RGBA8",
        ));
    }

    Ok(Image::new(info.width, info.height, rgba))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::BufWriter;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn image(value: u8) -> Image {
        Image::new(
            2,
            1,
            vec![value, value, value, 255, value, value, value, 255],
        )
    }

    #[test]
    fn compare_detects_mismatch() {
        let result = compare_images(&image(0), &image(255), &CompareConfig::default());
        assert!(!result.passed);
        assert_eq!(result.mismatched_pixels, 2);
    }

    #[test]
    fn compare_passes_for_exact_match() {
        let result = compare_images(&image(42), &image(42), &CompareConfig::default());
        assert!(result.passed);
        assert_eq!(result.mismatched_pixels, 0);
        assert_eq!(result.matched_ratio, 1.0);
        assert!(result.diff_image.is_none());
    }

    #[test]
    fn compare_rejects_malformed_images() {
        let valid = image(0);
        let malformed = Image {
            width: 1,
            height: 1,
            data: vec![0, 0, 0, 255, 1, 2, 3, 4],
        };

        let result = compare_images(&valid, &malformed, &CompareConfig::default());
        assert!(!result.passed);
        assert_eq!(result.mismatched_pixels, u32::MAX);
        assert!(result.diff_image.is_none());
    }

    #[test]
    fn compare_rejects_too_short_images() {
        let valid = image(0);
        let malformed = Image {
            width: 2,
            height: 1,
            data: vec![0, 0, 0, 255],
        };

        let result = compare_images(&malformed, &valid, &CompareConfig::default());
        assert!(!result.passed);
        assert_eq!(result.mismatched_pixels, u32::MAX);
        assert!(result.diff_image.is_none());
    }

    #[test]
    fn load_png_normalizes_grayscale_to_rgba() {
        let path = write_png(png::ColorType::Grayscale, png::BitDepth::Eight, &[64]);
        let image = load_png(&path).expect("grayscale PNG should decode");
        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.data, vec![64, 64, 64, 255]);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn load_png_normalizes_rgb_to_rgba() {
        let path = write_png(png::ColorType::Rgb, png::BitDepth::Eight, &[10, 20, 30]);
        let image = load_png(&path).expect("rgb PNG should decode");
        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.data, vec![10, 20, 30, 255]);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn assert_snapshot_matches_reports_visual_regression_and_writes_artifacts() {
        let baseline_path = unique_temp_png_path();
        save_png(&image(0), &baseline_path).expect("baseline PNG should be writable");

        let artifact_dir = unique_temp_artifact_dir("snapshot-regression");
        let error = assert_snapshot_matches(
            &image(255),
            &baseline_path,
            &artifact_dir,
            &SnapshotConfig {
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
            SnapshotError::Mismatch {
                baseline,
                artifacts,
                result,
            } => {
                assert_eq!(baseline, baseline_path);
                assert!(!result.passed);
                assert_eq!(result.mismatched_pixels, 2);
                assert_eq!(result.matched_ratio, 0.0);
                assert!(artifacts.actual_path.exists());
                assert!(artifacts
                    .diff_path
                    .as_ref()
                    .is_some_and(|path| path.exists()));
            }
            other => panic!("expected mismatch error, got {other:?}"),
        }

        let _ = std::fs::remove_file(baseline_path);
        let _ = std::fs::remove_dir_all(artifact_dir);
    }

    #[test]
    fn assert_snapshot_matches_passes_for_matching_baseline() {
        let baseline_path = unique_temp_png_path();
        save_png(&image(64), &baseline_path).expect("baseline PNG should be writable");

        let artifact_dir = unique_temp_artifact_dir("snapshot-match");
        assert_snapshot_matches(
            &image(64),
            &baseline_path,
            &artifact_dir,
            &SnapshotConfig::default(),
        )
        .expect("matching baseline should pass");

        assert!(!artifact_dir.join("actual.png").exists());
        assert!(!artifact_dir.join("diff.png").exists());

        let _ = std::fs::remove_file(baseline_path);
        let _ = std::fs::remove_dir_all(artifact_dir);
    }

    fn write_png(
        color_type: png::ColorType,
        bit_depth: png::BitDepth,
        data: &[u8],
    ) -> std::path::PathBuf {
        let path = unique_temp_png_path();
        let file = File::create(&path).expect("temporary PNG file should be creatable");
        let writer = BufWriter::new(file);
        let mut encoder = png::Encoder::new(writer, 1, 1);
        encoder.set_color(color_type);
        encoder.set_depth(bit_depth);
        let mut writer = encoder
            .write_header()
            .expect("PNG header should be writable");
        writer
            .write_image_data(data)
            .expect("PNG image data should be writable");
        path
    }

    fn unique_temp_png_path() -> std::path::PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "glasscheck-{}-{}-{}.png",
            std::process::id(),
            nanos,
            count
        ))
    }

    fn unique_temp_artifact_dir(prefix: &str) -> std::path::PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "glasscheck-{prefix}-{}-{}-{}",
            std::process::id(),
            nanos,
            count
        ));
        std::fs::create_dir_all(&path).expect("temporary directory should be creatable");
        path
    }
}
