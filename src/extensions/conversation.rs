//! A conversation with an agent as a document picker: one row per prompt, newest
//! first, and the whole conversation in the preview with the agent's markdown
//! laid out. The viewers of each agent's transcripts (`claude`, `codex`) read
//! their own format and write what was said here.
use crate::document_picker::{DocumentItem, DocumentPicker};
use crate::markdown_document::{BOLD, DocumentWriter, FAINT, Theme};
use nfm_egui::egui::Color32;
use nfm_egui::styled_document::{DocumentRange, Position, Style};

/// A conversation being written, oldest message first.
pub struct Conversation {
    out: DocumentWriter,
    items: Vec<DocumentItem>,
    /// Whether the agent spoke last, so its next message goes under the same name.
    agent_spoke: bool,
    agent: String,
    agent_color: Color32,
    you: Color32,
    theme: Theme,
}
impl Conversation {
    /// `agent` is the name over the agent's messages; `theme` lays out its markdown.
    pub fn new(agent: impl Into<String>, agent_color: Color32, you: Color32, theme: Theme) -> Self {
        Self {
            out: DocumentWriter::default(),
            items: Vec::new(),
            agent_spoke: false,
            agent: agent.into(),
            agent_color,
            you,
            theme,
        }
    }

    /// What the person typed. Each prompt is a row of the picker.
    pub fn prompt(&mut self, prompt: &str) {
        self.out.blank_line();
        let start = self.out.line();
        speaker(&mut self.out, "You", self.you);
        for text in prompt.lines() {
            self.out.line_of(text, Style::default());
        }
        self.items.push(DocumentItem {
            label: label(prompt),
            range: DocumentRange {
                start: Position {
                    line: start,
                    byte: 0,
                },
                end: Position {
                    line: self.out.line() - 1,
                    byte: self.out.last_line_len().saturating_sub(1),
                },
            },
        });
        self.agent_spoke = false;
    }

    /// What the agent said, as markdown.
    pub fn reply(&mut self, markdown: &str) {
        self.agent_speaks();
        self.out.markdown(markdown, &self.theme);
    }

    /// One faint line for something the agent did, such as `→ Edit src/bar.rs`.
    pub fn action(&mut self, text: &str) {
        self.agent_speaks();
        let faint = Style {
            flags: FAINT,
            ..Default::default()
        };
        self.out.line_of(text, faint);
    }

    fn agent_speaks(&mut self) {
        if !self.agent_spoke {
            self.out.blank_line();
            speaker(&mut self.out, &self.agent, self.agent_color);
            self.agent_spoke = true;
        }
    }

    /// The picker over the conversation. `highlight` is the selected prompt's
    /// background in the preview; `full_pane` opens it filling the monitor.
    pub fn picker(mut self, title: String, highlight: Color32, full_pane: bool) -> DocumentPicker {
        self.items.reverse();
        DocumentPicker {
            title,
            column: "Prompt".into(),
            document: self.out.finish(),
            items: self.items,
            highlight,
            full_pane,
        }
    }

    /// The lines written and the prompts' rows, oldest first.
    #[cfg(test)]
    pub(crate) fn written(&self) -> (Vec<String>, &[DocumentItem]) {
        (self.out.texts(), &self.items)
    }
}

fn speaker(out: &mut DocumentWriter, name: &str, color: Color32) {
    out.finish_line();
    out.push(
        "▍",
        Style {
            foreground: Some(color),
            ..Default::default()
        },
    );
    out.push(
        name,
        Style {
            foreground: Some(color),
            flags: BOLD,
            ..Default::default()
        },
    );
    out.end_line();
}

/// A prompt's first line, cut short.
fn label(prompt: &str) -> String {
    first_line(prompt, 200)
}

/// The first line of `text` with anything on it, cut to `length` characters.
pub fn first_line(text: &str, length: usize) -> String {
    let line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default();
    match line.char_indices().nth(length) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_the_first_line_cut_short() {
        assert_eq!(label("\n  \nfirst\nsecond"), "first");
        let long = "x".repeat(250);
        assert_eq!(label(&long).chars().count(), 201);
    }

    #[test]
    fn the_agents_messages_share_a_name_until_the_next_prompt() {
        let mut conversation =
            Conversation::new("Codex", Color32::WHITE, Color32::WHITE, Theme::default());
        conversation.prompt("one");
        conversation.action("→ ran it");
        conversation.reply("done");
        conversation.prompt("two");
        let (texts, items) = conversation.written();
        let names: Vec<_> = texts.iter().filter(|line| line.starts_with('▍')).collect();
        assert_eq!(names, ["▍You", "▍Codex", "▍You"]);
        assert_eq!(items.len(), 2);
        let picker = conversation.picker("t".into(), Color32::BLACK, true);
        let labels: Vec<_> = picker
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(labels, ["two", "one"]);
    }
}
