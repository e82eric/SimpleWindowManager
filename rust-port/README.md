# SimpleWindowManager Rust port

This is a parallel, non-destructive port. The original C/C++ sources, `makefile`,
and published binaries remain independent and are the behavioral reference.

```powershell
cargo test --manifest-path rust-port/Cargo.toml
cargo build --release --manifest-path rust-port/Cargo.toml
& .\rust-port\target\release\simple-window-manager-rs.exe
```

Run those commands from the repository root. The manager requests elevation,
matching the active C build (`props.mk.local` sets `requireAdmin = TRUE`). Quit
the C manager before starting the Rust manager because both intentionally use
the same single-instance mutex.

The Rust menu implementation is linked directly from the sibling
`../../RustWin32Host` Cargo package, so `rust_nfm.dll` is no longer required.
`LibNfm.dll` and its companion DLLs remain optional beside the executable for
the legacy diagnostic column menus and legacy fallback. The C build remains
independent.

The standalone restore helper is built at
`rust-port\target\release\restore_moved_windows.exe`. Pass `--NoRestore` to
preview which off-screen windows and hidden taskbars it would restore.

The port intentionally targets Windows with the MSVC Rust toolchain. Its modules
retain the terminology and control flow of the C implementation while Win32 and
callback unsafety is isolated in `platform`.

## Implemented surface

The Rust executable includes the native Win32 event and input hooks, live
window routing, all layouts, workspace/monitor movement, taskbar handling,
bars and status segments, focus border, drag/drop and resize handling,
minimize-to-hide cloaking, command/process/service/window/program/file menus,
the diagnostic buffers, and the single-instance lifecycle. The Rust menu is a
compile-time Cargo dependency; `LibNfm.dll` is only used for legacy menu
features that do not yet have a Rust crate API.

The `restore_moved_windows` binary is the Rust counterpart of
`RestoreMovedWindows.exe`.

Release artifacts are written only beneath `rust-port/target`; the C build and
its source files are not modified or replaced.
