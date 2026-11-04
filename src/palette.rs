//! The colors the bar and its drop-downs draw with, named by role: gruvbox dark.
use nfm_egui::egui::Color32;

/// Regions, the drop-down, and text on `ACCENT`.
pub const BACKGROUND: Color32 = Color32::from_rgb(0x28, 0x28, 0x28);
/// Hovered and selected rows; dividers inside lists.
pub const HOVER: Color32 = Color32::from_rgb(0x3c, 0x38, 0x36);
/// Borders, and dividers between a region's items.
pub const BORDER: Color32 = Color32::from_rgb(0x50, 0x49, 0x45);
/// The selected workspace on a monitor that isn't focused.
pub const INACTIVE: Color32 = Color32::from_rgb(0x66, 0x5c, 0x54);
/// Secondary details: separators, process ids, values that are off.
pub const FAINT: Color32 = Color32::from_rgb(0x92, 0x83, 0x74);
/// Context: headers, sources, and text on a monitor that isn't focused.
pub const MUTED: Color32 = Color32::from_rgb(0xa8, 0x99, 0x84);
pub const TEXT: Color32 = Color32::from_rgb(0xeb, 0xdb, 0xb2);
/// What to read first, such as an entry's title.
pub const TITLE: Color32 = Color32::from_rgb(0xfb, 0xf1, 0xc7);
/// The focused monitor's workspaces, and the focused window's outline.
pub const ACCENT: Color32 = Color32::from_rgb(0x83, 0xa5, 0x98);
pub const WARNING: Color32 = Color32::from_rgb(0xfa, 0xbd, 0x2f);
pub const CRITICAL: Color32 = Color32::from_rgb(0xfb, 0x49, 0x34);
