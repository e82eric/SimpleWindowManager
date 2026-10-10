//! SimpleWindowManager owns this native tool window, event loop and OpenGL renderer.
//! NFM supplies only enumeration, search and the embedded picker control.
use crate::egui_file_menu::{FileEvent, FileMenu};
use nfm_egui::shared::{self, SharedPicker, SharedSource};
use nfm_egui::{Appearance, PickerConfig, PickerEvent, Placement, egui};
use nfm_picker_sources::structured::StructuredPickerItem;
use nfm_window_egui::{WindowEvent, WindowPicker};
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

fn desktop_height(ctx: &egui::Context, appearance: &Appearance, result_rows: usize) -> f32 {
    // Reserve layout overhead for the configured result slots.
    (result_rows + 1) as f32 * appearance.typography.preview_row_height(ctx)
        + 56.0
        + 2.0 * PICKER_MARGIN
}

pub use crate::egui_data_menu::DataRow;

#[derive(Clone, Copy)]
pub struct ScreenBounds {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

pub enum Outcome {
    Selected(isize),
    FileSelected(String),
    ProgramSelected { path: String, elevated: bool },
    CommandSelected(String),
    MenuRequested(String),
    Cancelled,
    Failed(String),
}
type Completion = Arc<dyn Fn(u64, Outcome) + Send + Sync>;
enum Command {
    ShowData {
        session: u64,
        bounds: ScreenBounds,
        title: String,
        columns: Vec<String>,
        rows: Vec<crate::egui_data_menu::DataRow>,
    },
    ShowCommands {
        session: u64,
        bounds: ScreenBounds,
        rows: Vec<Vec<String>>,
        menu_actions: Vec<String>,
    },
    ShowProcesses {
        session: u64,
        bounds: ScreenBounds,
    },
    ShowFiles {
        session: u64,
        bounds: ScreenBounds,
        options: nfm_file_system::FileSystemPickerOptions,
        program_elevated: Option<bool>,
    },
    Show {
        session: u64,
        bounds: ScreenBounds,
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
    pub fn show(&self, session: u64, bounds: ScreenBounds) -> Result<(), String> {
        *self.last_session.lock().unwrap() = Some(session);
        self.send(Command::Show { session, bounds })
    }
    pub fn hide(&self) {
        *self.last_session.lock().unwrap() = None;
        let _ = self.send(Command::Hide);
    }
    pub fn show_file_system(
        &self,
        session: u64,
        bounds: ScreenBounds,
        options: nfm_file_system::FileSystemPickerOptions,
    ) -> Result<(), String> {
        *self.last_session.lock().unwrap() = Some(session);
        self.send(Command::ShowFiles {
            session,
            bounds,
            options,
            program_elevated: None,
        })
    }
    pub fn show_programs(
        &self,
        session: u64,
        bounds: ScreenBounds,
        mut directories: Vec<String>,
        elevated: bool,
        bat_theme: Option<String>,
    ) -> Result<(), String> {
        if directories.is_empty() {
            directories.push(
                std::env::current_dir()
                    .map_err(|error| error.to_string())?
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        *self.last_session.lock().unwrap() = Some(session);
        self.send(Command::ShowFiles {
            session,
            bounds,
            options: nfm_file_system::FileSystemPickerOptions {
                roots: directories,
                bat_theme,
                max_depth: 5,
                files_only: true,
                preview_visible: false,
                ..Default::default()
            },
            program_elevated: Some(elevated),
        })
    }
    pub fn focus(&self) {
        let _ = self.send(Command::Focus);
    }
    pub fn show_commands(
        &self,
        session: u64,
        bounds: ScreenBounds,
        rows: Vec<Vec<String>>,
    ) -> Result<(), String> {
        self.show_commands_with_menu_actions(session, bounds, rows, Vec::new())
    }
    pub fn show_commands_with_menu_actions(
        &self,
        session: u64,
        bounds: ScreenBounds,
        rows: Vec<Vec<String>>,
        menu_actions: Vec<String>,
    ) -> Result<(), String> {
        *self.last_session.lock().unwrap() = Some(session);
        self.send(Command::ShowCommands {
            session,
            bounds,
            rows,
            menu_actions,
        })
    }
    pub fn show_data(
        &self,
        session: u64,
        bounds: ScreenBounds,
        title: String,
        columns: Vec<String>,
        rows: Vec<DataRow>,
    ) -> Result<(), String> {
        *self.last_session.lock().unwrap() = Some(session);
        self.send(Command::ShowData {
            session,
            bounds,
            title,
            columns,
            rows,
        })
    }
    pub fn show_processes(&self, session: u64, bounds: ScreenBounds) -> Result<(), String> {
        *self.last_session.lock().unwrap() = Some(session);
        self.send(Command::ShowProcesses { session, bounds })
    }
}
impl Drop for WindowMenuHost {
    fn drop(&mut self) {
        let _ = self.send(Command::Quit);
    }
}

/// Exactly one workflow owns the retained picker at a time.
enum ActiveMenu {
    Windows(WindowPicker<SharedSource>),
    Commands {
        picker: SharedPicker,
        menu_actions: Vec<String>,
    },
    Data(SharedPicker),
    Files {
        menu: FileMenu,
        program_elevated: Option<bool>,
    },
    Processes(crate::egui_process_menu::ProcessMenu),
}
impl ActiveMenu {
    fn show(
        &mut self,
        ctx: &egui::Context,
        placement: Placement,
        appearance: &Appearance,
        awaiting_handoff: bool,
    ) -> Option<Outcome> {
        match self {
            Self::Windows(menu) => menu
                .show(ctx, placement, appearance)
                .map(|event| match event {
                    WindowEvent::Selected(hwnd) => Outcome::Selected(hwnd),
                    WindowEvent::Cancelled => Outcome::Cancelled,
                }),
            Self::Commands { .. } | Self::Data(_) => {
                let (picker, menu_actions) = match self {
                    Self::Commands {
                        picker,
                        menu_actions,
                    } => (picker, Some(menu_actions.as_slice())),
                    Self::Data(picker) => (picker, None),
                    _ => unreachable!(),
                };
                let is_commands = menu_actions.is_some();
                let output =
                    picker.show(ctx, placement, appearance, is_commands && awaiting_handoff);
                match output.event {
                    Some(PickerEvent::Accepted(selection)) if is_commands => {
                        let name = selection
                            .item
                            .downcast_ref::<StructuredPickerItem>()?
                            .fields
                            .get("Name")?
                            .clone();
                        Some(
                            if menu_actions.is_some_and(|actions| actions.contains(&name)) {
                                Outcome::MenuRequested(name)
                            } else {
                                Outcome::CommandSelected(name)
                            },
                        )
                    }
                    Some(PickerEvent::Accepted(_)) | Some(PickerEvent::Cancelled) => {
                        Some(Outcome::Cancelled)
                    }
                    Some(PickerEvent::CopyRequested(selection)) => {
                        ctx.copy_text(selection.text);
                        picker.notify_copied(ctx);
                        None
                    }
                    Some(PickerEvent::CustomAction { .. }) | None => None,
                }
            }
            Self::Processes(menu) => menu
                .show(ctx, placement, appearance)
                .then_some(Outcome::Cancelled),
            Self::Files {
                menu,
                program_elevated,
            } => match menu.show(ctx, placement, appearance) {
                Ok(Some(FileEvent::Selected(path))) => Some(match *program_elevated {
                    Some(elevated) => Outcome::ProgramSelected { path, elevated },
                    None => Outcome::FileSelected(path),
                }),
                Ok(Some(FileEvent::Cancelled)) => Some(Outcome::Cancelled),
                Ok(None) => None,
                Err(error) => Some(Outcome::Failed(error)),
            },
        }
    }
    fn into_picker(self) -> SharedPicker {
        match self {
            Self::Commands { picker, .. } | Self::Data(picker) => picker,
            Self::Windows(menu) => menu.into_picker(),
            Self::Files { menu, .. } => menu.into_picker(),
            Self::Processes(menu) => menu.into_picker(),
        }
    }
}

pub(crate) struct WindowMenuApp {
    receiver: mpsc::Receiver<Command>,
    completed: Completion,
    hwnd: isize,
    active_menu: Option<ActiveMenu>,
    awaiting_handoff: bool,
    session: Option<u64>,
    appearance: Appearance,
    file_preview_resolver: crate::file_preview::FilePreviewResolver,
    result_rows: usize,
    preview_rows: usize,
    had_focus: bool,
    quitting: bool,
    opened_frame: Option<u64>,
}
impl WindowMenuApp {
    fn take_picker(&mut self) -> Option<SharedPicker> {
        self.active_menu.take().map(ActiveMenu::into_picker)
    }
    fn clear_content(&mut self) {
        self.active_menu = None;
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
    fn show_files(
        &mut self,
        ctx: &egui::Context,
        session: u64,
        bounds: ScreenBounds,
        options: nfm_file_system::FileSystemPickerOptions,
        program_elevated: Option<bool>,
    ) {
        let previous = self.take_picker();
        self.clear_content();
        self.session = Some(session);
        self.opened_frame = Some(ctx.cumulative_frame_nr());
        let preview_visible = options.preview_visible;
        match FileMenu::with_picker(ctx, options, self.result_rows, self.preview_rows, previous) {
            Ok(mut menu) => {
                let resolve = self.file_preview_resolver.clone();
                menu.picker_mut().set_preview_provider(
                    ctx,
                    Arc::new(crate::file_preview::ResolvedPreviewProvider::new(
                        move |selection: &nfm_egui::Selection<nfm_egui::shared::SharedItem>,
                              cancel| {
                            let Some(path) = selection.item.downcast_ref::<String>() else {
                                return Ok(None);
                            };
                            resolve(std::path::Path::new(path), cancel)
                        },
                    )),
                );
                self.active_menu = Some(ActiveMenu::Files {
                    menu,
                    program_elevated,
                });
            }
            Err(error) => {
                self.finish(ctx, Outcome::Failed(error));
                return;
            }
        }
        let height = desktop_height(ctx, &self.appearance, self.result_rows)
            + if preview_visible {
                self.preview_rows as f32 * self.appearance.typography.preview_row_height(ctx) + 15.0
            } else {
                0.0
            };
        self.present(ctx, bounds, height);
    }
    fn finish(&mut self, ctx: &egui::Context, outcome: Outcome) {
        let session = self.session;
        self.hide(ctx);
        if let Some(session) = session {
            (self.completed)(session, outcome);
        }
    }
    fn show(&mut self, ctx: &egui::Context, session: u64, bounds: ScreenBounds) {
        let previous = self.take_picker();
        self.clear_content();
        self.session = Some(session);
        self.opened_frame = Some(ctx.cumulative_frame_nr());
        match WindowPicker::with_shared_picker(
            ctx,
            self.hwnd,
            PickerConfig {
                result_rows: self.result_rows,
                preview_rows: self.preview_rows,
                preview_visible: true,
                ..Default::default()
            },
            previous,
        ) {
            Ok(menu) => self.active_menu = Some(ActiveMenu::Windows(menu)),
            Err(error) => {
                self.finish(ctx, Outcome::Failed(error));
                return;
            }
        }
        self.present(
            ctx,
            bounds,
            desktop_height(ctx, &self.appearance, self.result_rows)
                + self.preview_rows as f32 * self.appearance.typography.preview_row_height(ctx)
                + 15.0,
        );
    }
    fn present(&self, ctx: &egui::Context, bounds: ScreenBounds, height_points: f32) {
        let visible = unsafe { IsWindowVisible(self.hwnd as _) != 0 };
        unsafe {
            let scale =
                windows_sys::Win32::UI::HiDpi::GetDpiForWindow(self.hwnd as _) as f32 / 96.0;
            let width = (PICKER_WIDTH * scale).round() as i32;
            let height = (height_points * scale).ceil() as i32;
            let monitor_margin = (80.0 * scale).round() as i32;
            let width = width
                .min(bounds.width.saturating_sub(monitor_margin))
                .max(1);
            let height = height
                .min(bounds.height.saturating_sub(monitor_margin))
                .max(1);
            SetWindowPos(
                self.hwnd as _,
                HWND_TOPMOST,
                bounds.left + (bounds.width - width) / 2,
                bounds.top + (bounds.height - height) / 2,
                width,
                height,
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
                Command::ShowCommands {
                    session,
                    bounds,
                    rows,
                    menu_actions,
                } => {
                    let previous = self.take_picker();
                    self.clear_content();
                    self.session = Some(session);
                    self.opened_frame = Some(ctx.cumulative_frame_nr());
                    let source = match crate::egui_command_menu::source(rows) {
                        Ok(source) => source,
                        Err(error) => {
                            self.finish(ctx, Outcome::Failed(error));
                            continue;
                        }
                    };
                    self.active_menu = Some(ActiveMenu::Commands {
                        menu_actions,
                        picker: shared::replace(
                            ctx,
                            source,
                            PickerConfig {
                                title: "Commands".into(),
                                result_rows: self.result_rows,
                                preview_rows: self.preview_rows,
                                preview_visible: false,
                                ..Default::default()
                            },
                            previous,
                        ),
                    });
                    self.present(
                        ctx,
                        bounds,
                        desktop_height(ctx, &self.appearance, self.result_rows)
                            + self.appearance.typography.preview_row_height(ctx),
                    );
                }
                Command::ShowData {
                    session,
                    bounds,
                    title,
                    columns,
                    rows,
                } => {
                    let previous = self.take_picker();
                    self.clear_content();
                    self.session = Some(session);
                    self.opened_frame = Some(ctx.cumulative_frame_nr());
                    match crate::egui_data_menu::replace_picker(
                        ctx,
                        title,
                        columns,
                        rows,
                        self.result_rows,
                        self.preview_rows,
                        previous,
                    ) {
                        Ok(picker) => self.active_menu = Some(ActiveMenu::Data(picker)),
                        Err(error) => {
                            self.finish(ctx, Outcome::Failed(error));
                            continue;
                        }
                    }
                    self.present(
                        ctx,
                        bounds,
                        desktop_height(ctx, &self.appearance, self.result_rows)
                            + self.appearance.typography.preview_row_height(ctx)
                            + self.preview_rows as f32
                                * self.appearance.typography.preview_row_height(ctx)
                            + 15.0,
                    );
                }
                Command::ShowProcesses { session, bounds } => {
                    let previous = self.take_picker();
                    self.clear_content();
                    self.session = Some(session);
                    self.opened_frame = Some(ctx.cumulative_frame_nr());
                    let menu = crate::egui_process_menu::ProcessMenu::with_picker(
                        ctx,
                        self.result_rows,
                        self.preview_rows,
                        previous,
                    );
                    self.active_menu = Some(ActiveMenu::Processes(menu));
                    self.present(
                        ctx,
                        bounds,
                        desktop_height(ctx, &self.appearance, self.result_rows)
                            + self.appearance.typography.preview_row_height(ctx)
                            + self.preview_rows as f32
                                * self.appearance.typography.preview_row_height(ctx)
                            + 15.0,
                    );
                }
                Command::ShowFiles {
                    session,
                    bounds,
                    options,
                    program_elevated,
                } => {
                    self.show_files(ctx, session, bounds, options, program_elevated);
                }
                Command::Show { session, bounds } => self.show(ctx, session, bounds),
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
        match menu.show(ctx, placement, &self.appearance, self.awaiting_handoff) {
            Some(Outcome::MenuRequested(name)) => {
                self.awaiting_handoff = true;
                if let Some(session) = self.session {
                    (self.completed)(session, Outcome::MenuRequested(name));
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
