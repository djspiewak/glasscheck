use crate::{Point, Rect, Size};

/// An RGBA8 image used for capture and assertion workflows.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Pixel data in row-major RGBA8 order.
    pub data: Vec<u8>,
}

impl Image {
    /// Creates an RGBA8 image, panicking if the buffer length is invalid.
    #[must_use]
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Self {
        assert_eq!(
            data.len(),
            width as usize * height as usize * 4,
            "RGBA image data must contain exactly width * height * 4 bytes"
        );
        Self {
            width,
            height,
            data,
        }
    }

    /// Returns `true` when the backing buffer exactly matches RGBA8 dimensions.
    #[must_use]
    pub fn is_valid_rgba(&self) -> bool {
        self.data.len() == self.width as usize * self.height as usize * 4
    }

    /// Returns the pixel at `(x, y)` if it lies inside the image bounds.
    #[must_use]
    pub fn pixel_at(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let base = (y as usize)
            .checked_mul(self.width as usize)?
            .checked_add(x as usize)?
            .checked_mul(4)?;

        Some([
            *self.data.get(base)?,
            *self.data.get(base + 1)?,
            *self.data.get(base + 2)?,
            *self.data.get(base + 3)?,
        ])
    }

    /// Crops the image to `rect`, clamping out-of-bounds coordinates.
    #[must_use]
    pub fn crop(&self, rect: Rect) -> Self {
        let start_x = rect.origin.x.max(0.0).floor() as u32;
        let start_y = rect.origin.y.max(0.0).floor() as u32;
        let end_x = (rect.origin.x + rect.size.width).max(0.0).ceil() as u32;
        let end_y = (rect.origin.y + rect.size.height).max(0.0).ceil() as u32;

        let bounded_end_x = end_x.min(self.width);
        let bounded_end_y = end_y.min(self.height);
        let bounded_start_x = start_x.min(bounded_end_x);
        let bounded_start_y = start_y.min(bounded_end_y);
        let width = bounded_end_x - bounded_start_x;
        let height = bounded_end_y - bounded_start_y;

        let mut data = Vec::with_capacity(width as usize * height as usize * 4);
        for y in bounded_start_y..bounded_end_y {
            for x in bounded_start_x..bounded_end_x {
                if let Some(pixel) = self.pixel_at(x, y) {
                    data.extend_from_slice(&pixel);
                }
            }
        }

        Self::new(width, height, data)
    }

    /// Computes the average RGBA value over `rect`.
    #[must_use]
    pub fn average_rgba(&self, rect: Rect) -> [f64; 4] {
        let region = self.crop(rect);
        if region.width == 0 || region.height == 0 {
            return [0.0; 4];
        }

        let mut sum = [0.0; 4];
        let mut count = 0.0;
        for chunk in region.data.chunks_exact(4) {
            sum[0] += f64::from(chunk[0]);
            sum[1] += f64::from(chunk[1]);
            sum[2] += f64::from(chunk[2]);
            sum[3] += f64::from(chunk[3]);
            count += 1.0;
        }

        [
            sum[0] / count,
            sum[1] / count,
            sum[2] / count,
            sum[3] / count,
        ]
    }

    /// Returns the fraction of pixels whose luminance exceeds `threshold`.
    #[must_use]
    pub fn bright_pixel_fraction(&self, threshold: f64) -> f64 {
        if self.width == 0 || self.height == 0 {
            return 0.0;
        }

        let bright = self
            .data
            .chunks_exact(4)
            .filter(|chunk| {
                let luminance = (0.299 * f64::from(chunk[0])
                    + 0.587 * f64::from(chunk[1])
                    + 0.114 * f64::from(chunk[2]))
                    / 255.0;
                luminance >= threshold
            })
            .count();

        bright as f64 / f64::from(self.width * self.height)
    }

    /// Returns the image size in view-coordinate form.
    #[must_use]
    pub fn size(&self) -> Size {
        Size::new(f64::from(self.width), f64::from(self.height))
    }

    /// Returns the center point of the image.
    #[must_use]
    pub fn center(&self) -> Point {
        Point::new(f64::from(self.width) / 2.0, f64::from(self.height) / 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_image() -> Image {
        Image::new(
            2,
            2,
            vec![0, 0, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255],
        )
    }

    #[test]
    fn crop_bounds_are_clamped() {
        let cropped = sample_image().crop(Rect::new(Point::new(-1.0, -1.0), Size::new(3.0, 3.0)));
        assert_eq!(cropped.width, 2);
        assert_eq!(cropped.height, 2);
    }

    #[test]
    fn average_rgba_uses_selected_region() {
        let avg = sample_image().average_rgba(Rect::new(Point::new(0.0, 0.0), Size::new(1.0, 1.0)));
        assert_eq!(avg, [0.0, 0.0, 0.0, 255.0]);
    }

    #[test]
    fn bright_fraction_counts_luminance() {
        let fraction = sample_image().bright_pixel_fraction(0.1);
        assert!(fraction > 0.5);
    }

    #[test]
    fn is_valid_rgba_reports_exact_length() {
        assert!(sample_image().is_valid_rgba());
        let malformed = Image {
            width: 1,
            height: 1,
            data: vec![1, 2, 3],
        };
        assert!(!malformed.is_valid_rgba());
    }

    #[test]
    #[should_panic(expected = "RGBA image data must contain exactly width * height * 4 bytes")]
    fn new_rejects_incorrect_data_length() {
        let _ = Image::new(1, 1, vec![0, 0, 0]);
    }
}
