#[derive(Clone, Debug, Default)]
/// Semantic metadata registered for a GTK widget exposed to query APIs.
pub struct InstrumentedWidget {
    pub id: Option<String>,
    pub role: Option<glasscheck_core::Role>,
    pub label: Option<String>,
}

#[derive(Debug, Default)]
/// Placeholder GTK window host until the Linux backend is implemented.
pub struct GtkWindowHost;
