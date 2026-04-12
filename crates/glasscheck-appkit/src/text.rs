#[cfg(target_os = "macos")]
mod imp {
    use glasscheck_core::{
        assert_anchored_text_renders, assert_text_renders, font_expectation_has_conflict,
        AnchoredTextAssertionError, AnchoredTextExpectation, AnchoredTextHarness, Image, Rect,
        TextAssertionConfig, TextAssertionError, TextExpectation, TextRenderer,
    };
    use objc2::rc::Retained;
    use objc2::MainThreadOnly;
    use objc2_app_kit::{
        NSBackingStoreType, NSColor, NSFont, NSFontManager, NSFontTraitMask, NSTextAlignment,
        NSTextView, NSView, NSWindow, NSWindowStyleMask,
    };
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRange, NSRect, NSSize, NSString};
    use std::path::Path;

    use crate::capture::{capture_view_image, crop_image_in_view_coordinates};
    use crate::window::AppKitWindowHost;

    /// Errors returned by the AppKit text harness.
    #[derive(Debug)]
    pub enum AppKitTextError {
        /// Capturing pixels from AppKit failed.
        CaptureFailed,
        /// A specific font name was combined with family/weight/italic options.
        ConflictingFontOptions {
            font_name: String,
            font_family: Option<String>,
            weight: Option<u16>,
            italic: bool,
        },
        /// The requested font could not be resolved by AppKit.
        FontUnavailable(String),
        /// The reference window did not produce a content view.
        WindowContentMissing,
    }

    impl std::fmt::Display for AppKitTextError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::CaptureFailed => write!(f, "capture failed"),
                Self::ConflictingFontOptions {
                    font_name,
                    font_family,
                    weight,
                    italic,
                } => write!(
                    f,
                    "font_name={font_name:?} cannot be combined with family={font_family:?}, weight={weight:?}, or italic={italic}"
                ),
                Self::FontUnavailable(font) => write!(f, "font unavailable: {font}"),
                Self::WindowContentMissing => write!(f, "window content view is missing"),
            }
        }
    }

    impl std::error::Error for AppKitTextError {}

    /// Errors returned by anchored AppKit text assertions.
    #[derive(Debug)]
    pub type AppKitAnchoredTextError = AnchoredTextAssertionError<AppKitTextError>;

    /// AppKit implementation of the `TextRenderer` trait.
    pub struct AppKitTextHarness<'a> {
        host: &'a AppKitWindowHost,
        mtm: MainThreadMarker,
    }

    impl<'a> AppKitTextHarness<'a> {
        /// Creates a text harness backed by `host`.
        #[must_use]
        pub fn new(host: &'a AppKitWindowHost, mtm: MainThreadMarker) -> Self {
            Self { host, mtm }
        }

        /// Resolves an anchored expectation and asserts it against the live AppKit view.
        pub fn assert_text_renders_anchored(
            &self,
            expectation: &AnchoredTextExpectation,
            artifact_dir: &Path,
            config: &TextAssertionConfig,
        ) -> Result<(), AppKitAnchoredTextError> {
            assert_anchored_text_renders(
                self,
                |region| self.host.resolve_region(region),
                expectation,
                artifact_dir,
                config,
            )
        }
    }

    impl TextRenderer for AppKitTextHarness<'_> {
        type Error = AppKitTextError;

        fn render_text_reference(
            &self,
            expectation: &TextExpectation,
        ) -> Result<Image, Self::Error> {
            let window = make_reference_window(self.mtm, expectation)?;
            let content_view = window
                .contentView()
                .ok_or(AppKitTextError::WindowContentMissing)?;
            let image = capture_view_image(&content_view).ok_or(AppKitTextError::CaptureFailed)?;
            Ok(crop_in_view_coordinates(&image, expectation.rect))
        }

        fn capture_text_region(&self, expectation: &TextExpectation) -> Result<Image, Self::Error> {
            let actual = self.host.capture().ok_or(AppKitTextError::CaptureFailed)?;
            Ok(crop_in_view_coordinates(&actual, expectation.rect))
        }
    }

    impl AnchoredTextHarness for AppKitTextHarness<'_> {
        fn assert_text_renders_anchored(
            &self,
            expectation: &AnchoredTextExpectation,
            artifact_dir: &Path,
            config: &TextAssertionConfig,
        ) -> Result<(), AnchoredTextAssertionError<Self::Error>> {
            Self::assert_text_renders_anchored(self, expectation, artifact_dir, config)
        }
    }

    fn make_reference_window(
        mtm: MainThreadMarker,
        expectation: &TextExpectation,
    ) -> Result<Retained<NSWindow>, AppKitTextError> {
        validate_font_expectation(expectation)?;
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                scene_frame(expectation.rect),
                NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Resizable,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe { window.setReleasedWhenClosed(false) };
        let scene = NSView::initWithFrame(NSView::alloc(mtm), scene_frame(expectation.rect));
        let text_view = make_reference_text_view(mtm, expectation)?;
        scene.addSubview(&text_view);
        window.setContentView(Some(&scene));
        Ok(window)
    }

    fn make_reference_text_view(
        mtm: MainThreadMarker,
        expectation: &TextExpectation,
    ) -> Result<Retained<NSTextView>, AppKitTextError> {
        let view = NSTextView::initWithFrame(NSTextView::alloc(mtm), text_frame(expectation.rect));
        view.setDrawsBackground(expectation.background.is_some());
        if let Some(background) = expectation.background {
            let background = color(background);
            view.setBackgroundColor(&background);
        }
        view.setEditable(false);
        view.setSelectable(false);
        let string = NSString::from_str(&expectation.content);
        view.setString(&string);
        if let Some(text_container) = unsafe { view.textContainer() } {
            text_container.setLineFragmentPadding(0.0);
        }
        view.setTextContainerInset(NSSize::new(0.0, 0.0));
        let font = font(mtm, expectation)?;
        view.setFont(Some(&font));
        let text_color = color(expectation.foreground);
        view.setTextColor(Some(&text_color));
        let range = NSRange::new(0, utf16_len(&expectation.content));
        view.setAlignment_range(NSTextAlignment::Left, range);
        Ok(view)
    }

    fn scene_frame(rect: Rect) -> NSRect {
        let origin_x = rect.origin.x.max(0.0);
        let origin_y = rect.origin.y.max(0.0);
        let width = (origin_x + rect.size.width).max(rect.size.width).max(1.0);
        let height = (origin_y + rect.size.height).max(rect.size.height).max(1.0);
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height))
    }

    fn text_frame(rect: Rect) -> NSRect {
        NSRect::new(
            NSPoint::new(rect.origin.x, rect.origin.y),
            NSSize::new(rect.size.width.max(1.0), rect.size.height.max(1.0)),
        )
    }

    fn validate_font_expectation(expectation: &TextExpectation) -> Result<(), AppKitTextError> {
        if font_expectation_has_conflict(expectation) {
            return Err(AppKitTextError::ConflictingFontOptions {
                font_name: expectation.font_name.clone().unwrap_or_default(),
                font_family: expectation.font_family.clone(),
                weight: expectation.weight,
                italic: expectation.italic,
            });
        }

        Ok(())
    }

    fn font(
        mtm: MainThreadMarker,
        expectation: &TextExpectation,
    ) -> Result<Retained<NSFont>, AppKitTextError> {
        validate_font_expectation(expectation)?;
        if let Some(name) = expectation.font_name.as_deref() {
            let name = NSString::from_str(name);
            return NSFont::fontWithName_size(&name, expectation.point_size)
                .ok_or_else(|| AppKitTextError::FontUnavailable(name.to_string()));
        }

        if let Some(family) = expectation.font_family.as_deref() {
            let manager = NSFontManager::sharedFontManager(mtm);
            let family = NSString::from_str(family);
            let traits = font_traits(expectation);
            let weight = font_manager_weight(expectation.weight);
            return manager
                .fontWithFamily_traits_weight_size(&family, traits, weight, expectation.point_size)
                .ok_or_else(|| AppKitTextError::FontUnavailable(family.to_string()));
        }

        let mut font = if let Some(weight) = expectation.weight {
            NSFont::systemFontOfSize_weight(expectation.point_size, system_font_weight(weight))
        } else {
            NSFont::systemFontOfSize(expectation.point_size)
        };
        if expectation.italic {
            let manager = NSFontManager::sharedFontManager(mtm);
            font = manager.convertFont_toHaveTrait(&font, NSFontTraitMask::ItalicFontMask);
        }
        Ok(font)
    }

    fn color(color: glasscheck_core::RgbaColor) -> Retained<NSColor> {
        NSColor::colorWithSRGBRed_green_blue_alpha(
            f64::from(color.red) / 255.0,
            f64::from(color.green) / 255.0,
            f64::from(color.blue) / 255.0,
            f64::from(color.alpha) / 255.0,
        )
    }

    fn font_traits(expectation: &TextExpectation) -> NSFontTraitMask {
        let mut traits = NSFontTraitMask::empty();
        if expectation.italic {
            traits |= NSFontTraitMask::ItalicFontMask;
        }
        if expectation.weight.is_some_and(|weight| weight >= 600) {
            traits |= NSFontTraitMask::BoldFontMask;
        }
        traits
    }

    fn font_manager_weight(weight: Option<u16>) -> isize {
        weight
            .map(|weight| ((weight.clamp(100, 900) - 100) / 100) as isize)
            .unwrap_or(5)
    }

    fn system_font_weight(weight: u16) -> f64 {
        (((weight.clamp(100, 900) as f64) - 400.0) / 500.0).clamp(-1.0, 1.0)
    }

    fn utf16_len(text: &str) -> usize {
        text.encode_utf16().count()
    }

    fn crop_in_view_coordinates(image: &Image, rect: Rect) -> Image {
        crop_image_in_view_coordinates(image, rect)
    }

    #[cfg(test)]
    mod tests {
        use super::{
            crop_in_view_coordinates, font_manager_weight, font_traits, text_frame, utf16_len,
            validate_font_expectation, AppKitTextError,
        };
        use glasscheck_core::{Image, Point, Rect, Size, TextExpectation};
        use objc2_app_kit::NSFontTraitMask;
        use objc2_foundation::NSPoint;

        #[test]
        fn utf16_len_counts_non_bmp_scalar_values_as_two_code_units() {
            assert_eq!(utf16_len("a"), 1);
            assert_eq!(utf16_len("😀"), 2);
            assert_eq!(utf16_len("a😀b"), 4);
        }

        #[test]
        fn font_name_conflicts_are_rejected() {
            let expectation = TextExpectation::new(
                "Hello",
                Rect::new(Point::new(0.0, 0.0), Size::new(10.0, 10.0)),
            )
            .with_font_name("SFProText-Regular")
            .with_weight(700)
            .italic(true);

            let error = validate_font_expectation(&expectation).unwrap_err();
            assert!(matches!(
                error,
                AppKitTextError::ConflictingFontOptions { .. }
            ));
        }

        #[test]
        fn crop_in_view_coordinates_uses_bottom_left_origin() {
            let image = Image::new(
                2,
                2,
                vec![
                    10, 10, 10, 255, 20, 20, 20, 255, 30, 30, 30, 255, 40, 40, 40, 255,
                ],
            );
            let cropped = crop_in_view_coordinates(
                &image,
                Rect::new(Point::new(0.0, 0.0), Size::new(1.0, 1.0)),
            );
            assert_eq!(cropped.data, vec![30, 30, 30, 255]);
        }

        #[test]
        fn crop_in_view_coordinates_clamps_negative_origin_to_visible_region() {
            let image = Image::new(
                3,
                3,
                vec![
                    10, 10, 10, 255, 20, 20, 20, 255, 30, 30, 30, 255, 40, 40, 40, 255, 50, 50, 50,
                    255, 60, 60, 60, 255, 70, 70, 70, 255, 80, 80, 80, 255, 90, 90, 90, 255,
                ],
            );
            let cropped = crop_in_view_coordinates(
                &image,
                Rect::new(Point::new(-1.0, -1.0), Size::new(2.0, 2.0)),
            );
            assert_eq!(cropped.width, 1);
            assert_eq!(cropped.height, 1);
            assert_eq!(cropped.data, vec![70, 70, 70, 255]);
        }

        #[test]
        fn text_frame_preserves_negative_origin() {
            let frame = text_frame(Rect::new(Point::new(-20.0, -12.0), Size::new(80.0, 80.0)));
            assert_eq!(frame.origin, NSPoint::new(-20.0, -12.0));
            assert_eq!(frame.size.width, 80.0);
            assert_eq!(frame.size.height, 80.0);
        }

        #[test]
        fn font_traits_include_bold_and_italic_flags() {
            let expectation = TextExpectation::new(
                "Hello",
                Rect::new(Point::new(0.0, 0.0), Size::new(10.0, 10.0)),
            )
            .with_font_family("Helvetica")
            .with_weight(700)
            .italic(true);

            let traits = font_traits(&expectation);
            assert!(traits.contains(NSFontTraitMask::BoldFontMask));
            assert!(traits.contains(NSFontTraitMask::ItalicFontMask));
        }

        #[test]
        fn font_manager_weight_clamps_css_weight_range() {
            assert_eq!(font_manager_weight(Some(50)), 0);
            assert_eq!(font_manager_weight(Some(100)), 0);
            assert_eq!(font_manager_weight(Some(700)), 6);
            assert_eq!(font_manager_weight(Some(950)), 8);
            assert_eq!(font_manager_weight(None), 5);
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    #[derive(Debug)]
    pub enum AppKitTextError {}

    #[derive(Debug)]
    pub enum AppKitAnchoredTextError {}

    pub struct AppKitTextHarness<'a> {
        _marker: std::marker::PhantomData<&'a ()>,
    }
}

pub use imp::{AppKitAnchoredTextError, AppKitTextError, AppKitTextHarness};
