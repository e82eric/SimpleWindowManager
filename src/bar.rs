use crate::palette;
use nfm_egui::egui;

#[derive(Clone, Debug)]
pub struct BarSnapshot {
    pub monitor_index: usize,
    pub selected_monitor: bool,
    pub tags: Vec<(usize, String, bool, bool, bool)>,
    pub workspace_info: String,
    pub focused_window: String,
    pub segments: Vec<RenderedSegment>,
    pub focus_style: crate::config::BarFocusStyle,
    /// Each configured mailbox with what its producer last published.
    pub mailboxes: Vec<(
        crate::mailbox::BarMailbox,
        std::sync::Arc<crate::mailbox::MailboxState>,
    )>,
    pub background_color: u32,
    pub focus_background_color: u32,
    pub text_color: u32,
    pub info_color: u32,
    pub disabled_color: u32,
    pub focus_text_color: u32,
    pub extra_focus_background_color: u32,
    pub font_name: String,
    pub normal_font_size: i32,
    pub icon_font_size: i32,
}

#[derive(Clone, Debug)]
pub struct RenderedSegment {
    pub separator: Option<(String, bool)>,
    pub header: Option<(String, bool)>,
    pub value: String,
    pub value_is_icon: bool,
    pub tone: Tone,
}

/// How a segment's value reads, which sets its color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Normal,
    Warning,
    Critical,
    /// Muted, such as volume at zero.
    Off,
}

/// CPU and memory use at or above these read as a warning or critical.
const WARNING_PERCENT: u32 = 75;
const CRITICAL_PERCENT: u32 = 90;

fn tone(value: crate::config::BarSegmentValue, text: &str) -> Tone {
    use crate::config::BarSegmentValue::*;
    let percent = || text.trim().trim_end_matches('%').trim().parse::<u32>().ok();
    match value {
        Cpu | Memory => match percent() {
            Some(percent) if percent >= CRITICAL_PERCENT => Tone::Critical,
            Some(percent) if percent >= WARNING_PERCENT => Tone::Warning,
            _ => Tone::Normal,
        },
        Volume if percent() == Some(0) => Tone::Off,
        Internet if text == crate::status::INTERNET_OFFLINE => Tone::Critical,
        Internet if text == crate::status::INTERNET_UNKNOWN => Tone::Warning,
        _ => Tone::Normal,
    }
}

#[derive(Debug)]
pub struct BarWindow {
    pub hwnd: isize,
    pub monitor_index: usize,
}

