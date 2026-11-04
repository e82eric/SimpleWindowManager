//! Claude Code sessions in a bar mailbox: running, waiting for you, and finished
//! in the last 2 days. Opening an entry shows the session's conversation in a
//! picker, a row per prompt, with Claude's markdown laid out in the preview.
//!
//! ```ignore
//! use crate::extensions::claude::{self, ClaudeOptions};
//! claude::register(configuration, ClaudeOptions::default());
//! ```
//!
//! `register` returns the mailbox, so the configuration can restyle a state with
//! `.state(claude::RUNNING, ...)`. To build the mailbox yourself, pass `producer`
//! or `producer_with(options)` to `Configuration::bar_mailbox`.
mod sessions;
mod transcript;

pub use sessions::{DONE, DONE_FOR, RUNNING, WAITING, producer, producer_with};
pub use transcript::ClaudeOptions;

use crate::mailbox::{Animation, BarMailbox, Icon, StateStyle};
use nfm_egui::egui::{self, Color32};

/// The mailbox's name, for `Action::ShowMailbox`.
pub const MAILBOX: &str = "Agents";

/// Adds the `MAILBOX` mailbox to the bar, with a header and a style per state:
/// a growing and shrinking `spark` while running, a pulsing dot while waiting, and
/// a check when done.
pub fn register(
    configuration: &mut crate::Configuration,
    options: ClaudeOptions,
) -> &mut BarMailbox {
    configuration
        .bar_mailbox(MAILBOX, producer_with(options))
        .header("Agents:")
        .state(
            RUNNING,
            StateStyle::icon(
                Icon::drawn(spark, Color32::from_rgb(0xd9, 0x77, 0x57)).animation(
                    Animation::Scale {
                        min: 0.45,
                        max: 1.0,
                        period: 1.2,
                    },
                ),
            ),
        )
        .state(
            WAITING,
            StateStyle::icon(
                Icon::glyph("●", Color32::from_rgb(0xfb, 0x49, 0x34)).animation(Animation::Pulse {
                    min: 0.4,
                    period: 1.0,
                }),
            ),
        )
        .state(
            DONE,
            StateStyle::icon(Icon::glyph("✓", Color32::from_rgb(0xb8, 0xbb, 0x26))),
        )
}

/// Claude's spark, the running state's icon: uneven rays around a center, filling
/// `area`. Pass it to `Icon::drawn` to use it in a style of your own.
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

    #[test]
    fn register_adds_the_mailbox_with_every_state_and_restyling_replaces_one() {
        let mut configuration = crate::Configuration::default();
        let red = StateStyle::icon(Icon::glyph("!", Color32::RED));
        register(&mut configuration, ClaudeOptions::default()).state(RUNNING, red);
        let mailbox = &configuration.bar_mailboxes[0];
        assert_eq!(mailbox.name, MAILBOX);
        assert_eq!(mailbox.header.as_deref(), Some("Agents:"));
        let states: Vec<_> = mailbox
            .states
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(states, [RUNNING, WAITING, DONE]);
        let running = mailbox.style(RUNNING).and_then(|style| style.icon.as_ref());
        assert_eq!(running.map(|icon| icon.color), Some(Color32::RED));
    }
}
