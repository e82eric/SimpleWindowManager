//! A Codex session as a document picker: one row per prompt, newest first, and
//! the whole conversation in the preview with Codex's markdown laid out.
//!
//! Sessions reach the bar through Codex's hooks, which write entries to a mailbox
//! folder (`extensions::spool`, and `mailbox\agent-hook.ps1`) with
//! `"kind": "codex"`. The folder gives those entries badges of their own and this
//! picker once the configuration adds `kind` to it:
//!
//! ```ignore
//! use crate::extensions::{codex, spool};
//! spool::register_with(
//!     configuration,
//!     spool::SpoolOptions::named("Agents").kind(codex::kind(Default::default())),
//! );
//! ```
//!
//! The session is read from the file Codex keeps of it, a "rollout" under
//! `~/.codex/sessions`. That file is Codex's own and its format is not
//! documented, so only the records known here are read and anything else is
//! passed over: a Codex that writes them differently shows less, not an error.
use crate::document_picker::DocumentPicker;
use crate::extensions::conversation::{Conversation, first_line};
use crate::extensions::spool::{Kind, ring};
use crate::markdown_document::Theme;
use nfm_egui::egui::Color32;
use serde_json::Value;
use std::path::Path;

/// How a session is shown.
#[derive(Clone, Debug)]
pub struct CodexOptions {
    /// Codex's markdown.
    pub theme: Theme,
    /// The selected prompt's background in the preview.
    pub highlight: Color32,
    pub you: Color32,
    pub codex: Color32,
    /// The session opens filling the picker monitor up to the bar, rather than
    /// centered; Ctrl+Shift+F switches between the two. Off by default.
    pub full_pane: bool,
}
impl Default for CodexOptions {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            highlight: Color32::from_rgb(0x3c, 0x38, 0x36),
            you: Color32::from_rgb(0x8e, 0xc0, 0x7c),
            codex: crate::palette::ACCENT,
            full_pane: false,
        }
    }
}

/// Codex sessions as a kind of entry in a mailbox folder: entries with
/// `"kind": "codex"` have a ring as their badges, in Codex's color while running,
/// and open as the session's conversation.
pub fn kind(options: CodexOptions) -> Kind {
    Kind::new("codex", ring, options.codex).viewer(move |path, title| picker(path, title, &options))
}

/// The picker for the rollout at `path`.
fn picker(path: &Path, title: String, options: &CodexOptions) -> Result<DocumentPicker, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("reading {}: {error}", path.display()))?;
    Ok(conversation(&text, options).picker(title, options.highlight, options.full_pane))
}

/// How much of a command or a search is shown, in characters.
const ACTION_LENGTH: usize = 160;