unsafe extern "system" fn bar_message_loop(
    hwnd: windows_sys::Win32::Foundation::HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let span = matches!(
        message,
        WM_NCCREATE | WM_PAINT | WM_TIMER | WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_CLOSE | WM_DESTROY
    )
    .then(|| {
        tracing::info_span!(
            "window_message",
            component = "bar",
            message_name = crate::platform::window_message_name(message),
            message,
            hwnd = hwnd as usize,
            wparam,
            lparam,
        )
    });
    let _entered = span.as_ref().map(tracing::Span::enter);
    match message {
        WM_NCCREATE => {
            let create = unsafe { &*(lparam as *const CREATESTRUCTW) };
            unsafe {
                windows_sys::Win32::Foundation::SetLastError(0);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
                let error = windows_sys::Win32::Foundation::GetLastError();
                if error != 0 {
                    crate::error_log::report_win32("SetWindowLongPtrW(bar userdata) failed", error);
                    return 0;
                }
            }
            1
        }
        WM_PAINT => {
            unsafe { windows_sys::Win32::Foundation::SetLastError(0) };
            let userdata = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
            let userdata_error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            if userdata == 0 && userdata_error != 0 {
                crate::error_log::report_win32(
                    "GetWindowLongPtrW(bar userdata) failed",
                    userdata_error,
                );
            }
            let monitor = (userdata as usize).saturating_sub(1);
            let Some(snapshot) = crate::platform::bar_snapshot(monitor) else {
                return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
            };
            let mut paint: PAINTSTRUCT = unsafe { zeroed() };
            let dc = unsafe { BeginPaint(hwnd, &mut paint) };
            if dc.is_null() {
                crate::platform_error::PlatformError::null_output("BeginPaint(bar)").report();
                return 0;
            }
            let mut client: RECT = unsafe { zeroed() };
            if unsafe { GetClientRect(hwnd, &mut client) } == 0 {
                crate::platform_error::PlatformError::last_win32("GetClientRect(bar)").report();
                // A successful BeginPaint must be paired with EndPaint, even
                // when later paint setup fails.
                unsafe { EndPaint(hwnd, &paint) };
                return 0;
            }
            if let Err(error) = crate::bar_renderer::paint(
                hwnd,
                [client.right.max(0) as u32, client.bottom.max(0) as u32],
                &snapshot,
            ) {
                crate::error_log::report("egui bar paint", error);
            }
            // Repaint quickly only while a mailbox shows an animated state.
            if snapshot
                .mailboxes
                .iter()
                .any(|(mailbox, state)| mailbox.animated(state))
            {
                unsafe { SetTimer(hwnd, ANIMATION_TIMER, ANIMATION_INTERVAL_MS, None) };
            } else {
                unsafe { KillTimer(hwnd, ANIMATION_TIMER) };
            }
            // EndPaint is documented to always return nonzero.
            unsafe { EndPaint(hwnd, &paint) };
            0
        }
        WM_TIMER => {
            if unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) } == 0 {
                crate::error_log::last_win32("InvalidateRect(bar timer) failed");
            }
            0
        }
        WM_ERASEBKGND => 1,
        WM_MOUSEACTIVATE => MA_NOACTIVATE as isize,
        WM_SIZE => {
            unsafe {
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_NCDESTROY => {
            crate::bar_renderer::detach(hwnd);
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_LBUTTONDOWN => {
            unsafe { windows_sys::Win32::Foundation::SetLastError(0) };
            let userdata = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
            let userdata_error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            if userdata == 0 && userdata_error != 0 {
                crate::error_log::report_win32(
                    "GetWindowLongPtrW(bar userdata) failed",
                    userdata_error,
                );
            }
            let monitor = (userdata as usize).saturating_sub(1);
            let x = lparam as i16 as i32;
            if let Some(snapshot) = crate::platform::bar_snapshot(monitor) {
                if let Some((_, _, index)) = mailbox_at_x(snapshot.monitor_index, x) {
                    toggle_mailbox(hwnd, snapshot.monitor_index, index);
                } else if let Some(workspace) = workspace_at_x(&snapshot, x) {
                    crate::platform::bar_workspace_clicked(monitor, workspace);
                }
            }
            0
        }
        WM_RBUTTONDOWN => {
            crate::platform::bar_commands_clicked();
            0
        }
        WM_CLOSE => {
            if unsafe { DestroyWindow(hwnd) } == 0 {
                crate::error_log::last_win32("DestroyWindow(bar) failed");
            }
            0
        }
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

/// The bar's regular repaint timer is id 0; this one runs only while animating.
const ANIMATION_TIMER: usize = 1;
const ANIMATION_INTERVAL_MS: u32 = 60;
/// Between a state's icon and its count, and between states.
const ICON_GAP: f32 = 6.0;
const STATE_GAP: f32 = 14.0;

/// A bar's mailbox regions from its last paint: (left, right, mailbox index).
type MailboxRegions = Vec<(f32, f32, usize)>;

thread_local! {
    /// Each monitor's mailbox regions, for clicks. Painting and clicks both run on
    /// the bar's thread.
    static MAILBOX_REGIONS: std::cell::RefCell<std::collections::HashMap<usize, MailboxRegions>> =
        Default::default();
}

/// The mailbox under `x` on the bar of `monitor_index`, with its region's edges.
pub(crate) fn mailbox_at_x(monitor_index: usize, x: i32) -> Option<(f32, f32, usize)> {
    MAILBOX_REGIONS.with(|all| {
        all.borrow().get(&monitor_index).and_then(|regions| {
            regions
                .iter()
                .copied()
                .find(|&(left, right, _)| (left..right).contains(&(x as f32)))
        })
    })
}

/// Opens or closes the drop-down of mailbox `index` below `hwnd`, the bar of
/// `monitor_index`: under the mailbox's region, or at the bar's right end while
/// the mailbox shows nothing.
pub(crate) fn toggle_mailbox(
    hwnd: windows_sys::Win32::Foundation::HWND,
    monitor_index: usize,
    index: usize,
) {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;
    let right = MAILBOX_REGIONS.with(|all| {
        all.borrow().get(&monitor_index).and_then(|regions| {
            regions
                .iter()
                .find(|&&(_, _, region)| region == index)
                .map(|&(_, right, _)| right)
        })
    });
    let mut bar: RECT = unsafe { std::mem::zeroed() };
    if unsafe { GetWindowRect(hwnd, &mut bar) } == 0 {
        crate::error_log::last_win32("GetWindowRect(bar) failed");
        return;
    }
    crate::mailbox_popup::toggle(
        index,
        monitor_index,
        crate::mailbox_popup::Anchor {
            right: right.map_or(bar.right, |right| bar.left + right as i32),
            top: bar.bottom,
            bounds: (bar.left, bar.right),
        },
    );
}

/// Paints `mailbox` at the right of `space`, and returns its region; `None` when no
/// declared state has a count or there is no room.
fn paint_mailbox(
    canvas: &egui::Painter,
    mailbox: &crate::mailbox::BarMailbox,
    state: &crate::mailbox::MailboxState,
    font: &egui::FontId,
    seconds: f64,
    shade: Shade,
    space: egui::Rect,
) -> Option<egui::Rect> {
    use crate::mailbox::Shape;
    let text_color = shade.content();
    let visible = mailbox.visible(state);
    if visible.is_empty() {
        return None;
    }
    let galley = |text: &str, color| canvas.layout_no_wrap(text.to_owned(), font.clone(), color);
    let header = mailbox
        .header
        .as_deref()
        .map(|header| galley(header, shade.context()));
    let parts: Vec<_> = visible
        .iter()
        .map(|(_, style, count)| {
            let icon = style.icon.as_ref();
            let icon_width = icon.map_or(0.0, |icon| match &icon.shape {
                // Wide enough for every frame, so the count does not move.
                Shape::Glyph(glyph) => std::iter::once(glyph)
                    .chain(match &icon.animation {
                        Some(crate::mailbox::Animation::Frames { frames, .. }) => frames.iter(),
                        _ => [].iter(),
                    })
                    .map(|text| galley(text, text_color).size().x)
                    .fold(0.0, f32::max),
                Shape::Drawn(_) => font.size,
            });
            let count = style.show_count.then(|| {
                galley(
                    &count.to_string(),
                    icon.map_or(text_color, |icon| shade.mark(icon.color)),
                )
            });
            (icon, icon_width, count)
        })
        .collect();
    let mut content = header
        .as_ref()
        .map_or(0.0, |header| header.size().x + ICON_GAP);
    for (index, (_, icon_width, count)) in parts.iter().enumerate() {
        if index > 0 {
            content += STATE_GAP;
        }
        content += icon_width;
        if let Some(count) = count {
            content += count.size().x + if *icon_width > 0.0 { ICON_GAP } else { 0.0 };
        }
    }
    let width = content + REGION_PADDING * 2.0;
    let region_left = (space.right() - width).max(space.left());
    if space.right() <= region_left {
        return None;
    }
    let region = egui::Rect::from_min_max(
        egui::pos2(region_left, space.top()),
        egui::pos2(space.right(), space.bottom()),
    );
    draw_region(canvas, region, false, false);
    let painter = canvas.with_clip_rect(region);
    let middle = region.center().y;
    let mut x = region.left() + REGION_PADDING;
    let text = |painter: &egui::Painter, x: &mut f32, galley: std::sync::Arc<egui::Galley>| {
        let width = galley.size().x;
        painter.galley(
            egui::pos2(*x, middle - galley.size().y / 2.0),
            galley,
            text_color,
        );
        *x += width;
    };
    if let Some(header) = header {
        text(&painter, &mut x, header);
        x += ICON_GAP;
    }
    for (index, (icon, icon_width, count)) in parts.into_iter().enumerate() {
        if index > 0 {
            x += STATE_GAP;
        }
        if let Some(icon) = icon {
            let frame = icon
                .animation
                .as_ref()
                .map_or(crate::mailbox::Frame::STILL, |a| a.frame(seconds));
            let color = shade.mark(icon.color).gamma_multiply(frame.opacity);
            let center = egui::pos2(x + icon_width / 2.0, middle);
            match &icon.shape {
                Shape::Glyph(glyph) => {
                    let glyph = match (&icon.animation, frame.glyph) {
                        (Some(crate::mailbox::Animation::Frames { frames, .. }), Some(i)) => {
                            &frames[i]
                        }
                        _ => glyph,
                    };
                    painter.text(
                        center,
                        egui::Align2::CENTER_CENTER,
                        glyph,
                        egui::FontId::new(font.size * frame.scale, font.family.clone()),
                        color,
                    );
                }
                Shape::Drawn(draw) => {
                    let size = font.size * frame.scale;
                    let area = egui::Rect::from_center_size(center, egui::vec2(size, size));
                    draw_shape(*draw, &painter, area, color);
                }
            }
            x += icon_width;
            if count.is_some() {
                x += ICON_GAP;
            }
        }
        if let Some(count) = count {
            text(&painter, &mut x, count);
        }
    }
    Some(region)
}

/// Runs a configured shape; one that panics is reported once and then not drawn.
fn draw_shape(
    draw: crate::mailbox::DrawShape,
    painter: &egui::Painter,
    area: egui::Rect,
    color: egui::Color32,
) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static FAILED: AtomicBool = AtomicBool::new(false);
    if FAILED.load(Ordering::Relaxed) {
        return;
    }
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| draw(painter, area, color)));
    if result.is_err() && !FAILED.swap(true, Ordering::Relaxed) {
        crate::error_log::report(
            "bar mailbox",
            "a drawn shape panicked; it is no longer drawn",
        );
    }
}

