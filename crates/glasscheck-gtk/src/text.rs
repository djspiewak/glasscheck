#[cfg(target_os = "linux")]
mod imp {
    use std::path::Path;

    use glasscheck_core::{
        assert_anchored_text_renders, font_expectation_has_conflict, AnchoredTextAssertionError,
        AnchoredTextExpectation, AnchoredTextHarness, Image, TextAssertionConfig, TextExpectation,
        TextRenderer,
    };
    use gtk4::glib;
    use gtk4::prelude::*;

    use crate::screen::present_window_offscreen;
    use crate::window::{capture_widget_image, GtkWindowHost};

    /// Errors returned by the GTK text harness.
    #[derive(Debug)]
    pub enum GtkTextError {
        /// Capturing pixels from GTK failed.
        CaptureFailed,
        /// A specific font name was combined with family/weight/italic options.
        ConflictingFontOptions {
            font_name: String,
            font_family: Option<String>,
            weight: Option<u16>,
            italic: bool,
        },
    }

    impl std::fmt::Display for GtkTextError {
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
            }
        }
    }

    impl std::error::Error for GtkTextError {}

    /// Errors returned by anchored GTK text assertions.
    pub type GtkAnchoredTextError = AnchoredTextAssertionError<GtkTextError>;

    /// GTK implementation of the `TextRenderer` trait.
    pub struct GtkTextHarness<'a> {
        host: &'a GtkWindowHost,
    }

    impl<'a> GtkTextHarness<'a> {
        /// Creates a text harness backed by `host`.
        #[must_use]
        pub fn new(host: &'a GtkWindowHost) -> Self {
            Self { host }
        }

        /// Resolves an anchored expectation and asserts it against the live GTK widget tree.
        pub fn assert_text_renders_anchored(
            &self,
            expectation: &AnchoredTextExpectation,
            artifact_dir: &Path,
            config: &TextAssertionConfig,
        ) -> Result<(), GtkAnchoredTextError> {
            assert_anchored_text_renders(
                self,
                |region| self.host.resolve_region(region),
                expectation,
                artifact_dir,
                config,
            )
        }
    }

    impl TextRenderer for GtkTextHarness<'_> {
        type Error = GtkTextError;

        fn render_text_reference(
            &self,
            expectation: &TextExpectation,
        ) -> Result<Image, Self::Error> {
            validate_font_expectation(expectation)?;

            let scene_size = scene_size(expectation);
            let window = gtk4::Window::builder()
                .default_width(scene_size.0)
                .default_height(scene_size.1)
                .build();
            let fixed = gtk4::Fixed::new();
            fixed.set_widget_name("glasscheck-reference-root");
            fixed.set_size_request(scene_size.0, scene_size.1);
            let text_view = make_reference_text_view(expectation)?;
            fixed.put(
                &text_view,
                expectation.rect.origin.x,
                f64::from(scene_size.1) - expectation.rect.origin.y - expectation.rect.size.height,
            );
            window.set_child(Some(&fixed));
            install_reference_css(&window, expectation)?;
            present_window_offscreen(&window);
            flush_main_context();

            let image =
                capture_widget_image(fixed.upcast_ref()).ok_or(GtkTextError::CaptureFailed)?;
            Ok(glasscheck_core::crop_image_bottom_left(
                &image,
                expectation.rect,
            ))
        }

        fn capture_text_region(&self, expectation: &TextExpectation) -> Result<Image, Self::Error> {
            let actual = self.host.capture().ok_or(GtkTextError::CaptureFailed)?;
            Ok(glasscheck_core::crop_image_bottom_left(
                &actual,
                expectation.rect,
            ))
        }
    }

    impl AnchoredTextHarness for GtkTextHarness<'_> {
        fn assert_text_renders_anchored(
            &self,
            expectation: &AnchoredTextExpectation,
            artifact_dir: &Path,
            config: &TextAssertionConfig,
        ) -> Result<(), AnchoredTextAssertionError<Self::Error>> {
            Self::assert_text_renders_anchored(self, expectation, artifact_dir, config)
        }
    }

    fn make_reference_text_view(
        expectation: &TextExpectation,
    ) -> Result<gtk4::TextView, GtkTextError> {
        let text_view = gtk4::TextView::new();
        text_view.set_widget_name("glasscheck-reference-text");
        text_view.set_size_request(
            expectation.rect.size.width.round().max(1.0) as i32,
            expectation.rect.size.height.round().max(1.0) as i32,
        );
        text_view.set_editable(false);
        text_view.set_cursor_visible(false);
        text_view.set_wrap_mode(gtk4::WrapMode::None);
        text_view.set_left_margin(0);
        text_view.set_right_margin(0);
        text_view.set_top_margin(0);
        text_view.set_bottom_margin(0);
        let buffer = text_view.buffer();
        buffer.set_text(&expectation.content);
        Ok(text_view)
    }

    fn install_reference_css(
        window: &gtk4::Window,
        expectation: &TextExpectation,
    ) -> Result<(), GtkTextError> {
        let provider = gtk4::CssProvider::new();
        provider.load_from_data(&reference_css(expectation));
        let display = gtk4::prelude::WidgetExt::display(window);
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        Ok(())
    }

    fn reference_css(expectation: &TextExpectation) -> String {
        let family = expectation
            .font_name
            .clone()
            .or_else(|| expectation.font_family.clone())
            .unwrap_or_else(|| "Sans".into());
        let weight = expectation.weight.unwrap_or(400);
        let style = if expectation.italic {
            "italic"
        } else {
            "normal"
        };
        let foreground = rgba(expectation.foreground);
        let background = expectation
            .background
            .map(rgba)
            .unwrap_or_else(|| "rgba(255,255,255,0.0)".into());
        format!(
            "#glasscheck-reference-root {{ background-color: {background}; }}
             #glasscheck-reference-text {{
                 background-color: {background};
                 color: {foreground};
                 font-family: \"{family}\";
                 font-size: {}pt;
                 font-style: {style};
                 font-weight: {weight};
                 padding: 0;
             }}",
            expectation.point_size
        )
    }

    fn rgba(color: glasscheck_core::RgbaColor) -> String {
        format!(
            "rgba({}, {}, {}, {:.3})",
            color.red,
            color.green,
            color.blue,
            f64::from(color.alpha) / 255.0
        )
    }

    fn validate_font_expectation(expectation: &TextExpectation) -> Result<(), GtkTextError> {
        if font_expectation_has_conflict(expectation) {
            return Err(GtkTextError::ConflictingFontOptions {
                font_name: expectation.font_name.clone().unwrap_or_default(),
                font_family: expectation.font_family.clone(),
                weight: expectation.weight,
                italic: expectation.italic,
            });
        }
        Ok(())
    }

    fn scene_size(expectation: &TextExpectation) -> (i32, i32) {
        let origin_x = expectation.rect.origin.x.max(0.0);
        let origin_y = expectation.rect.origin.y.max(0.0);
        let width = (origin_x + expectation.rect.size.width)
            .max(expectation.rect.size.width)
            .max(1.0)
            .round() as i32;
        let height = (origin_y + expectation.rect.size.height)
            .max(expectation.rect.size.height)
            .max(1.0)
            .round() as i32;
        (width, height)
    }

    fn flush_main_context() {
        let context = glib::MainContext::default();
        while context.pending() {
            context.iteration(false);
        }
        context.iteration(false);
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    #[derive(Debug)]
    pub enum GtkTextError {}

    pub type GtkAnchoredTextError = glasscheck_core::AnchoredTextAssertionError<GtkTextError>;

    pub struct GtkTextHarness<'a> {
        _marker: std::marker::PhantomData<&'a ()>,
    }
}

pub use imp::{GtkAnchoredTextError, GtkTextError, GtkTextHarness};
