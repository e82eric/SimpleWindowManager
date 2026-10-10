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
and the border and drop-preview windows. All menus use a SimpleWindowManager-owned egui tool window and OpenGL context on the dedicated `swm-egui-window-menu` thread. Its window/context persist while hidden. SWM no longer starts NFM's Skia picker runtime or its presentation thread.

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

## egui window switcher

The existing `ShowWindows` binding now opens `nfm-egui` (the default configuration
uses Alt+Space). SimpleWindowManager creates and owns the Win32 popup, OpenGL
context and Win32 message loop directly. Windows keyboard, text/IME, pointer and
clipboard input is translated into egui events; neither eframe nor winit is used. NFM provides window
enumeration, immutable HWND payloads, background fuzzy search and the picker UI.
The popup is a tool window, so it is excluded from normal window management and
the picker itself. Opening refreshes the candidate list; closing retains the GL
context for the next opening. Startup creates the popup without showing it or
taking focus. Window/process/filesystem sources are refreshed when their menus
open.

Type to filter, use Up/Down or Ctrl+N to navigate, and Enter/double-click to switch
to a window. Escape, Alt+F4 or losing focus dismisses the menu. Ctrl+C copies the
selected row. Selection uses the existing worker window command; cancellation
uses the existing session-checked focus-restoration path. Opening another menu
silently hides this one. NFM owns enumeration, picker interactions, and live DWM thumbnails for the selected window.

Build the Rust port and restart your running Rust window manager to try the
updated binding. The C implementation remains independent.

The native smoke example exercises the host without installing hooks or starting
a second window manager. It briefly opens the popup and sends input only to that
window, checking the selected HWND, Escape, reopen, hide and shutdown. It requires
an interactive desktop with at least one application window:

```powershell
cargo run --manifest-path rust-port/Cargo.toml --example egui_window_menu_smoke --locked
```
## User configuration

Picker sizes can be overridden in your `configure()` function:

```rust
configuration.picker_result_rows = 7;
configuration.picker_preview_rows = 14;
```

These are the defaults for window, file, program, process, and command pickers.
Result counts are clamped to 1–100 and preview counts to 1–256; available screen
space can reduce the visible counts. Rebuild and restart after changing config.

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
.\rust-port\build.ps1 -GhosttyRoot C:\src\ghostty2
```

ANSI previews require the standalone Ghostty VT library. In the Ghostty checkout,
build it once with `zig build -Demit-lib-vt -Doptimize=ReleaseFast
-Dvt-features=-kitty-graphics --prefix zig-out/nfm-vt` (as one command).
The shared VT preview build currently disables Kitty graphics.
The wrapper currently defaults to `C:\src\ghostty2`; pass `-GhosttyRoot` to
override it. Direct Cargo builds use `GHOSTTY_ROOT` instead.
Explicit `NFM_GHOSTTY_VT_INCLUDE_DIR` and
`NFM_GHOSTTY_VT_LIB_DIR` override the derived paths.

File previews default to bat's `gruvbox-dark` theme. Set
`configuration.file_preview_bat_theme = Some("other-theme".into())` in your Rust
configuration to override it, or set it to `None` to use bat's own settings.

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

The Rust menus use shared crates from [npicker](https://github.com/e82eric/npicker),
pinned to `de012392133660e8678c54afadefaadf1ec4c1b3` in Cargo.toml.
`nfm-win32` supplies Windows enumeration, process
actions, drive roots, and typed snapshots. `nfm-preview` supplies file previews
through its optional egui adapter, and `nfm-file-system` owns filesystem options
and scanning. SWM does not depend on `nfm-rust-win32host` or compile its legacy
picker runtime, view model, DLL entry points, or Skia menu renderer.
Neither `rust_nfm.dll` nor `LibNfm.dll` is required. The bar also uses egui and
OpenGL; SWM has no Skia dependency. Each bar owns a persistent OpenGL 3.3 context
and font atlas on its Win32 window thread. Its existing pixel layout, workspace
click targets, 1500 ms status timer, and non-activating tool window remain intact.
Configured Windows fonts are loaded through GDI, with installed JetBrains Nerd
Fonts and egui's default fonts as fallbacks. Text and shadows may differ slightly
from the former Skia renderer.
The C build remains independent.

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
the diagnostic buffers, and the single-instance lifecycle. NFM's picker control,
sources, and preview helpers are compile-time Cargo dependencies.

Services, float logs, client logs, error logs, and configuration state use a shared
structured egui picker. Existing log columns and error/configuration detail previews
are retained. Services expose State and Service columns. Structured `/` filters,
sorting, display selection, copying, and keybinding help use the shared picker.
Enter closes these read-only menus; Escape, Alt+F4, and focus loss dismiss them.
Closing restores focus through the same session-checked path as other menus.

Selecting another menu from Commands keeps the native popup visible and focused.
All menu types reuse the picker's `replace_source` API through an adapter that
preserves typed payloads and each source's search behavior. Previous rows remain
visible while the replacement source loads, with stale selections blocked.
The last complete frame remains visible while the
main thread resolves the command. Ordinary commands and dismissal still close
the popup. Replacement resets the query and suppresses the accepting Enter event.

The `restore_moved_windows` binary is the Rust counterpart of
`RestoreMovedWindows.exe`.

Release artifacts are written only beneath `rust-port/target`; the C build and
its source files are not modified or replaced.

## egui filesystem menu

`ShowFileSystem` uses the same persistent, undecorated OpenGL window as the window
switcher. NFM streams filesystem snapshots into the shared egui picker while
scanning continues. The initial scan retains the existing five-level limit;
entering a directory searches its subtree. Closing or navigating cancels the old
scan and pending preview work.

Enter/double-click enters a directory or opens a selected file using the existing
program action. Ctrl+U goes to the parent (or back to the drive roots). Ctrl+P
toggles previews, Ctrl+PageUp/PageDown scrolls them, Ctrl+C copies the path, and
Escape dismisses the menu.

Previews reuse NFM's Skia file-menu pipeline: `bat --color=always` for colored
text, `ffmpeg` for images, and `ffprobe` plus `ffmpeg` for a frame at 25% of a
video's duration. Directory and other-file listings use the same native resolver.
These tools must be on PATH, as with the Skia menu. Text falls back to the bounded
built-in preview when the command provider fails. Images preserve their aspect
ratio and fit the preview area. Completed previews are cached during scrolling;
replacement previews retain the previous display until ready.
The native smoke example also checks filesystem typing and exact file selection
with previews enabled, without capturing the desktop.
## egui programs menu

Both `ShowPrograms(false)` and `ShowPrograms(true)` use the persistent egui host.
Each uses its configured launcher directories, streaming file-only results with
the existing five-level scan limit. An empty directory list searches the current
working directory, matching the previous menu. Previews are hidden initially,
keeping the compact desktop layout. Selection preserves the existing normal or
elevated program action; cancellation restores focus through the menu session.
The native smoke test verifies the selected path and elevation flag for both
launchers without executing the selected program.
## egui process menu

`ShowProcesses` uses the shared egui host and NFM's typed process snapshots.
The Name, PID, WorkingSet, PrivateBytes and CPU header follows the structured
query, with the existing structured filtering and Tab completion. Ctrl+P toggles
the details preview, initially hidden as in the Skia menu. Ctrl+R refreshes while
preserving the query; Ctrl+K terminates the selected PID and refreshes the list.
Enumeration and actions run on background workers, retaining the displayed rows
and preview during refresh. Errors and successful termination are shown in the
menu. Enter and Escape close it through the existing focus-restoration path.

Tests cover header rendering, structured PID filtering, preview toggling,
same-count CPU updates, refresh and termination of a disposable test process.
The native smoke test opens and dismisses the process menu without desktop capture.
The process query UI includes the Skia-style `/` completion menu. Use Up/Down
and Enter or Tab to build filters, sorts and display expressions; Escape dismisses
suggestions first. For example, `/:CPU>=10 /!Descending==CPU` filters and sorts,
and `/#Display==Name,PID,CPU` changes the header and displayed columns.
The commands menu now uses the owned egui host with the original Name, Type, KeyBinding, and Description columns. Structured `/` filters, sorting, and display selection remain available; accepting a row dispatches its configured command name.