const BAR_MARGIN: f32 = 2.0;
const REGION_GAP: f32 = 12.0;
const REGION_PADDING: f32 = 14.0;
const TAG_WIDTH: f32 = 42.0;

fn tag_groups(snapshot: &BarSnapshot) -> Vec<(usize, usize, f32, f32)> {
    if snapshot.tags.is_empty() {
        Vec::new()
    } else {
        let width = REGION_PADDING * 2.0 + snapshot.tags.len() as f32 * TAG_WIDTH;
        vec![(0, snapshot.tags.len(), BAR_MARGIN, BAR_MARGIN + width)]
    }
}

fn workspace_at_x(snapshot: &BarSnapshot, x: i32) -> Option<usize> {
    let x = x as f32;
    for (start, end, left, right) in tag_groups(snapshot) {
        if x >= left + REGION_PADDING && x < right - REGION_PADDING {
            let index = start + ((x - left - REGION_PADDING) / TAG_WIDTH) as usize;
            if index < end {
                return Some(snapshot.tags[index].0);
            }
        }
    }
    None
}

pub(super) fn paint_bar(ui: &mut egui::Ui, size: [u32; 2], snapshot: &BarSnapshot) {
    use egui::{FontFamily, FontId, Rect, pos2};
    let canvas = ui.ctx().layer_painter(egui::LayerId::background());
    let normal_font = FontId::new(
        snapshot.normal_font_size.max(1) as f32,
        FontFamily::Name("bar".into()),
    );
    let icon_font = FontId::new(
        snapshot.icon_font_size.max(1) as f32,
        FontFamily::Name("bar".into()),
    );
    let rect = |left, top, right, bottom| Rect::from_min_max(pos2(left, top), pos2(right, bottom));
    let top = 4.0;
    let bottom = size[1] as f32 - 4.0;
    let shade = Shade::of(snapshot);
    let groups = tag_groups(snapshot);
    for (start, end, left, right) in &groups {
        draw_region(
            &canvas,
            rect(*left, top, *right, bottom),
            snapshot.selected_monitor,
            false,
        );
        for index in *start..*end {
            let (_, tag, selected, has_clients, is_icon) = &snapshot.tags[index];
            let item_left = *left + REGION_PADDING + (index - *start) as f32 * TAG_WIDTH;
            let item = rect(item_left, top + 3.0, item_left + TAG_WIDTH, bottom - 3.0);
            if *selected {
                canvas.rect_filled(
                    item,
                    7,
                    if snapshot.selected_monitor {
                        palette::BACKGROUND
                    } else {
                        palette::INACTIVE
                    },
                );
            }
            let color = match (shade, *selected) {
                (Shade::Dimmed, true) => palette::MUTED,
                (_, true) => palette::TEXT,
                // On the focused monitor's accent background.
                (Shade::Focused, false) => palette::BACKGROUND,
                (Shade::Unfocused, false) => palette::FAINT,
                (Shade::Dimmed, false) => palette::INACTIVE,
            };
            draw_centered_text(
                &canvas,
                tag,
                rect(item.left(), item.top(), item.right(), item.bottom() - 6.0),
                if *is_icon { &icon_font } else { &normal_font },
                color,
            );
            if *has_clients {
                canvas.rect_filled(
                    rect(
                        item.center().x - 6.0,
                        item.bottom() - 4.0,
                        item.center().x + 6.0,
                        item.bottom() - 1.0,
                    ),
                    egui::CornerRadius::same(2),
                    color,
                );
            }
        }
    }
    let mut left = groups
        .last()
        .map(|group| group.3 + REGION_GAP)
        .unwrap_or(BAR_MARGIN);
    let status = status_pieces(&canvas, &snapshot.segments, &normal_font, &icon_font, shade);
    let status_width = pieces_width(&status) + REGION_PADDING * 2.0;
    let status_left = (size[0] as f32 - BAR_MARGIN - status_width).max(left);
    let info_width = (text_width(&canvas, &normal_font, &snapshot.workspace_info)
        + REGION_PADDING * 2.0)
        .max(180.0);
    let info = rect(left, top, left + info_width, bottom);
    let region_text = shade.content();
    draw_region(&canvas, info, false, false);
    draw_centered_text(
        &canvas,
        &snapshot.workspace_info,
        info,
        &normal_font,
        region_text,
    );
    left = info.right() + REGION_GAP;
    let mut available_right = status_left - REGION_GAP;
    let seconds = ui.input(|input| input.time);
    let mut regions = Vec::new();
    // The first mailbox added is leftmost, so place them from the right.
    for (index, (mailbox, state)) in snapshot.mailboxes.iter().enumerate().rev() {
        let Some(region) = paint_mailbox(
            &canvas,
            mailbox,
            state,
            &normal_font,
            seconds,
            shade,
            rect(left, top, available_right, bottom),
        ) else {
            continue;
        };
        regions.push((region.left(), region.right(), index));
        available_right = region.left() - REGION_GAP;
    }
    MAILBOX_REGIONS.with(|all| all.borrow_mut().insert(snapshot.monitor_index, regions));
    let desired_focus = (text_width(&canvas, &normal_font, &snapshot.focused_window)
        + REGION_PADDING * 2.0)
        .max(240.0);
    let focus_width = desired_focus.min((available_right - left).max(0.0));
    let centered_left = size[0] as f32 / 2.0 - focus_width / 2.0;
    let focus_left = centered_left
        .max(left)
        .min((available_right - focus_width).max(left));
    if focus_width > 0.0 {
        let focus = rect(focus_left, top, focus_left + focus_width, bottom);
        draw_region(
            &canvas,
            focus,
            false,
            snapshot.selected_monitor && !snapshot.focused_window.is_empty(),
        );
        draw_centered_text(
            &canvas,
            &snapshot.focused_window,
            focus,
            &normal_font,
            region_text,
        );
    }
    if status_left < size[0] as f32 - BAR_MARGIN {
        let region = rect(status_left, top, size[0] as f32 - BAR_MARGIN, bottom);
        draw_region(&canvas, region, false, false);
        draw_pieces(
            &canvas.with_clip_rect(region),
            &status,
            region.left() + REGION_PADDING,
            region,
        );
    }
}