/// The conversation in the rollout's lines: the prompts, what Codex said, and a
/// line for each command it ran, file it changed and search it made. Its
/// instructions, reasoning and the commands' output are left out.
fn conversation(rollout: &str, options: &CodexOptions) -> Conversation {
    let mut out = Conversation::new("Codex", options.codex, options.you, options.theme.clone());
    // A message is a completed item, or before September 2026 an event of its
    // own. Should a Codex write both, the second of a pair is passed over.
    let mut last_message = String::new();
    let mut say = |out: &mut Conversation, from_you: bool, message: &str| {
        let message = message.trim();
        if message.is_empty() || (message == last_message) {
            return;
        }
        if from_you {
            out.prompt(message);
        } else {
            out.reply(message);
        }
        last_message = message.to_owned();
    };
    for line in rollout.lines() {
        // Most of a rollout is instructions, tool output and token counts: only
        // the lines that can be one of the events below are worth parsing.
        const EVENTS: [&str; 4] = [
            "\"item_completed\"",
            "\"user_message\"",
            "\"agent_message\"",
            "\"turn_aborted\"",
        ];
        if !EVENTS.iter().any(|name| line.contains(name)) {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let event = &entry["payload"];
        if entry["type"] != "event_msg" {
            continue;
        }
        let message = event["message"].as_str().unwrap_or_default();
        match event["type"].as_str() {
            Some("turn_aborted") => out.action("■ Interrupted"),
            Some("user_message") => say(&mut out, true, message),
            Some("agent_message") => say(&mut out, false, message),
            _ => {}
        }
        if event["type"] != "item_completed" {
            continue;
        }
        let item = &event["item"];
        match item["type"].as_str() {
            Some("UserMessage") => say(&mut out, true, &text(&item["content"])),
            Some("AgentMessage") => say(&mut out, false, &text(&item["content"])),
            Some("CommandExecution") => {
                // As Codex read the command, without the shell it ran it in.
                let command = item["parsed_cmd"][0]["cmd"]
                    .as_str()
                    .or_else(|| item["command"].as_array()?.last()?.as_str())
                    .unwrap_or_default();
                if !command.trim().is_empty() {
                    out.action(&format!("→ {}", first_line(command, ACTION_LENGTH)));
                }
            }
            Some("FileChange") => {
                let files: Vec<&str> = item["changes"]
                    .as_object()
                    .into_iter()
                    .flat_map(|changes| changes.keys())
                    .map(|path| path.rsplit(['\\', '/']).next().unwrap_or(path))
                    .collect();
                if !files.is_empty() {
                    let more = files.len().saturating_sub(3);
                    let mut names = files[..files.len() - more].join(", ");
                    if more > 0 {
                        names.push_str(&format!(" and {more} more"));
                    }
                    out.action(&format!("→ Edit {names}"));
                }
            }
            Some("Extension") if item["kind"] == "web.search" => {
                if let Some(query) = item["query"].as_str().filter(|q| !q.trim().is_empty()) {
                    out.action(&format!("→ Search {}", first_line(query, ACTION_LENGTH)));
                }
            }
            _ => {}
        }
    }
    out
}

/// The text of a message's content: its text parts, one after another.
fn text(content: &Value) -> String {
    let parts = content.as_array().into_iter().flatten();
    parts
        .filter_map(|part| part["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(payload: &str) -> String {
        format!(r#"{{"timestamp":"2026-11-10T20:40:46Z","type":"event_msg","payload":{payload}}}"#)
    }
    fn item(item: &str) -> String {
        event(&format!(
            r#"{{"type":"item_completed","turn_id":"t","item":{item}}}"#
        ))
    }

    #[test]
    fn prompts_replies_and_what_codex_did_are_read_from_completed_items() {
        let rollout = [
            // What a rollout is mostly made of, none of it conversation.
            r#"{"type":"session_meta","payload":{"cwd":"C:\\src","base_instructions":{"text":"You are Codex"}}}"#.to_owned(),
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<environment_context>item_completed</environment_context>"}]}}"#.to_owned(),
            item(r#"{"type":"UserMessage","content":[{"type":"text","text":"fix the bar\nplease"}]}"#),
            item(r#"{"type":"Reasoning","summary_text":[],"raw_content":[]}"#),
            item(r#"{"type":"AgentMessage","content":[{"type":"Text","text":"Looking at **it**."}],"phase":"commentary"}"#),
            item(r#"{"type":"CommandExecution","command":["pwsh.exe","-Command","rg bar\nrg baz"],"parsed_cmd":[{"type":"unknown","cmd":"rg bar\nrg baz"}]}"#),
            item(r#"{"type":"CommandExecution","command":["pwsh.exe","-Command","cargo test"]}"#),
            item(r#"{"type":"FileChange","changes":{"C:\\src\\swm\\src\\bar.rs":{"type":"update"}}}"#),
            item(r#"{"type":"Extension","kind":"web.search","query":"egui painter"}"#),
            item(r#"{"type":"ImageView","path":"file:///C:/x.png"}"#),
            event(r#"{"type":"token_count","info":{}}"#),
            "not json item_completed".to_owned(),
            item(r#"{"type":"AgentMessage","content":[{"type":"Text","text":"Done."}],"phase":"final_answer"}"#),
            item(r#"{"type":"UserMessage","content":[{"type":"text","text":"again"}]}"#),
            event(r#"{"type":"turn_aborted","reason":"interrupted"}"#),
        ]
        .join("\n");
        let conversation = conversation(&rollout, &CodexOptions::default());
        let (texts, items) = conversation.written();
        assert_eq!(
            texts,
            [
                "▍You",
                "fix the bar",
                "please",
                "",
                "▍Codex",
                "Looking at it.",
                "",
                "→ rg bar",
                "→ cargo test",
                "→ Edit bar.rs",
                "→ Search egui painter",
                "Done.",
                "",
                "▍You",
                "again",
                "",
                "▍Codex",
                "■ Interrupted",
            ]
        );
        let labels: Vec<_> = items.iter().map(|item| item.label.as_str()).collect();
        assert_eq!(labels, ["fix the bar", "again"]);
    }

    #[test]
    fn many_changed_files_are_counted_and_a_rollout_of_another_shape_is_empty() {
        let changes = r#"{"type":"FileChange","changes":{"a/1.rs":{},"a/2.rs":{},"a/3.rs":{},"a/4.rs":{},"a/5.rs":{}}}"#;
        let conversation = conversation(&item(changes), &CodexOptions::default());
        let (texts, _) = conversation.written();
        assert_eq!(
            texts.last().map(String::as_str),
            Some("→ Edit 1.rs, 2.rs, 3.rs and 2 more")
        );

        // As Codex wrote messages before it wrote completed items.
        let earlier = [
            event(r#"{"type":"user_message","message":"review the change","images":[]}"#),
            event(r#"{"type":"agent_message","message":"Reading the **diff**.","phase":"commentary"}"#),
            event(r#"{"type":"task_complete","last_agent_message":"Reading the diff."}"#),
        ]
        .join("\n");
        let conversation = super::conversation(&earlier, &CodexOptions::default());
        let (texts, items) = conversation.written();
        assert_eq!(
            texts[..5],
            [
                "▍You",
                "review the change",
                "",
                "▍Codex",
                "Reading the diff."
            ]
        );
        assert_eq!(items.len(), 1);

        let other = r#"{"type":"message","role":"user","content":"hello"}"#;
        let conversation = super::conversation(other, &CodexOptions::default());
        assert_eq!(conversation.written().0, Vec::<String>::new());
    }
}
