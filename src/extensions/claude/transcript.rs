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
                if let Some(prompt) = typed_prompt(&entry) {
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

/// How much of the end of a transcript is read for an interrupt.
const INTERRUPT_TAIL_BYTES: u64 = 64 * 1024;

/// `Interrupted`, if the transcript at `path` ends with a turn the person
/// stopped at or after `since`, which is when its mailbox entry last started
/// running or waiting.
pub(super) fn interrupted(path: &Path, since: std::time::SystemTime) -> Option<String> {
    let text = crate::extensions::spool::tail(path, INTERRUPT_TAIL_BYTES).ok()?;
    let since = since.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    interrupted_since(&text, since).then(|| "Interrupted".to_owned())
}

/// Whether the transcript's last message is Claude Code's note that the person
/// interrupted the turn, written at or after `since` (seconds since 1970). An
/// earlier note belongs to a turn before the one the entry is running now.
fn interrupted_since(transcript: &str, since: u64) -> bool {
    for line in transcript.lines().rev() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if entry["isSidechain"] == true {
            continue;
        }
        match entry["type"].as_str() {
            // Claude has answered since any interrupt.
            Some("assistant") => return false,
            Some("user") => {}
            // Records of the session itself, such as its mode.
            _ => continue,
        }
        let content = &entry["message"]["content"];
        let text = content.as_str().or_else(|| content[0]["text"].as_str());
        if text.is_some_and(|text| text.starts_with("[Request interrupted by user")) {
            return entry["timestamp"]
                .as_str()
                .and_then(nfm_picker_sources::value::parse_timestamp)
                .and_then(|time| u64::try_from(time.timestamp()).ok())
                .is_some_and(|time| time >= since);
        }
    }
    false
}

/// The prompt in a `user` entry, if a person typed it. Claude delivers task
/// notifications as messages too; newer transcripts mark who a message came
/// from.
pub(super) fn typed_prompt(entry: &Value) -> Option<String> {
    let from_person = entry["origin"]["kind"]
        .as_str()
        .is_none_or(|kind| kind == "human");
    prompt(&entry["message"]["content"]).filter(|_| from_person)
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
    fn a_turn_is_interrupted_when_the_note_of_it_is_the_last_message_since_its_prompt() {
        const PROMPT: &str = r#"{"type":"user","timestamp":"2026-11-12T05:45:25.100Z","message":{"content":"resync"}}"#;
        const REPLY: &str = r#"{"type":"assistant","timestamp":"2026-11-12T05:45:28Z","message":{"content":[{"type":"text","text":"Looking."}]}}"#;
        const NOTE: &str = r#"{"type":"user","isSidechain":false,"timestamp":"2026-11-12T05:45:31.385Z","message":{"role":"user","content":[{"type":"text","text":"[Request interrupted by user]"}]}}"#;
        const MODE: &str = r#"{"type":"mode","mode":"normal"}"#;
        const SUBAGENT: &str = r#"{"type":"assistant","isSidechain":true,"timestamp":"2026-11-12T05:45:40Z","message":{"content":[{"type":"text","text":"still going"}]}}"#;
        let noted = nfm_picker_sources::value::parse_timestamp("2026-11-12T05:45:31Z")
            .unwrap()
            .timestamp() as u64;
        let prompted = noted - 6;
        let interrupted = |lines: &[&str], since| interrupted_since(&transcript(lines), since);

        assert!(interrupted(&[PROMPT, REPLY, NOTE], prompted));
        // The session's own records and a subagent's messages may follow it.
        assert!(interrupted(&[PROMPT, NOTE, MODE, SUBAGENT, "{"], prompted));
        assert!(interrupted(&[NOTE], noted));
        // Stopped while a tool was asking for permission.
        let tool = NOTE.replace("by user]", "by user for tool use]");
        assert!(interrupted(&[PROMPT, &tool], prompted));

        // Still working, or finished.
        assert!(!interrupted(&[PROMPT, REPLY], prompted));
        assert!(!interrupted(&[], prompted));
        // Claude answered after it.
        assert!(!interrupted(&[PROMPT, NOTE, REPLY], prompted));
        // The entry started running after it: a turn before this one was interrupted.
        assert!(!interrupted(&[PROMPT, NOTE], noted + 1));
        assert!(!interrupted(&[PROMPT, NOTE, PROMPT], noted + 30));
        // A prompt that only quotes the note.
        let quoted = PROMPT.replace("resync", "why [Request interrupted by user]?");
        assert!(!interrupted(&[&quoted], 0));
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