/// How brightly a bar draws, by whether its monitor is focused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shade {
    Focused,
    Unfocused,
    /// Unfocused, with `BarFocusStyle::dim_unfocused`.
    Dimmed,
}
impl Shade {
    fn of(snapshot: &BarSnapshot) -> Self {
        match (
            snapshot.selected_monitor,
            snapshot.focus_style.dim_unfocused,
        ) {
            (true, _) => Self::Focused,
            (false, false) => Self::Unfocused,
            (false, true) => Self::Dimmed,
        }
    }
    /// Headers, processes and other context.
    fn context(self) -> egui::Color32 {
        match self {
            Self::Focused => palette::MUTED,
            Self::Unfocused => palette::FAINT,
            Self::Dimmed => palette::INACTIVE,
        }
    }
    /// Values, titles and other content.
    fn content(self) -> egui::Color32 {
        match self {
            Self::Focused => palette::TEXT,
            Self::Unfocused => palette::MUTED,
            Self::Dimmed => palette::FAINT,
        }
    }
    /// A color that carries meaning, such as a warning or a mailbox icon.
    fn mark(self, color: egui::Color32) -> egui::Color32 {
        if self == Self::Dimmed {
            color.gamma_multiply(0.55)
        } else {
            color
        }
    }
}

/// What a status or focus region draws, left to right.
enum Piece {
    Text(std::sync::Arc<egui::Galley>),
    /// Space, in points.
    Gap(f32),
    /// A thin vertical line between items.
    Divider,
}
/// Space on each side of a divider, and between items without one.
const DIVIDER_GAP: f32 = 9.0;
const ITEM_GAP: f32 = 8.0;
/// Space between a segment's header and its value.
const HEADER_GAP: f32 = 5.0;

