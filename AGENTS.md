## Commit Hygiene

- Run formatting on every affected file before staging and committing.

## Backend Testing

- GTK backend tests that rely on `xvfb-run` or X11 must be run outside the Codex sandbox. The sandbox blocks the X server setup and socket access needed by GTK.
- This restriction is GTK-specific and does not apply to the AppKit backend.

## Verification Commands

- Linux/GTK: `xvfb-run -a cargo test -p glasscheck-gtk --test gtk_smoke --features native-smoke-tests`
- Linux/GTK: `xvfb-run -a cargo test -p glasscheck-gtk --test gtk_contracts --features native-contract-tests`
- macOS/AppKit: `cargo test -p glasscheck-appkit --test appkit_smoke --features native-smoke-tests`
- macOS/AppKit: `cargo test -p glasscheck-appkit --test appkit_contracts --features native-contract-tests`
