## Commit Hygiene

- Run formatting on every affected file before staging and committing.

## Backend Testing

- GTK backend tests that rely on `xvfb-run` or X11 must be run outside the Codex sandbox. The sandbox blocks the X server setup and socket access needed by GTK.
- This restriction is GTK-specific and does not apply to the AppKit backend.

## Verification Commands

- Linux/GTK real X11 display: `env GDK_BACKEND=x11 cargo test -p glasscheck-gtk --test gtk_smoke --features native-smoke-tests`
- Linux/GTK real X11 display: `env GDK_BACKEND=x11 cargo test -p glasscheck-gtk --test gtk_contracts --features native-contract-tests`
- Linux/GTK hidden X11 via `xvfb-run`: `env GDK_BACKEND=x11 xvfb-run -a cargo test -p glasscheck-gtk --test gtk_smoke --features native-smoke-tests`
- Linux/GTK hidden X11 via `xvfb-run`: `env GDK_BACKEND=x11 xvfb-run -a cargo test -p glasscheck-gtk --test gtk_contracts --features native-contract-tests`
- macOS/AppKit: `cargo test -p glasscheck-appkit --test appkit_smoke --features native-smoke-tests`
- macOS/AppKit: `cargo test -p glasscheck-appkit --test appkit_contracts --features native-contract-tests`

## Linux GTK Notes

- GTK on Linux now has two supported verification modes:
  - real X11 display for validating live desktop behavior, including that test windows do not flash on screen
  - hidden X11 via `xvfb-run` for CI-style or headless verification
- Force `GDK_BACKEND=x11` in both modes. Plain `xvfb-run` can still select a non-X11 backend in some environments.
- `xvfb-run` and X11 access must still be run outside the Codex sandbox. This remains GTK-specific and does not affect AppKit.