fn pieces_width(pieces: &[Piece]) -> f32 {
    pieces
        .iter()
        .map(|piece| match piece {
            Piece::Text(galley) => galley.size().x,
            Piece::Gap(gap) => *gap,
            Piece::Divider => DIVIDER_GAP * 2.0 + 1.0,
        })
        .sum()
}

fn draw_pieces(painter: &egui::Painter, pieces: &[Piece], mut x: f32, region: egui::Rect) {
    let middle = region.center().y;
    for piece in pieces {
        match piece {
            Piece::Text(galley) => {
                let size = galley.size();
                painter.galley(
                    egui::pos2(x, middle - size.y / 2.0),
                    galley.clone(),
                    palette::TEXT,
                );
                x += size.x;
            }
            Piece::Gap(gap) => x += gap,
            Piece::Divider => {
                x += DIVIDER_GAP;
                let inset = region.height() * 0.28;
                painter.line_segment(
                    [
                        egui::pos2(x + 0.5, region.top() + inset),
                        egui::pos2(x + 0.5, region.bottom() - inset),
                    ],
                    egui::Stroke::new(1.0, palette::BORDER),
                );
                x += 1.0 + DIVIDER_GAP;
            }
        }
    }
}

/// The status segments, the first configured rightmost: headers muted, values
/// colored by their tone, and a `|` separator drawn as a divider.
fn status_pieces(
    canvas: &egui::Painter,
    segments: &[RenderedSegment],
    normal: &egui::FontId,
    icon: &egui::FontId,
    shade: Shade,
) -> Vec<Piece> {
    let (header_color, value_color) = (shade.context(), shade.content());
    let text = |text: &str, is_icon: bool, color| {
        let font = if is_icon { icon } else { normal };
        Piece::Text(canvas.layout_no_wrap(text.to_owned(), font.clone(), color))
    };
    let mut pieces = Vec::new();
    for (index, segment) in segments.iter().rev().enumerate() {
        match &segment.separator {
            Some((separator, _)) if separator.trim() == "|" => {
                if index > 0 {
                    pieces.push(Piece::Divider);
                }
            }
            Some((separator, is_icon)) if !separator.trim().is_empty() => {
                pieces.push(text(separator, *is_icon, shade.mark(palette::FAINT)));
            }
            _ => {
                if index > 0 {
                    pieces.push(Piece::Gap(ITEM_GAP));
                }
            }
        }
        if let Some((header, is_icon)) = &segment.header {
            pieces.push(text(header.trim_end(), *is_icon, header_color));
            pieces.push(Piece::Gap(HEADER_GAP));
        }
        let color = match segment.tone {
            Tone::Normal => value_color,
            Tone::Warning => shade.mark(palette::WARNING),
            Tone::Critical => shade.mark(palette::CRITICAL),
            Tone::Off => shade.mark(palette::FAINT),
        };
        // The value starts right after its header and keeps the width of its padded
        // text, so `7%` and `100%` sit the same distance from the header and the
        // segments beside it don't move as it changes.
        let font = if segment.value_is_icon { icon } else { normal };
        let padded = canvas
            .layout_no_wrap(segment.value.clone(), font.clone(), color)
            .size()
            .x;
        let value = canvas.layout_no_wrap(segment.value.trim().to_owned(), font.clone(), color);
        let rest = padded - value.size().x;
        pieces.push(Piece::Text(value));
        if rest > 0.5 {
            pieces.push(Piece::Gap(rest));
        }
    }
    pieces
}

/// `text` on one line of at most `width`, ending in an ellipsis when cut.
pub(crate) fn one_line(
    painter: &egui::Painter,
    text: &str,
    font: &egui::FontId,
    color: egui::Color32,
    width: f32,
) -> std::sync::Arc<egui::Galley> {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut job = egui::text::LayoutJob::simple(flat, font.clone(), color, width.max(1.0));
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    painter.layout_job(job)
}

fn text_width(canvas: &egui::Painter, font: &egui::FontId, text: &str) -> f32 {
    canvas
        .layout_no_wrap(text.to_owned(), font.clone(), egui::Color32::WHITE)
        .size()
        .x
}

