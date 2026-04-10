#[cfg(target_os = "macos")]
mod imp {
    use functional_ui_core::{NodeMetadata, Point, QueryRoot, Rect, Role, Size};
    use objc2::rc::Retained;
    use objc2::MainThreadOnly;
    use objc2_app_kit::{NSBackingStoreType, NSView, NSWindow, NSWindowStyleMask};
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};
    use std::cell::RefCell;

    use crate::capture::capture_view_image;
    use crate::input::AppKitInputDriver;

    #[derive(Clone, Debug)]
    pub struct InstrumentedView {
        pub id: Option<String>,
        pub role: Option<Role>,
        pub label: Option<String>,
    }

    struct RegisteredView {
        view: Retained<NSView>,
        descriptor: InstrumentedView,
    }

    pub struct AppKitWindowHost {
        window: Retained<NSWindow>,
        registry: RefCell<Vec<RegisteredView>>,
    }

    impl AppKitWindowHost {
        #[must_use]
        pub fn new(mtm: MainThreadMarker, width: f64, height: f64) -> Self {
            let rect = NSRect::new(NSPoint::new(100.0, 100.0), NSSize::new(width, height));
            let style = NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Resizable;
            let window = unsafe {
                NSWindow::initWithContentRect_styleMask_backing_defer(
                    NSWindow::alloc(mtm),
                    rect,
                    style,
                    NSBackingStoreType::Buffered,
                    false,
                )
            };
            unsafe { window.setReleasedWhenClosed(false) };
            Self {
                window,
                registry: RefCell::new(Vec::new()),
            }
        }

        #[must_use]
        pub fn window(&self) -> &NSWindow {
            &self.window
        }

        pub fn set_content_view(&self, view: &NSView) {
            self.window.setContentView(Some(view));
        }

        #[must_use]
        pub fn capture(&self) -> Option<functional_ui_core::Image> {
            let content = self.window.contentView()?;
            capture_view_image(&content)
        }

        #[must_use]
        pub fn capture_view(&self, view: &NSView) -> Option<functional_ui_core::Image> {
            capture_view_image(view)
        }

        #[must_use]
        pub fn input(&self) -> AppKitInputDriver<'_> {
            AppKitInputDriver::new(&self.window)
        }

        pub fn register_view(&self, view: &NSView, descriptor: InstrumentedView) {
            let retained = unsafe {
                Retained::retain(view as *const NSView as *mut NSView)
                    .expect("registered view should retain successfully")
            };
            self.registry.borrow_mut().push(RegisteredView {
                view: retained,
                descriptor,
            });
        }

        #[must_use]
        pub fn query_root(&self) -> QueryRoot {
            let nodes = self
                .registry
                .borrow()
                .iter()
                .map(|entry| NodeMetadata {
                    id: entry.descriptor.id.clone(),
                    role: entry.descriptor.role.clone(),
                    label: entry.descriptor.label.clone(),
                    rect: rect_in_window(&entry.view, &self.window),
                })
                .collect();
            QueryRoot::new(nodes)
        }

        pub fn set_title(&self, title: &str) {
            let title = NSString::from_str(title);
            self.window.setTitle(&title);
        }
    }

    fn rect_in_window(view: &NSView, window: &NSWindow) -> Rect {
        let converted = if let Some(content_view) = window.contentView() {
            view.convertRect_toView(view.bounds(), Some(&*content_view))
        } else {
            view.frame()
        };

        Rect::new(
            Point::new(converted.origin.x, converted.origin.y),
            Size::new(converted.size.width, converted.size.height),
        )
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    #[derive(Clone, Debug)]
    pub struct InstrumentedView {
        pub id: Option<String>,
        pub role: Option<functional_ui_core::Role>,
        pub label: Option<String>,
    }

    pub struct AppKitWindowHost;
}

pub use imp::{AppKitWindowHost, InstrumentedView};
