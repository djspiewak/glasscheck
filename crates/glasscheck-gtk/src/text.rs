use glasscheck_core::TextAssertionError;

#[derive(Debug)]
/// Errors returned by the GTK text harness placeholder.
pub enum GtkTextError {}

impl std::fmt::Display for GtkTextError {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {}
    }
}

impl std::error::Error for GtkTextError {}

#[derive(Debug)]
/// Errors returned by anchored GTK text assertions.
pub enum GtkAnchoredTextError {
    /// The underlying text assertion failed.
    Assert(TextAssertionError<GtkTextError>),
}

impl std::fmt::Display for GtkAnchoredTextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Assert(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for GtkAnchoredTextError {}

#[derive(Clone, Copy, Debug, Default)]
/// Placeholder GTK text harness until the Linux backend is implemented.
pub struct GtkTextHarness;