fn draw_region(
    canvas: &egui::Painter,
    rect: egui::Rect,
    focused_monitor_background: bool,
    focus_outline: bool,
) {
    use egui::{Color32, Stroke, StrokeKind};
    canvas.add(
        egui::epaint::Shadow {
            offset: [0, 2],
            blur: 5,
            spread: 0,
            color: Color32::from_black_alpha(45),
        }
        .as_shape(rect, 8),
    );
    canvas.rect_filled(
        rect,
        8,
        if focused_monitor_background {
            palette::ACCENT
        } else {
            palette::BACKGROUND
        },
    );
    if focus_outline {
        canvas.rect_stroke(
            rect.shrink(0.75),
            egui::CornerRadius::same(7),
            Stroke::new(1.5, palette::ACCENT),
            StrokeKind::Middle,
        );
    }
}

fn draw_centered_text(
    canvas: &egui::Painter,
    text: &str,
    rect: egui::Rect,
    font: &egui::FontId,
    color: egui::Color32,
) {
    // Clip long titles horizontally so they cannot cover adjacent regions.
    // Keep the bar's vertical clip: short workspace-label regions reserve room
    // for occupancy markers but must not crop the font's ascenders/descenders.
    let clip = egui::Rect::from_min_max(
        egui::pos2(rect.left(), canvas.clip_rect().top()),
        egui::pos2(rect.right(), canvas.clip_rect().bottom()),
    );
    canvas.with_clip_rect(clip).text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        font.clone(),
        color,
    );
}

pub fn create_bars(
    monitors: &[crate::Monitor],
    bar_height: i32,
) -> crate::platform_error::PlatformResult<Vec<BarWindow>> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Controls::ShowScrollBar;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let class_name: Vec<u16> = "keys2BarWindowClass\0".encode_utf16().collect();
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    if instance.is_null() {
        return Err(crate::platform_error::PlatformError::last_win32(
            "GetModuleHandleW(bar)",
        ));
    }
    let mut class: WNDCLASSEXW = unsafe { zeroed() };
    class.cbSize = size_of::<WNDCLASSEXW>() as u32;
    class.style = CS_OWNDC;
    class.lpfnWndProc = Some(bar_message_loop);
    class.hInstance = instance;
    class.lpszClassName = class_name.as_ptr();
    class.hCursor = unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) };
    if class.hCursor.is_null() {
        crate::error_log::last_win32("LoadCursorW(bar) failed");
    }
    class.hIcon = unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) };
    if class.hIcon.is_null() {
        crate::error_log::last_win32("LoadIconW(bar) failed");
    }
    class.hIconSm = class.hIcon;
    class.hbrBackground = std::ptr::null_mut();
    unsafe {
        if RegisterClassExW(&class) == 0 {
            let error = crate::platform_error::PlatformError::last_win32("RegisterClassExW(bar)");
            if !error.is_win32(ERROR_CLASS_ALREADY_EXISTS) {
                return Err(error);
            }
        }
    }
    let title: Vec<u16> = "SimpleWM Bar\0".encode_utf16().collect();
    let mut result: Vec<BarWindow> = Vec::new();
    for (index, monitor) in monitors
        .iter()
        .enumerate()
        .filter(|(_, monitor)| !monitor.is_hidden)
    {
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_POPUP | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
                monitor.x_offset,
                0,
                monitor.width,
                bar_height,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                (index + 1) as *mut _,
            )
        };
        if hwnd.is_null() {
            let error = crate::platform_error::PlatformError::last_win32("CreateWindowExW(bar)");
            for bar in &result {
                if unsafe { DestroyWindow(bar.hwnd as _) } == 0 {
                    crate::platform_error::PlatformError::last_win32(
                        "DestroyWindow(bar creation cleanup)",
                    )
                    .report();
                }
            }
            return Err(error);
        }
        if let Err(error) = crate::bar_renderer::attach(hwnd) {
            crate::error_log::report("egui bar initialization", error);
            unsafe {
                DestroyWindow(hwnd);
                for bar in &result {
                    DestroyWindow(bar.hwnd as _);
                }
            }
            return Err(crate::platform_error::PlatformError::api_code(
                "egui bar initialization",
                -1,
            ));
        }
        unsafe {
            if ShowScrollBar(hwnd, SB_BOTH, 0) == 0 {
                crate::error_log::last_win32("ShowScrollBar(bar) failed");
            }
            if SetTimer(hwnd, 0, 1500, None) == 0 {
                let error = crate::platform_error::PlatformError::last_win32("SetTimer(bar)");
                if DestroyWindow(hwnd) == 0 {
                    crate::platform_error::PlatformError::last_win32(
                        "DestroyWindow(bar timer cleanup)",
                    )
                    .report();
                }
                for bar in &result {
                    if DestroyWindow(bar.hwnd as _) == 0 {
                        crate::platform_error::PlatformError::last_win32(
                            "DestroyWindow(bar creation cleanup)",
                        )
                        .report();
                    }
                }
                return Err(error);
            }
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            if windows_sys::Win32::Graphics::Gdi::UpdateWindow(hwnd) == 0 {
                crate::error_log::last_win32("UpdateWindow(bar) failed");
            }
        }
        result.push(BarWindow {
            hwnd: hwnd as isize,
            monitor_index: index,
        });
    }
    Ok(result)
}

