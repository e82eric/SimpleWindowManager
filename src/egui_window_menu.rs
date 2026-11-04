//! SimpleWindowManager owns this native tool window, event loop and OpenGL renderer.
//! NFM supplies only enumeration, search and the embedded picker control.
use crate::picker::{Outcome, PickerHost, PickerRequest, PickerView};
use nfm_egui::{Appearance, Placement, egui};
use std::sync::{Arc, Mutex, mpsc};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

// Desktop presentation belongs to this host. NFM's compact defaults and
// caller-provided bounds remain available to terminal hosts such as Ghostty.
const PICKER_WIDTH: f32 = 1600.0;
const PICKER_MARGIN: f32 = 8.0;
pub(crate) const RESULT_ROWS: usize = 7;
pub(crate) const PREVIEW_ROWS: usize = 14;

pub(crate) fn desktop_appearance() -> Appearance {
    let mut appearance = Appearance::default();
    appearance.typography.normal = egui::FontId::monospace(15.0);
    appearance.typography.bold = egui::FontId::monospace(15.0);
    appearance.typography.row_height = 30.0;
    appearance.typography.cell_width = 9.0;
    appearance
}

#[derive(Clone, Copy)]
pub struct ScreenBounds {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

type Completion = Arc<dyn Fn(u64, Outcome) + Send + Sync>;
enum Command {
    Show {
        session: u64,
        bounds: ScreenBounds,
        request: Box<dyn PickerRequest>,
    },
    Hide,
    Focus,
    Quit,
}

/// One persistent context; reopening a menu does not recreate the GPU resources.
pub struct WindowMenuHost {
    sender: mpsc::Sender<Command>,
    context: Arc<Mutex<Option<egui::Context>>>,
    last_session: Arc<Mutex<Option<u64>>>,
}
impl WindowMenuHost {
    pub fn start(completed: impl Fn(u64, Outcome) + Send + Sync + 'static) -> Result<Self, String> {
        Self::start_with_rows(RESULT_ROWS, PREVIEW_ROWS, completed)
    }
    pub fn start_with_rows(
        result_rows: usize,
        preview_rows: usize,
        completed: impl Fn(u64, Outcome) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        Self::start_with_preview_resolver(
            result_rows,
            preview_rows,
            crate::file_preview::resolver(Default::default()),
            completed,
        )
    }
    pub fn start_with_preview_resolver(
        result_rows: usize,
        preview_rows: usize,
        file_preview_resolver: crate::file_preview::FilePreviewResolver,
        completed: impl Fn(u64, Outcome) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        let result_rows = result_rows.clamp(1, 100);
        let preview_rows = preview_rows.clamp(1, 256);
        let (sender, receiver) = mpsc::channel();
        let (ready, initialized) = mpsc::sync_channel(1);
        let context = Arc::new(Mutex::new(None));
        let thread_context = context.clone();
        let last_session = Arc::new(Mutex::new(None));
        let thread_session = last_session.clone();
        let completed: Completion = Arc::new(completed);
        let failed = completed.clone();
        std::thread::Builder::new()
            .name("swm-egui-window-menu".into())
            .spawn(move || {
                let startup_ready = ready.clone();
                let result = crate::egui_native::run(
                    move |hwnd, ctx| {
                        *thread_context.lock().unwrap() = Some(ctx.clone());
                        WindowMenuApp {
                            receiver,
                            completed,
                            hwnd,
                            active_menu: None,
                            awaiting_handoff: false,
                            session: None,
                            appearance: desktop_appearance(),
                            file_preview_resolver,
                            result_rows,
                            preview_rows,
                            had_focus: false,
                            quitting: false,
                            opened_frame: None,
                            presentation: None,
                            full_pane: false,
                            preview_visible: false,
                        }
                    },
                    move || {
                        let _ = startup_ready.send(Ok(()));
                    },
                );
                if let Err(error) = result {
                    let _ = ready.send(Err(error.clone()));
                    crate::error_log::report("egui window menu stopped", error.to_string());
                    if let Some(session) = *thread_session.lock().unwrap() {
                        failed(session, Outcome::Failed(error.to_string()));
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        initialized
            .recv()
            .map_err(|_| "picker startup thread stopped before initialization".to_string())??;
        Ok(Self {
            sender,
            context,
            last_session,
        })
    }
    fn send(&self, command: Command) -> Result<(), String> {
        self.sender
            .send(command)
            .map_err(|error| error.to_string())?;
        if let Some(ctx) = self.context.lock().unwrap().as_ref() {
            ctx.request_repaint();
        }
        Ok(())
    }
    pub fn show_request(
        &self,
        session: u64,
        bounds: ScreenBounds,
        request: Box<dyn PickerRequest>,
    ) -> Result<(), String> {
        *self.last_session.lock().unwrap() = Some(session);
        self.send(Command::Show {
            session,
            bounds,
            request,
        })
    }
    pub fn hide(&self) {
        *self.last_session.lock().unwrap() = None;
        let _ = self.send(Command::Hide);
    }
    pub fn focus(&self) {
        let _ = self.send(Command::Focus);
    }
}
impl Drop for WindowMenuHost {
    fn drop(&mut self) {
        let _ = self.send(Command::Quit);
    }
}

// Native pixels: the supplied monitor bounds already exclude the SWM bar.
fn picker_window_bounds(
    bounds: ScreenBounds,
    height_points: f32,
    scale: f32,
    full_pane: bool,
) -> ScreenBounds {
    if full_pane {
        return bounds;
    }
    let monitor_margin = (80.0 * scale).round() as i32;
    let width = ((PICKER_WIDTH * scale).round() as i32)
        .min(bounds.width.saturating_sub(monitor_margin))
        .max(1);
    let height = ((height_points * scale).ceil() as i32)
        .min(bounds.height.saturating_sub(monitor_margin))
        .max(1);
    ScreenBounds {
        left: bounds.left + (bounds.width - width) / 2,
        top: bounds.top + (bounds.height - height) / 2,
        width,
        height,
    }
}

fn full_pane(menu: &dyn PickerView) -> bool {
    menu.picker().effective_position(menu.position()) == nfm_egui::Position::FullPane
}

pub(crate) struct WindowMenuApp {
    receiver: mpsc::Receiver<Command>,
    completed: Completion,
    hwnd: isize,
    active_menu: Option<Box<dyn PickerView>>,
    awaiting_handoff: bool,
    session: Option<u64>,
    appearance: Appearance,
    file_preview_resolver: crate::file_preview::FilePreviewResolver,
    result_rows: usize,
    preview_rows: usize,
    had_focus: bool,
    quitting: bool,
    opened_frame: Option<u64>,
    presentation: Option<ScreenBounds>,
    full_pane: bool,
    preview_visible: bool,
}
impl WindowMenuApp {
    fn clear_content(&mut self) {
        self.active_menu = None;
        self.presentation = None;
        self.full_pane = false;
        self.preview_visible = false;
        self.session = None;
        self.had_focus = false;
        self.opened_frame = None;
        self.awaiting_handoff = false;
    }
    fn hide(&mut self, ctx: &egui::Context) {
        self.clear_content();
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        unsafe {
            ShowWindow(self.hwnd as _, SW_HIDE);
        }
    }
    fn finish(&mut self, ctx: &egui::Context, outcome: Outcome) {
        let session = self.session;
        self.hide(ctx);
        if let Some(session) = session {
            (self.completed)(session, outcome);
        }
    }
    /// Window height in points: the configured result rows, the view's header row
    /// and, while it is visible, the preview pane.
    fn height(&self, ctx: &egui::Context, menu: &dyn PickerView) -> f32 {
        let row = self.appearance.typography.preview_row_height(ctx);
        let mut rows = self.result_rows + 1;
        if menu.has_header() {
            rows += 1;
        }
        let preview = if menu.picker().preview_visible() {
            self.preview_rows as f32 * row + 15.0
        } else {
            0.0
        };
        rows as f32 * row + 56.0 + 2.0 * PICKER_MARGIN + preview
    }
    /// Size the window for the active picker; called again when its layout changes.
    fn present(&mut self, ctx: &egui::Context, bounds: ScreenBounds) {
        let Some(menu) = &self.active_menu else {
            return;
        };
        let height_points = self.height(ctx, menu.as_ref());
        self.presentation = Some(bounds);
        self.full_pane = full_pane(menu.as_ref());
        self.preview_visible = menu.picker().preview_visible();
        let visible = unsafe { IsWindowVisible(self.hwnd as _) != 0 };
        unsafe {
            let scale =
                windows_sys::Win32::UI::HiDpi::GetDpiForWindow(self.hwnd as _) as f32 / 96.0;
            let window = picker_window_bounds(bounds, height_points, scale, self.full_pane);
            SetWindowPos(
                self.hwnd as _,
                HWND_TOPMOST,
                window.left,
                window.top,
                window.width,
                window.height,
                SWP_NOACTIVATE,
            );
        }
        if !visible {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        ctx.request_repaint();
    }

    pub(crate) fn logic(&mut self, ctx: &egui::Context, close_requested: bool) {
        // Handle close/focus from the previous session before queued reopen requests.
        if close_requested && !self.quitting {
            self.finish(ctx, Outcome::Cancelled);
        }
        if self.session.is_some() {
            // Queued focus events can lag behind a Hide/Show pair. Use the actual
            // native foreground window so old events cannot cancel a new opening.
            let focused = unsafe { GetForegroundWindow() == self.hwnd as _ };
            if self.had_focus && !focused {
                self.finish(ctx, Outcome::Cancelled);
            } else {
                self.had_focus |= focused;
            }
        }
        // Native wake messages process commands even while the window is hidden.
        while let Ok(command) = self.receiver.try_recv() {
            match command {
                Command::Show {
                    session,
                    bounds,
                    request,
                } => {
                    self.clear_content();
                    self.session = Some(session);
                    self.opened_frame = Some(ctx.cumulative_frame_nr());
                    let host = PickerHost {
                        ctx,
                        hwnd: self.hwnd,
                        appearance: &self.appearance,
                        result_rows: self.result_rows,
                        preview_rows: self.preview_rows,
                        file_preview_resolver: &self.file_preview_resolver,
                    };
                    match request.build(&host) {
                        Ok(menu) => {
                            self.active_menu = Some(menu);
                            self.present(ctx, bounds);
                        }
                        Err(error) => self.finish(ctx, Outcome::Failed(error)),
                    }
                }
                Command::Hide => self.hide(ctx),
                Command::Focus if self.session.is_some() => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                Command::Focus => {}
                Command::Quit => {
                    self.hide(ctx);
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
            }
        }
    }
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx();
        if self.awaiting_handoff
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.finish(ctx, Outcome::Cancelled);
            return;
        }
        // A callback can reopen the menu during the same egui frame that accepted
        // or cancelled it. Do not deliver that frame's Enter/Escape to the new picker.
        if self.opened_frame == Some(ctx.cumulative_frame_nr()) {
            ctx.input_mut(|input| input.events.clear());
        }
        let Some(menu) = &mut self.active_menu else {
            return;
        };
        let mut placement = Placement::new(ctx.content_rect());
        placement.margin = PICKER_MARGIN;
        // Ctrl+Shift+F switches away from the picker's own position.
        placement.position = menu.position();
        let outcome = menu.show(ctx, placement, &self.appearance, self.awaiting_handoff);
        // Ctrl+P and full-pane toggles change the window's size.
        if full_pane(menu.as_ref()) != self.full_pane
            || menu.picker().preview_visible() != self.preview_visible
        {
            if let Some(bounds) = self.presentation {
                self.present(ctx, bounds);
            }
        }
        match outcome {
            // Keep the window for the next picker; see `awaiting_handoff`.
            Some(Outcome::Action(action)) if crate::ui::is_menu(action) => {
                self.awaiting_handoff = true;
                if let Some(session) = self.session {
                    (self.completed)(session, Outcome::Action(action));
                }
            }
            Some(outcome) => self.finish(ctx, outcome),
            None => {}
        }
    }
    pub(crate) fn quitting(&self) -> bool {
        self.quitting
    }
    pub(crate) fn awaiting_handoff(&self) -> bool {
        self.awaiting_handoff
    }
}

#[cfg(test)]
mod full_pane_geometry_tests {
    use super::*;
    fn dimensions(bounds: ScreenBounds) -> [i32; 4] {
        [bounds.left, bounds.top, bounds.width, bounds.height]
    }
    #[test]
    fn full_pane_uses_monitor_pixels_without_rescaling_or_covering_bar() {
        let monitor = ScreenBounds {
            left: -2560,
            top: 45,
            width: 2560,
            height: 1395,
        };
        for scale in [1.0, 1.5, 2.0] {
            assert_eq!(
                dimensions(picker_window_bounds(monitor, 500.0, scale, true)),
                [-2560, 45, 2560, 1395]
            );
        }
    }
    #[test]
    fn popup_restores_centered_dpi_scaled_size_and_stays_inside_monitor() {
        let monitor = ScreenBounds {
            left: -2560,
            top: 45,
            width: 2560,
            height: 1395,
        };
        assert_eq!(
            dimensions(picker_window_bounds(monitor, 500.0, 1.5, false)),
            [-2480, 367, 2400, 750]
        );
        let small = ScreenBounds {
            left: 0,
            top: 29,
            width: 800,
            height: 571,
        };
        assert_eq!(
            dimensions(picker_window_bounds(small, 500.0, 1.0, false)),
            [40, 69, 720, 491]
        );
    }
}
