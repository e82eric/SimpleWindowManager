//! Claude Code pieces for a bar mailbox: the picker that shows a session's
//! conversation, a row per prompt with Claude's markdown laid out in the preview,
//! and Claude's spark as an icon.
//!
//! Sessions reach the bar through Claude Code's hooks, which write entries to a
//! mailbox folder (`extensions::spool`, and `mailbox\agent-hook.ps1`) with
//! `"kind": "claude"`. The folder gives those entries the spark and this picker
//! once the configuration adds `kind` to it.
//!
//! `grep_command` is `GrepClaudeSessions` for the commands picker: it asks for
//! text, searches every conversation for it and lists the messages containing
//! it to fuzzy-find among. `configuration.picker_command(claude::grep_command(options))`.
mod grep;
mod transcript;

pub use grep::command as grep_command;
pub use transcript::ClaudeOptions;

use nfm_egui::egui::{self, Color32};

/// Claude Code sessions as a kind of entry in a mailbox folder: entries with
/// `"kind": "claude"` have the spark as their badges, in Claude's color while
/// running, and open as the session's conversation.
/// `SpoolOptions::named("Agents").kind(claude::kind(options))`.
pub fn kind(options: ClaudeOptions) -> crate::extensions::spool::Kind {
    crate::extensions::spool::Kind::new("claude", spark, options.claude)
        .viewer(move |path, title| transcript::picker(path, title, &options))
}

/// Claude's spark: uneven rays around a center, filling `area`. Pass it to
/// `Icon::drawn` to use it in a state's style.
pub fn spark(painter: &egui::Painter, area: egui::Rect, color: Color32) {
    const RAYS: [f32; 12] = [
        1.0, 0.72, 0.9, 0.65, 1.0, 0.78, 0.88, 0.62, 0.95, 0.7, 0.85, 0.68,
    ];
    let center = area.center();
    let radius = area.width().min(area.height()) / 2.0;
    let stroke = egui::Stroke::new((radius * 2.0 * 0.13).max(1.2), color);
    for (index, &length) in RAYS.iter().enumerate() {
        let angle =
            index as f32 * std::f32::consts::TAU / RAYS.len() as f32 - std::f32::consts::FRAC_PI_2;
        let end = center + egui::vec2(angle.cos(), angle.sin()) * radius * length;
        painter.line_segment([center, end], stroke);
    }
    painter.circle_filled(center, stroke.width * 0.8, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spark_rays_stay_within_their_area() {
        let ctx = egui::Context::default();
        for size in [7.2, 16.0] {
            let area = egui::Rect::from_center_size(egui::pos2(50.0, 20.0), egui::vec2(size, size));
            let output = ctx.run_ui(egui::RawInput::default(), |ui| {
                spark(ui.painter(), area, Color32::WHITE);
            });
            let ends: Vec<f32> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::LineSegment { points, .. } => {
                        Some(points[1].distance(area.center()))
                    }
                    _ => None,
                })
                .collect();
            output.drop_without_applying_deltas();
            assert_eq!(ends.len(), 12);
            let longest = ends.iter().cloned().fold(0.0, f32::max);
            assert!((longest - size / 2.0).abs() < 1e-3, "{longest} at {size}");
        }
    }
}