pub fn redraw(bars: &[BarWindow]) {
    for bar in bars {
        unsafe {
            if windows_sys::Win32::Graphics::Gdi::InvalidateRect(bar.hwnd as _, std::ptr::null(), 0)
                == 0
            {
                crate::error_log::last_win32("InvalidateRect(bar redraw) failed");
            }
        }
    }
}

pub fn status_text(segments: &[crate::config::BarSegmentConfiguration]) -> String {
    segments
        .iter()
        .map(|segment| {
            let value = segment_value(segment.value);
            format!(
                "{}{}{:>width$}",
                segment.separator_text.as_deref().unwrap_or(""),
                segment.header_text.as_deref().unwrap_or(""),
                value,
                width = segment.variable_text_fixed_width
            )
        })
        .collect()
}

/// `rendered_segments`, reused for up to a second. The Claude animation repaints
/// every bar many times a second; the values themselves change far more slowly.
/// Every bar shows the same segments, and the regular 1500 ms repaint always
/// refreshes them.
pub fn cached_segments(
    segments: &[crate::config::BarSegmentConfiguration],
) -> Vec<RenderedSegment> {
    use std::time::{Duration, Instant};
    static CACHE: std::sync::Mutex<Option<(Instant, Vec<RenderedSegment>)>> =
        std::sync::Mutex::new(None);
    let mut cache = CACHE.lock().unwrap();
    match &*cache {
        Some((at, rendered)) if at.elapsed() < Duration::from_secs(1) => rendered.clone(),
        _ => {
            let rendered = rendered_segments(segments);
            *cache = Some((Instant::now(), rendered.clone()));
            rendered
        }
    }
}

pub fn rendered_segments(
    segments: &[crate::config::BarSegmentConfiguration],
) -> Vec<RenderedSegment> {
    segments
        .iter()
        .map(|segment| {
            let value = segment_value(segment.value);
            RenderedSegment {
                separator: segment
                    .separator_text
                    .clone()
                    .map(|text| (text, segment.separator_is_icon)),
                header: segment
                    .header_text
                    .clone()
                    .map(|text| (text, segment.header_is_icon)),
                tone: tone(segment.value, &value),
                value: format!(
                    "{:>width$}",
                    value,
                    width = segment.variable_text_fixed_width
                ),
                value_is_icon: segment.variable_is_icon,
            }
        })
        .collect()
}

fn segment_value(value: crate::config::BarSegmentValue) -> String {
    use crate::config::BarSegmentValue::*;
    match value {
        SystemTime => system_time(false),
        LocalTime => system_time(true),
        LocalDate => local_date(),
        Memory => memory_percent(),
        Cpu => cpu_percent(),
        Volume => crate::status::volume_percent(),
        Internet => crate::status::internet_icon(),
    }
}

fn system_time(local: bool) -> String {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    let mut time: SYSTEMTIME = unsafe { zeroed() };
    unsafe {
        if local {
            windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut time);
        } else {
            windows_sys::Win32::System::SystemInformation::GetSystemTime(&mut time);
        }
    }
    format!("{:02}:{:02}", time.wHour, time.wMinute)
}
fn local_date() -> String {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    let mut time: SYSTEMTIME = unsafe { zeroed() };
    unsafe {
        windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut time);
    }
    format!("{:04}-{:02}-{:02}", time.wYear, time.wMonth, time.wDay)
}
fn memory_percent() -> String {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut value: MEMORYSTATUSEX = unsafe { zeroed() };
    value.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    unsafe {
        if GlobalMemoryStatusEx(&mut value) == 0 {
            crate::platform_error::PlatformError::last_win32("GlobalMemoryStatusEx").report();
            return "  ?%".into();
        }
    }
    format!("{:3}%", value.dwMemoryLoad)
}

