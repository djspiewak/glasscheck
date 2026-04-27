//! GTK4 backend for in-process testing of native Linux UIs.
//!
//! Use this crate when tests can run inside the GTK main context and need scene
//! snapshots, capture, direct widget-level interaction, native dialogs, or
//! popover-backed context menus. On X11-backed windows, pointer input uses
//! native X11 dispatch while keyboard and text input still follow GTK controller
//! and text APIs. Other GTK backends report input unavailability rather than
//! silently degrading.
//!
//! `GtkSession` implements the shared dialog API for `MessageDialog`,
//! `FileChooserDialog`, and generic `Dialog` surfaces. `GtkDialogController`
//! can register metadata-only async dialogs that have no widget surface; those
//! controllers support kind/title matching but report live operations as
//! unsupported capabilities. `GtkContextMenu` models visible `Popover` menus
//! and can activate button-backed menu items discovered in the widget tree.

mod dialog;
mod harness;
mod input;
mod menu;
mod screen;
mod session;
mod text;
mod window;

pub use dialog::GtkDialogController;
pub use glasscheck_core::InstrumentedNode as InstrumentedWidget;
pub use glasscheck_core::{HitPointSearch, HitPointStrategy, InstrumentedNode};
pub use harness::GtkHarness;
pub use input::GtkInputDriver;
pub use menu::{GtkContextMenu, GtkContextMenuError};
pub use session::GtkSession;
pub use text::{GtkAnchoredTextError, GtkTextError, GtkTextHarness};
pub use window::{GtkSceneSource, GtkSnapshotContext, GtkWindowHost};
