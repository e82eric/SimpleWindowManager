//! A Claude Code session as a document picker: one row per prompt, newest first,
//! and the whole conversation in the preview with the assistant's markdown laid
//! out. A mailbox folder opens it for the entry pressed; see `ClaudeOptions`.
use crate::document_picker::DocumentPicker;
use crate::extensions::conversation::Conversation;
use crate::markdown_document::Theme;
use nfm_egui::egui::Color32;
use serde_json::Value;
use std::path::Path;

/// How a session is shown.
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
    Ok(conversation(&text, options).picker(title, options.highlight, options.full_pane))
}

/// The conversation in the transcript's lines. Subagents' messages, tool results
/// and Claude's own notes are left out.
fn conversation(transcript: &str, options: &ClaudeOptions) -> Conversation {
    let mut out = Conversation::new("Claude", options.claude, options.you, options.theme.clone());
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
                if let Some(prompt) = prompt(content).filter(|_| from_person) {
                    out.prompt(&prompt);
                }
            }
            Some("assistant") => {
                for block in content.as_array().into_iter().flatten() {
                    if let Some(text) = block["text"].as_str().filter(|t| !t.trim().is_empty()) {
                        out.reply(text);
                    } else if block["type"] == "tool_use" {
                        out.action(&format!("→ {}", tool_summary(block)));
                    }
                }
            }
            _ => {}
        }
    }
    out
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

/// A tool call as its name and what it acted on: `Edit src/bar.rs`.
fn tool_summary(block: &Value) -> String {
    let name = block["name"].as_str().unwrap_or("Tool");
    let input = &block["input"];
    let target = [
        "file_path",
        "command",
        "pattern",
        "path",
        "url",
        "description",
    ]
    .iter()
    .find_map(|key| input[*key].as_str())
    .unwrap_or_default();
    // A multi-line command shows its first line.
    let target = target.lines().next().unwrap_or_default();
    format!("{name} {target}").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transcript(lines: &[&str]) -> String {
        lines.join("\n")
    }

    #[test]
    fn a_tool_call_is_its_name_and_the_first_line_of_what_it_acted_on() {
        let call = |json: &str| tool_summary(&serde_json::from_str(json).unwrap());
        assert_eq!(
            call(r#"{"name":"Edit","input":{"file_path":"src/bar.rs"}}"#),
            "Edit src/bar.rs"
        );
        assert_eq!(
            call(r#"{"name":"Bash","input":{"command":"cargo test\ncargo fmt"}}"#),
            "Bash cargo test"
        );
        assert_eq!(call(r#"{"input":{}}"#), "Tool");
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
        let conversation = conversation(&text, &ClaudeOptions::default());
        let (texts, items) = conversation.written();
        assert_eq!(
            texts,
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
}