fn cpu_percent() -> String {
    use std::mem::zeroed;
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::GetSystemTimes;
    static STATE: Mutex<(u64, u64, u64, i32, Option<std::time::Instant>)> =
        Mutex::new((0, 0, 0, 0, None));
    let mut state = STATE.lock().unwrap();
    if state
        .4
        .is_some_and(|last| last.elapsed() < std::time::Duration::from_secs(1))
    {
        return format!("{:3}%", state.3);
    }
    state.4 = Some(std::time::Instant::now());
    let mut idle: FILETIME = unsafe { zeroed() };
    let mut kernel: FILETIME = unsafe { zeroed() };
    let mut user: FILETIME = unsafe { zeroed() };
    if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
        crate::error_log::last_win32("GetSystemTimes failed");
        return "  0%".into();
    }
    let value = crate::process::filetime_ticks;
    let (idle, kernel, user) = (value(idle), value(kernel), value(user));
    let total = kernel.saturating_sub(state.1) + user.saturating_sub(state.2);
    let result = if state.1 != 0 && total != 0 {
        ((total.saturating_sub(idle.saturating_sub(state.0))) * 100 / total) as i32
    } else {
        state.3
    };
    if (0..=100).contains(&result) {
        state.3 = result;
    }
    state.0 = idle;
    state.1 = kernel;
    state.2 = user;
    format!("{:3}%", state.3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unfocused_bars_dim_a_step_further_when_set_to() {
        let mut snapshot = crate::bar_renderer::tests::snapshot();
        assert_eq!(Shade::of(&snapshot), Shade::Focused);
        snapshot.selected_monitor = false;
        assert_eq!(Shade::of(&snapshot), Shade::Dimmed);
        snapshot.focus_style.dim_unfocused = false;
        assert_eq!(Shade::of(&snapshot), Shade::Unfocused);
        assert_eq!(Shade::Dimmed.content(), Shade::Unfocused.context());
        assert_eq!(Shade::Unfocused.mark(palette::CRITICAL), palette::CRITICAL);
        assert_ne!(Shade::Dimmed.mark(palette::CRITICAL), palette::CRITICAL);
    }

    #[test]
    fn values_read_as_warning_critical_or_off() {
        use crate::config::BarSegmentValue::*;
        assert_eq!(tone(Cpu, " 74%"), Tone::Normal);
        assert_eq!(tone(Cpu, " 75%"), Tone::Warning);
        assert_eq!(tone(Memory, " 90%"), Tone::Critical);
        assert_eq!(tone(Memory, "  ?%"), Tone::Normal);
        assert_eq!(tone(Volume, "  0%"), Tone::Off);
        assert_eq!(tone(Volume, " 30%"), Tone::Normal);
        assert_eq!(
            tone(Internet, crate::status::INTERNET_OFFLINE),
            Tone::Critical
        );
        assert_eq!(
            tone(Internet, crate::status::INTERNET_UNKNOWN),
            Tone::Warning
        );
        assert_eq!(
            tone(Internet, crate::status::INTERNET_CONNECTED),
            Tone::Normal
        );
        assert_eq!(tone(LocalTime, "99"), Tone::Normal);
    }

    /// Runs `paint` with a painter whose fonts include the bar font.
    fn with_painter(mut paint: impl FnMut(&egui::Painter, &egui::FontId)) {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::bar_renderer::bar_fonts(""));
        let font = egui::FontId::new(12.0, egui::FontFamily::Name("bar".into()));
        ctx.run_ui(egui::RawInput::default(), |ui| paint(ui.painter(), &font))
            .drop_without_applying_deltas();
    }

    #[test]
    fn bar_separators_become_dividers_between_segments_only() {
        let segment = |separator: &str| RenderedSegment {
            separator: Some((separator.into(), false)),
            header: Some(("CPU:".into(), false)),
            value: " 5%".into(),
            value_is_icon: false,
            tone: Tone::Normal,
        };
        with_painter(|painter, font| {
            let kinds = |segments: &[RenderedSegment]| {
                status_pieces(painter, segments, font, font, Shade::Focused)
                    .iter()
                    .map(|piece| match piece {
                        Piece::Text(galley) => galley.text().to_owned(),
                        Piece::Gap(_) => "gap".into(),
                        Piece::Divider => "divider".into(),
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                kinds(&[segment(" | "), segment(" | ")]),
                [
                    "CPU:", "gap", "5%", "gap", "divider", "CPU:", "gap", "5%", "gap"
                ]
            );
            assert_eq!(
                kinds(&[segment(""), segment(" - ")]),
                [
                    " - ", "CPU:", "gap", "5%", "gap", "gap", "CPU:", "gap", "5%", "gap"
                ]
            );
        });
    }

    #[test]
    fn values_start_the_same_distance_from_their_header_and_keep_their_width() {
        let segment = |value: &str| RenderedSegment {
            separator: None,
            header: Some(("{f028} ".into(), true)),
            value: value.into(),
            value_is_icon: false,
            tone: Tone::Normal,
        };
        with_painter(|painter, font| {
            // The header's width plus the gap, and the whole segment's width.
            let measure = |value: &str| {
                let pieces = status_pieces(painter, &[segment(value)], font, font, Shade::Focused);
                let before_value = pieces_width(&pieces[..2]);
                (before_value, pieces_width(&pieces))
            };
            let (seven, hundred) = (measure("  7%"), measure("100%"));
            assert_eq!(seven.0, hundred.0);
            assert!((seven.1 - hundred.1).abs() < 0.5, "{seven:?} {hundred:?}");
        });
    }

    #[test]
    fn long_text_ends_in_an_ellipsis_within_its_width() {
        with_painter(|painter, font| {
            let title = one_line(
                painter,
                &"Long title ".repeat(50),
                font,
                palette::TEXT,
                120.0,
            );
            assert!(title.size().x <= 120.0 + 1.0, "{}", title.size().x);
            // The galley keeps the whole text; its one row is what is drawn.
            let drawn = title.rows[0].text();
            assert_eq!(title.rows.len(), 1);
            assert!(drawn.ends_with('…'), "{drawn}");
        });
    }

    #[test]
    fn workspace_hit_targets_use_the_rendered_pixel_layout() {
        let mut snapshot = crate::bar_renderer::tests::snapshot();
        snapshot.tags[0].0 = 10;
        snapshot.tags[1].0 = 20;
        assert_eq!(workspace_at_x(&snapshot, 15), None);
        assert_eq!(workspace_at_x(&snapshot, 16), Some(10));
        assert_eq!(workspace_at_x(&snapshot, 57), Some(10));
        assert_eq!(workspace_at_x(&snapshot, 58), Some(20));
        assert_eq!(workspace_at_x(&snapshot, 142), None);
    }
}
