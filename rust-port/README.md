# SimpleWindowManager Rust port

This is a parallel, non-destructive port. The original C/C++ sources, `makefile`,
and published binaries remain independent and are the behavioral reference.

```powershell
cargo test --manifest-path rust-port/Cargo.toml
cargo build --release --manifest-path rust-port/Cargo.toml
& .\rust-port\target\release\simple-window-manager-rs.exe
```

## UI and window-manager threads

The main thread owns the input hooks, bar windows, and menu orchestration.
`swm-window-manager` owns the runtime, window-event hooks, window positioning,
and the border and drop-preview windows. The picker library retains its own
existing presentation thread.

Keyboard hooks recognize compiled bindings and make the swallow/pass-through
decision immediately. Window-manager actions use a bounded, nonblocking queue;
menu requests use a separate UI queue. A full worker queue therefore does not
prevent opening the commands menu. New shortcut presses that cannot be queued
pass through; already swallowed key presses retain matching suppression of
repeats and key-up events.

The worker publishes immutable snapshots for bar painting and menu data. The UI
keeps the last snapshot if publication is in progress, and never reads the
runtime lock. Workspace/client information can remain stale while the worker is
blocked, but the bar can still repaint and menus can open. Window-manager actions
still wait for the worker; this split does not make Win32 positioning calls
asynchronous or give them a timeout.

Mouse releases carry their original screen coordinates and gesture identity.
An old release cannot complete a newer drag. If a release cannot be queued,
the worker cancels drag state when it resumes. Menu-close messages carry an
explicit session identity so late cleanup cannot close a newer menu. Confirmed
window, program, and command selections execute even if another menu has opened.
Selection cleanup does not request focus restoration; cancellation does.

For manual validation, temporarily block a test application's UI thread and
trigger a workspace operation involving it. While the worker is stalled, verify
that the bar repaints, right-click opens the commands menu, and typing in another
responsive application works. After recovery, check workspace changes, dragging,
empty workspaces, and closing/reopening menus. Also test quitting during the
stall: shutdown waits at most 500 ms for the worker before allowing process exit.

Worker command traces include `sequence`, `command`, and `queue_delay_us` to
distinguish queued work from time spent executing it.

## User configuration

The default build uses `rust-port/default_user_config.rs`, which contains the
Rust equivalent of the active C configuration. To keep configuration in
dotfiles, copy that file (or start from `rust-port/user_config.rs.example`) and
select it in a local wrapper settings file:

```powershell
Copy-Item rust-port\build.local.ps1.example rust-port\build.local.ps1
```

The ignored `rust-port/build.local.ps1` contains:

```powershell
$SwmConfigRs = 'G:\src\dotfiles\simplewindowmanager\config.rs'
```

Edit that path if the dotfiles checkout is elsewhere. The wrapper exports it as
`SWM_CONFIG_RS` for the Cargo process.

Use the PowerShell wrapper to print the resolved path and effective
configuration as normal output before building:

```powershell
.\rust-port\build.ps1
```

The wrapper builds release by default; pass `-Dev` for a debug build. An
additional `-Timings` switch generates Cargo's build timing report at
`rust-port/target/cargo-timings/cargo-timing.html`:

```powershell
.\rust-port\build.ps1 -Timings
```

An explicit `SWM_CONFIG_RS` environment variable takes precedence over
`build.local.ps1`. If the
selected file delegates to the repository defaults, the delegated
`configure()` implementation is expanded in the output. Direct Cargo commands
do not load `build.local.ps1`; they use `SWM_CONFIG_RS` when set and otherwise
fall back to `default_user_config.rs`.

Relative `SWM_CONFIG_RS` paths are resolved from `rust-port`. Cargo watches both
the environment variable and selected file, so changing either triggers a
rebuild. The selected file must define:

```rust
pub fn configure(configuration: &mut simple_window_manager_rs::Configuration) {
    // User configuration.
}
```

Like the C `configFile` workflow, configuration is compiled into the executable
and changes require rebuilding it.

Run those commands from the repository root. The manager requests elevation,
matching the active C build (`props.mk.local` sets `requireAdmin = TRUE`). Quit
the C manager before starting the Rust manager because both intentionally use
the same single-instance mutex.

The Rust menu implementation is linked directly from the sibling
`../../RustWin32Host` Cargo package, so neither `rust_nfm.dll` nor `LibNfm.dll`
is required. The C build remains independent.

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
compile-time Cargo dependency, including the float and client log menus.

The `restore_moved_windows` binary is the Rust counterpart of
`RestoreMovedWindows.exe`.

Release artifacts are written only beneath `rust-port/target`; the C build and
its source files are not modified or replaced.