Filesystem keyboard parity includes Ctrl+U (parent / drives), Ctrl+P (preview), Ctrl+Page Up/Down (preview scrolling), Ctrl+C (selected path), Ctrl+Shift+C (plain preview text), and Ctrl+Shift+/ (scrollable keybinding help). Escape dismisses help before closing the menu. Query cursor movement, selection, deletion, and paste use the egui text editor.

Command sources are built synchronously from the small in-memory command list on each opening. Existing sessions retain their own immutable snapshots.

Filesystem directory transitions retain the same egui control and its presented rows/preview while the replacement scan begins. Old selections are disabled until replacement results arrive; incomplete empty snapshots do not clear the display. Completed empty directories clear the retained content.

The native egui host disables OpenGL swap-interval waiting, matching Ghostty. Opening frames discard stale input but still render the picker, avoiding a deliberately blank frame. Repaint requests wake the Win32 message loop directly, including while hidden; there is no hidden-window sleep. A separate native popup still requires window visibility and focus transitions. The host never registers raw input, attaches input queues, or injects modifier events into Windows. SWM's global keyboard hook remains unchanged.

The egui windows picker displays a live DWM thumbnail of the selected window. Ctrl+P or Ctrl+Space toggles preview. Thumbnails preserve aspect ratio, follow DPI and layout changes, and are released when hidden, help opens, the selected window disappears, or the menu closes. SWM owns DWM handles; the shared NFM control only supplies preview bounds.

The egui workflows use NFM's `SharedPicker` for cross-type source replacement.
SWM no longer maintains its own menu item enum, source mapper, or native-source
adapters. NFM preserves transitions and supplies previews and workflow actions;
SWM handles native hosting, presentation, and selection outcomes.

### Picker monitor

All pickers (windows, processes, programs, files, commands, services, and data/log
menus) share `Configuration.picker_monitor`. In your Rust configuration:

```rust
use crate::config::PickerMonitor;
configuration.picker_monitor = PickerMonitor::Fixed(1); // Second SWM monitor
// Or follow SWM's focused monitor whenever a picker opens:
// configuration.picker_monitor = PickerMonitor::Focused;
```

The default is `Fixed(0)`. Indices refer to SWM's zero-based monitor list, not
Windows display numbers. Hidden, missing, or unusable targets fall back to the
first visible monitor. Rebuild and restart SWM after changing the Rust config.
