//! A Claude Code session as a document picker: one row per prompt, newest first,
//! and the whole conversation in the preview with the assistant's markdown laid
//! out. The Agents mailbox opens it for the entry pressed; see `ClaudeOptions`.
use crate::document_picker::{DocumentItem, DocumentPicker};
use crate::markdown_document::{BOLD, DocumentWriter, FAINT, Theme};
use nfm_egui::egui::Color32;
use nfm_egui::styled_document::{DocumentRange, Position, Style};
use serde_json::Value;
use std::path::Path;

/// How the Agents mailbox shows a session.
#[derive(Clone, Debug)]
pub struct ClaudeOptions {
    /// The assistant's markdown.
    pub theme: Theme,
    /// The selected prompt's background in the preview.
    pub highlight: Color32,
    pub you: Color32,
    pub claude: Color32,
    /// The session opens filling the picker monitor up to the bar, rather than
    /// centered; Ctrl+Shift+F switches between the two. Off by default.
    pub full_pane: bool,
}
impl Default for ClaudeOptions {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            highlight: Color32::from_rgb(0x3c, 0x38, 0x36),
            you: Color32::from_rgb(0x8e, 0xc0, 0x7c),
            claude: Color32::from_rgb(0xd9, 0x77, 0x57),
            full_pane: false,
        }
    }
}

/// The picker for the transcript at `path`.
pub(super) fn picker(
    path: &Path,
    title: String,
    options: &ClaudeOptions,
) -> Result<DocumentPicker, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("reading {}: {error}", path.display()))?;
    let (document, mut items) = conversation(&text, options);
    items.reverse();
    Ok(DocumentPicker {
        title,
        column: "Prompt".into(),
        document: document.finish(),
        items,
        highlight: options.highlight,
        full_pane: options.full_pane,
    })
}

#[derive(PartialEq)]
enum Speaker {
    You,
    Claude,
}

/// The conversation in the transcript's lines, and a range per prompt, oldest first.
/// Subagents' messages, tool results and Claude's own notes are left out.
fn conversation(transcript: &str, options: &ClaudeOptions) -> (DocumentWriter, Vec<DocumentItem>) {
    let mut out = DocumentWriter::default();
    let mut items = Vec::new();
    let mut last = None;
    let faint = Style {
        flags: FAINT,
        ..Default::default()
    };
    for line in transcript.lines() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if entry["isSidechain"] == true || entry["isMeta"] == true {
            continue;
        }
        let content = &entry["message"]["content"];
        match entry["type"].as_str() {
            Some("user") => {
                // Claude delivers task notifications as messages too; newer
                // transcripts mark who a message came from.
                let from_person = entry["origin"]["kind"]
                    .as_str()
                    .is_none_or(|kind| kind == "human");
                let Some(prompt) = prompt(content).filter(|_| from_person) else {
                    continue;
                };
                out.blank_line();
                let start = out.line();
                speaker(&mut out, "You", options.you);
                for text in prompt.lines() {
                    out.line_of(text, Style::default());
                }
                items.push(DocumentItem {
                    label: label(&prompt),
                    range: DocumentRange {
                        start: Position {
                            line: start,
                            byte: 0,
                        },
                        end: Position {
                            line: out.line() - 1,
                            byte: out.last_line_len().saturating_sub(1),
                        },
                    },
                });
                last = Some(Speaker::You);
            }
            Some("assistant") => {
                let Some(blocks) = content.as_array() else {
                    continue;
                };
                for block in blocks {
                    let text = block["text"].as_str().filter(|t| !t.trim().is_empty());
                    let tool =
                        (block["type"] == "tool_use").then(|| super::sessions::tool_summary(block));
                    if text.is_none() && tool.is_none() {
                        continue;
                    }
                    if last != Some(Speaker::Claude) {
                        out.blank_line();
                        speaker(&mut out, "Claude", options.claude);
                        last = Some(Speaker::Claude);
                    }
                    match (text, tool) {
                        (Some(text), _) => {
                            out.markdown(text, &options.theme);
                        }
                        (None, Some(tool)) => out.line_of(&format!("→ {tool}"), faint),
                        (None, None) => {}
                    }
                }
            }
            _ => {}
        }
    }
    (out, items)
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

/// The text of a prompt the person typed: a string, or text blocks without tool
/// results. A slash command shows as its name and arguments, and its output and
/// Claude's notes are left out.
fn prompt(content: &Value) -> Option<String> {
    let text = match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => {
            if blocks.iter().any(|block| block["type"] == "tool_result") {
                return None;
            }
            blocks
                .iter()
                .filter(|block| block["type"] == "text")
                .filter_map(|block| block["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        }
        _ => return None,
    };
    let text = text.trim();
    if let Some(command) = tagged(text, "command-name") {
        let args = tagged(text, "command-args").unwrap_or_default();
        return Some(format!("{command} {args}").trim().to_owned());
    }
    if text.is_empty()
        || text.starts_with("<local-command")
        || text.starts_with("<system-reminder>")
    {
        return None;
    }
    Some(text.to_owned())
}

/// The text between `<tag>` and `</tag>` in `text`.
fn tagged<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let start = text.find(&format!("<{tag}>"))? + tag.len() + 2;
    let end = text[start..].find(&format!("</{tag}>"))? + start;
    Some(text[start..end].trim())
}

/// A prompt's first line, cut short.
fn label(prompt: &str) -> String {
    let line = prompt
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default();
    match line.char_indices().nth(200) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transcript(lines: &[&str]) -> String {
        lines.join("\n")
    }

    #[test]
    fn prompts_and_replies_alternate_and_each_prompt_is_an_item() {
        let text = transcript(&[
            r#"{"type":"user","message":{"content":"fix the bar\nplease"}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Looking at **it**."}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"src/bar.rs"}}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"ok"}]}}"#,
            r#"{"type":"assistant","isSidechain":true,"message":{"content":[{"type":"text","text":"subagent"}]}}"#,
            r#"{"type":"user","isMeta":true,"message":{"content":"caveat"}}"#,
            r#"{"type":"user","origin":{"kind":"task-notification"},"message":{"content":"<task-notification>done</task-notification>"}}"#,
            r#"{"type":"user","message":{"content":"<command-name>/clear</command-name><command-args></command-args>"}}"#,
        ]);
        let (out, items) = conversation(&text, &ClaudeOptions::default());
        assert_eq!(
            out.texts(),
            [
                "▍You",
                "fix the bar",
                "please",
                "",
                "▍Claude",
                "Looking at it.",
                "",
                "→ Edit src/bar.rs",
                "",
                "▍You",
                "/clear",
            ]
        );
        let labels: Vec<_> = items.iter().map(|item| item.label.as_str()).collect();
        assert_eq!(labels, ["fix the bar", "/clear"]);
        let first = &items[0].range;
        assert_eq!((first.start.line, first.end.line), (0, 2));
        assert_eq!(first.end.byte, "please".len() - 1);
        assert_eq!(
            (items[1].range.start.line, items[1].range.end.line),
            (9, 10)
        );
    }

    #[test]
    fn notes_and_command_output_are_not_prompts() {
        let string = |text: &str| Value::String(text.into());
        assert_eq!(
            prompt(&string("<local-command-stdout>x</local-command-stdout>")),
            None
        );
        assert_eq!(prompt(&string("  ")), None);
        assert_eq!(
            prompt(&string(
                "<command-name>/model</command-name><command-args>opus</command-args>"
            )),
            Some("/model opus".into())
        );
        let blocks = serde_json::json!([{"type":"text","text":"a"},{"type":"text","text":"b"}]);
        assert_eq!(prompt(&blocks), Some("a\nb".into()));
    }

    #[test]
    fn labels_are_the_first_line_cut_short() {
        assert_eq!(label("\n  \nfirst\nsecond"), "first");
        let long = "x".repeat(250);
        assert_eq!(label(&long).chars().count(), 201);
    }
}
