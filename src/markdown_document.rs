//! Builds nfm styled documents: lines of styled text, and markdown laid out as
//! them. The preview wraps the lines to its width, and its copy mode and quick
//! select work on them. Nothing here depends on the rest of SWM.
use nfm_egui::egui::Color32;
use nfm_egui::styled_document::{
    ColumnStyleRun, DocumentBuilder, DocumentRange, Position, Style, StyledDocument,
};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::sync::{Arc, OnceLock};
use two_face::re_exports::syntect::easy::HighlightLines;
use two_face::re_exports::syntect::highlighting::FontStyle;
use two_face::re_exports::syntect::parsing::SyntaxSet;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub const BOLD: u8 = 1;
pub const ITALIC: u8 = 2;
pub const STRIKE: u8 = 4;
pub const FAINT: u8 = 32;

/// Colors for markdown; `None` uses the preview's text color.
#[derive(Clone, Debug)]
pub struct Theme {
    pub text: Option<Color32>,
    /// Headings by level, the last repeating for deeper levels.
    pub headings: Vec<Color32>,
    /// Inline code, and fenced code that isn't highlighted.
    pub code: Color32,
    pub code_background: Option<Color32>,
    /// The bat theme fenced code is highlighted with, such as `gruvbox-dark`; `None`,
    /// an unknown theme, or a language bat doesn't know leaves it in `code`.
    pub code_theme: Option<String>,
    pub link: Color32,
    pub quote: Color32,
    pub marker: Color32,
}
impl Default for Theme {
    fn default() -> Self {
        Self {
            text: None,
            headings: vec![
                Color32::from_rgb(0xfa, 0xbd, 0x2f),
                Color32::from_rgb(0x83, 0xa5, 0x98),
                Color32::from_rgb(0x8e, 0xc0, 0x7c),
            ],
            code: Color32::from_rgb(0xfe, 0x80, 0x19),
            code_background: Some(Color32::from_rgb(0x32, 0x30, 0x2f)),
            code_theme: Some("gruvbox-dark".into()),
            link: Color32::from_rgb(0x83, 0xa5, 0x98),
            quote: Color32::from_rgb(0xa8, 0x99, 0x84),
            marker: Color32::from_rgb(0xa8, 0x99, 0x84),
        }
    }
}

/// Writes a document a line at a time.
#[derive(Default)]
pub struct DocumentWriter {
    builder: DocumentBuilder,
    lines: usize,
    current: Vec<(String, Style)>,
    /// The length in bytes of the last line ended.
    last_len: usize,
    /// Whether the last line ended was empty, so blank lines don't repeat.
    blank: bool,
    /// The lines ended, for tests: nfm lays documents out only for previews.
    #[cfg(test)]
    rows: Vec<Vec<(String, Style)>>,
}
impl DocumentWriter {
    /// The index of the line being written.
    pub fn line(&self) -> usize {
        self.lines
    }
    /// The length in bytes of the last line ended, for a range ending there.
    pub fn last_line_len(&self) -> usize {
        self.last_len
    }
    /// Each line's text so far.
    #[cfg(test)]
    pub(crate) fn texts(&self) -> Vec<String> {
        self.rows
            .iter()
            .map(|row| row.iter().map(|(text, _)| text.as_str()).collect())
            .collect()
    }
    /// Appends `text` to the current line; tabs become spaces and line breaks are
    /// dropped.
    pub fn push(&mut self, text: &str, style: Style) {
        let text: String = text
            .chars()
            .filter(|c| *c != '\r' && *c != '\n')
            .map(|c| if c == '\t' { ' ' } else { c })
            .collect();
        if text.is_empty() {
            return;
        }
        match self.current.last_mut() {
            Some((last, last_style)) if *last_style == style => last.push_str(&text),
            _ => self.current.push((text, style)),
        }
    }
    /// Ends the current line, empty or not.
    pub fn end_line(&mut self) {
        let mut text = String::new();
        let mut columns = Vec::new();
        let mut styles = Vec::new();
        let mut x = 0;
        #[cfg(test)]
        self.rows.push(self.current.clone());
        for (segment, style) in std::mem::take(&mut self.current) {
            let start = x;
            for grapheme in segment.graphemes(true) {
                columns.extend(std::iter::repeat_n(x, grapheme.len()));
                x += UnicodeWidthStr::width(grapheme).max(1) as u32;
            }
            text.push_str(&segment);
            styles.push(ColumnStyleRun {
                columns: start..x,
                style,
            });
        }
        self.blank = text.is_empty();
        self.last_len = text.len();
        // Every row here is well formed, so this cannot fail.
        let _ = self.builder.push_row(&text, &columns, false, &styles);
        self.lines += 1;
    }
    /// Ends the current line if it has text.
    pub fn finish_line(&mut self) {
        if !self.current.is_empty() {
            self.end_line();
        }
    }
    /// Ends the current line and leaves one blank line, unless the document is
    /// empty or already ends with one.
    pub fn blank_line(&mut self) {
        self.finish_line();
        if self.lines > 0 && !self.blank {
            self.end_line();
        }
    }
    /// A whole line of `text` in `style`.
    pub fn line_of(&mut self, text: &str, style: Style) {
        self.finish_line();
        self.push(text, style);
        self.end_line();
    }
    /// Lays out `markdown` from the start of a line.
    pub fn markdown(&mut self, markdown: &str, theme: &Theme) {
        self.finish_line();
        Markdown::new(self, theme).render(markdown);
    }
    pub fn finish(mut self) -> Arc<StyledDocument> {
        self.finish_line();
        self.builder.finish()
    }
}

/// The range covering line `line`, `len` bytes long.
pub fn line_range(line: usize, len: usize) -> DocumentRange {
    DocumentRange {
        start: Position { line, byte: 0 },
        end: Position {
            line,
            byte: len.saturating_sub(1),
        },
    }
}

/// bat's syntaxes, loaded the first time code is highlighted.
fn syntaxes() -> &'static SyntaxSet {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAXES.get_or_init(two_face::syntax::extra_newlines)
}

/// A highlighter for code fenced as `language`, such as `rust`, `rs` or
/// `rust,ignore`, in the bat theme `theme`.
fn highlighter(language: &str, theme: Option<&str>) -> Option<HighlightLines<'static>> {
    let theme = theme?;
    static THEMES: OnceLock<two_face::theme::EmbeddedLazyThemeSet> = OnceLock::new();
    let token = language
        .split(|c: char| c == ',' || c.is_whitespace())
        .next()?;
    let syntax = syntaxes().find_syntax_by_token(token)?;
    let name = two_face::theme::EmbeddedLazyThemeSet::theme_names()
        .iter()
        .find(|candidate| candidate.as_name().eq_ignore_ascii_case(theme))?;
    let theme = THEMES.get_or_init(two_face::theme::extra).get(*name);
    Some(HighlightLines::new(syntax, theme))
}

struct List {
    next: Option<u64>,
    /// The marker's width, which continuation lines are indented by.
    width: usize,
}

struct Markdown<'a> {
    out: &'a mut DocumentWriter,
    theme: &'a Theme,
    flags: u8,
    heading: Option<HeadingLevel>,
    links: usize,
    code_block: bool,
    /// The fenced code block's highlighter, if its language is known.
    highlighter: Option<HighlightLines<'static>>,
    quotes: usize,
    lists: Vec<List>,
    /// The marker of a list item whose first line has not started.
    marker: Option<String>,
    table: Option<Table>,
}
#[derive(Default)]
struct Table {
    rows: Vec<Vec<String>>,
    head_rows: usize,
}

impl<'a> Markdown<'a> {
    fn new(out: &'a mut DocumentWriter, theme: &'a Theme) -> Self {
        Self {
            out,
            theme,
            flags: 0,
            heading: None,
            links: 0,
            code_block: false,
            highlighter: None,
            quotes: 0,
            lists: Vec::new(),
            marker: None,
            table: None,
        }
    }

    fn render(mut self, markdown: &str) {
        let options =
            Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
        for event in Parser::new_ext(markdown, options) {
            self.event(event);
        }
        self.out.finish_line();
    }

    fn style(&self) -> Style {
        let mut style = Style {
            foreground: self.theme.text,
            flags: self.flags,
            ..Default::default()
        };
        if let Some(level) = self.heading {
            let index = level as usize - 1;
            style.foreground = self
                .theme
                .headings
                .get(index)
                .or(self.theme.headings.last())
                .copied();
            style.flags |= BOLD;
            style.underline = level == HeadingLevel::H1;
        } else if self.links > 0 {
            style.foreground = Some(self.theme.link);
            style.underline = true;
        } else if self.quotes > 0 {
            style.foreground = Some(self.theme.quote);
        }
        style
    }

    fn marker_style(&self) -> Style {
        Style {
            foreground: Some(self.theme.marker),
            ..Default::default()
        }
    }

    /// Starts a line with the quote bars and list indentation it needs.
    fn begin_line(&mut self) {
        if !self.out.current.is_empty() {
            return;
        }
        let marker = self.marker_style();
        for _ in 0..self.quotes {
            self.out.push("│ ", marker);
        }
        let Some((current, outer)) = self.lists.split_last() else {
            return;
        };
        let indent: usize = outer.iter().map(|list| list.width).sum();
        if indent > 0 {
            self.out.push(&" ".repeat(indent), Style::default());
        }
        match self.marker.take() {
            Some(text) => self.out.push(&text, marker),
            None => self.out.push(&" ".repeat(current.width), Style::default()),
        }
    }

    fn text(&mut self, text: &str) {
        if let Some(table) = &mut self.table {
            if let Some(cell) = table.rows.last_mut().and_then(|row| row.last_mut()) {
                cell.push_str(text);
            }
            return;
        }
        self.begin_line();
        let style = self.style();
        self.out.push(text, style);
    }

    fn end_block(&mut self) {
        self.out.finish_line();
        if self.lists.is_empty() {
            self.out.blank_line();
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) if self.code_block => self.code_lines(&text),
            Event::Text(text) => self.text(&text),
            Event::Code(code) => {
                if self.table.is_some() {
                    return self.text(&code);
                }
                self.begin_line();
                let style = Style {
                    foreground: Some(self.theme.code),
                    background: self.theme.code_background,
                    ..Default::default()
                };
                self.out.push(&code, style);
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                for line in html.lines() {
                    self.begin_line();
                    let style = Style {
                        flags: FAINT,
                        ..self.style()
                    };
                    self.out.push(line, style);
                    if html.ends_with('\n') {
                        self.out.end_line();
                    }
                }
            }
            Event::SoftBreak => self.text(" "),
            Event::HardBreak => self.out.end_line(),
            Event::Rule => {
                self.out.finish_line();
                let style = self.marker_style();
                self.out.line_of(&"─".repeat(40), style);
                self.out.blank_line();
            }
            Event::TaskListMarker(done) => {
                self.begin_line();
                let style = self.marker_style();
                self.out.push(if done { "[x] " } else { "[ ] " }, style);
            }
            Event::FootnoteReference(name) => self.text(&format!("[{name}]")),
            Event::InlineMath(math) | Event::DisplayMath(math) => self.text(&math),
        }
    }

    fn code_lines(&mut self, text: &str) {
        let style = Style {
            foreground: Some(self.theme.code),
            background: self.theme.code_background,
            ..Default::default()
        };
        for line in text.split_inclusive('\n') {
            self.begin_line();
            self.out.push("  ", style);
            let pieces = self
                .highlighter
                .as_mut()
                .and_then(|highlighter| highlighter.highlight_line(line, syntaxes()).ok());
            match pieces {
                Some(pieces) => {
                    for (highlight, piece) in pieces {
                        let font = highlight.font_style;
                        let color = highlight.foreground;
                        let style = Style {
                            foreground: Some(Color32::from_rgb(color.r, color.g, color.b)),
                            flags: if font.contains(FontStyle::BOLD) {
                                BOLD
                            } else {
                                0
                            } | if font.contains(FontStyle::ITALIC) {
                                ITALIC
                            } else {
                                0
                            },
                            underline: font.contains(FontStyle::UNDERLINE),
                            ..style
                        };
                        self.out.push(piece, style);
                    }
                }
                None => self.out.push(line, style),
            }
            if line.ends_with('\n') {
                self.out.end_line();
            }
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {}
            Tag::Heading { level, .. } => {
                self.out.finish_line();
                self.heading = Some(level);
            }
            Tag::BlockQuote(_) => {
                self.out.finish_line();
                self.quotes += 1;
            }
            Tag::CodeBlock(kind) => {
                self.out.finish_line();
                if let CodeBlockKind::Fenced(language) = kind
                    && !language.is_empty()
                {
                    self.begin_line();
                    let style = Style {
                        flags: FAINT,
                        ..self.marker_style()
                    };
                    self.out.push(&language, style);
                    self.out.end_line();
                    self.highlighter = highlighter(&language, self.theme.code_theme.as_deref());
                }
                self.code_block = true;
            }
            Tag::List(start) => {
                self.out.finish_line();
                self.lists.push(List {
                    next: start,
                    width: 2,
                });
            }
            Tag::Item => {
                self.out.finish_line();
                if let Some(list) = self.lists.last_mut() {
                    let marker = match &mut list.next {
                        Some(number) => {
                            *number += 1;
                            format!("{}. ", *number - 1)
                        }
                        None => "• ".into(),
                    };
                    list.width = UnicodeWidthStr::width(marker.as_str());
                    self.marker = Some(marker);
                }
            }
            Tag::Emphasis => self.flags |= ITALIC,
            Tag::Strong => self.flags |= BOLD,
            Tag::Strikethrough => self.flags |= STRIKE,
            Tag::Link { .. } | Tag::Image { .. } => self.links += 1,
            Tag::Table(_) => {
                self.out.finish_line();
                self.table = Some(Table::default());
            }
            Tag::TableHead => {
                if let Some(table) = &mut self.table {
                    table.rows.push(Vec::new());
                }
            }
            Tag::TableRow => {
                if let Some(table) = &mut self.table {
                    table.rows.push(Vec::new());
                }
            }
            Tag::TableCell => {
                if let Some(row) = self.table.as_mut().and_then(|t| t.rows.last_mut()) {
                    row.push(String::new());
                }
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.end_block(),
            TagEnd::Heading(_) => {
                self.heading = None;
                self.out.finish_line();
                self.out.blank_line();
            }
            TagEnd::BlockQuote(_) => {
                self.out.finish_line();
                self.quotes -= 1;
                if self.quotes == 0 && self.lists.is_empty() {
                    self.out.blank_line();
                }
            }
            TagEnd::CodeBlock => {
                self.code_block = false;
                self.highlighter = None;
                self.end_block();
            }
            TagEnd::List(_) => {
                self.out.finish_line();
                self.lists.pop();
                if self.lists.is_empty() {
                    self.out.blank_line();
                }
            }
            TagEnd::Item => {
                // An item with no text still shows its marker.
                if self.marker.is_some() {
                    self.begin_line();
                }
                self.out.finish_line();
            }
            TagEnd::Emphasis => self.flags &= !ITALIC,
            TagEnd::Strong => self.flags &= !BOLD,
            TagEnd::Strikethrough => self.flags &= !STRIKE,
            TagEnd::Link | TagEnd::Image => self.links -= 1,
            TagEnd::TableHead => {
                if let Some(table) = &mut self.table {
                    table.head_rows = table.rows.len();
                }
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    self.table_lines(table);
                }
                self.end_block();
            }
            _ => {}
        }
    }

    /// Cells padded to their column's widest, between bars.
    fn table_lines(&mut self, table: Table) {
        let columns = table.rows.iter().map(Vec::len).max().unwrap_or(0);
        let width = |cell: &str| UnicodeWidthStr::width(cell);
        let widths: Vec<usize> = (0..columns)
            .map(|column| {
                table
                    .rows
                    .iter()
                    .filter_map(|row| row.get(column))
                    .map(|cell| width(cell))
                    .max()
                    .unwrap_or(0)
            })
            .collect();
        let bar = self.marker_style();
        for (index, row) in table.rows.iter().enumerate() {
            self.begin_line();
            let style = Style {
                flags: if index < table.head_rows { BOLD } else { 0 },
                ..self.style()
            };
            for (column, cell_width) in widths.iter().enumerate() {
                if column > 0 {
                    self.out.push(" │ ", bar);
                }
                let cell = row.get(column).map_or("", String::as_str);
                self.out.push(cell, style);
                let pad = cell_width - width(cell);
                if pad > 0 && column + 1 < widths.len() {
                    self.out.push(&" ".repeat(pad), style);
                }
            }
            self.out.end_line();
            if index + 1 == table.head_rows {
                self.begin_line();
                let rule: Vec<String> = widths.iter().map(|w| "─".repeat(*w)).collect();
                self.out.push(&rule.join("─┼─"), bar);
                self.out.end_line();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(markdown: &str) -> DocumentWriter {
        let mut writer = DocumentWriter::default();
        writer.markdown(markdown, &Theme::default());
        writer.finish_line();
        writer
    }
    /// Each line's text.
    fn lines(markdown: &str) -> Vec<String> {
        written(markdown).texts()
    }

    #[test]
    fn blocks_are_separated_and_inline_text_joins() {
        assert_eq!(
            lines("# Title\n\nSome *text*\nwrapped `code`.\n\nNext."),
            ["Title", "", "Some text wrapped code.", "", "Next.", ""]
        );
    }

    #[test]
    fn lists_nest_with_markers_and_continuations() {
        assert_eq!(
            lines("- one\n- two\n  - inner\n3. three\n4. four"),
            ["• one", "• two", "  • inner", "", "3. three", "4. four", ""]
        );
    }

    #[test]
    fn code_blocks_keep_their_lines_and_language() {
        assert_eq!(
            lines("```rust\nfn main() {\n    run();\n}\n```\nAfter"),
            [
                "rust",
                "  fn main() {",
                "      run();",
                "  }",
                "",
                "After",
                ""
            ]
        );
    }

    #[test]
    fn quotes_and_tables_are_drawn() {
        assert_eq!(lines("> quoted\n> more"), ["│ quoted more", ""]);
        assert_eq!(
            lines("| a | bee |\n|---|---|\n| cc | d |"),
            ["a  │ bee", "───┼────", "cc │ d", ""]
        );
    }

    #[test]
    fn styles_follow_emphasis_headings_and_code() {
        let theme = Theme::default();
        let rows = written("## Head\n\n**bold** `x` [link](u)").rows;
        let (heading, style) = &rows[0][0];
        assert_eq!(heading, "Head");
        assert_eq!(style.foreground, Some(theme.headings[1]));
        assert_eq!(style.flags, BOLD);
        let body: Vec<_> = rows[2]
            .iter()
            .map(|(text, style)| (text.as_str(), style.flags, style.foreground))
            .collect();
        assert_eq!(
            body,
            [
                ("bold", BOLD, None),
                (" ", 0, None),
                ("x", 0, Some(theme.code)),
                (" ", 0, None),
                ("link", 0, Some(theme.link)),
            ]
        );
    }

    /// The distinct colors on the code line `fn main() {}` fenced as `fence`.
    fn code_colors(fence: &str, theme: Theme) -> Vec<Option<Color32>> {
        let mut writer = DocumentWriter::default();
        writer.markdown(&format!("```{fence}\nfn main() {{}}\n```"), &theme);
        // A fence without a language has no label line above the code.
        let code = writer
            .rows
            .iter()
            .find(|row| row.iter().any(|(text, _)| text.contains("main")));
        let mut colors: Vec<_> = code.unwrap().iter().map(|(_, s)| s.foreground).collect();
        colors.dedup();
        colors
    }

    #[test]
    fn known_languages_are_highlighted_in_the_theme() {
        let theme = Theme::default();
        let plain = Some(theme.code);
        assert!(code_colors("rust", theme.clone()).len() > 2);
        assert!(code_colors("rs,ignore", theme.clone()).len() > 2);
        assert_eq!(code_colors("not-a-language", theme.clone()), [plain]);
        assert_eq!(code_colors("", theme.clone()), [plain]);
        let without = Theme {
            code_theme: None,
            ..theme.clone()
        };
        assert_eq!(code_colors("rust", without), [plain]);
        let unknown = Theme {
            code_theme: Some("no-such-theme".into()),
            ..theme
        };
        assert_eq!(code_colors("rust", unknown), [plain]);
    }

    #[test]
    fn highlighted_code_keeps_the_block_background_and_text() {
        let theme = Theme::default();
        let mut writer = DocumentWriter::default();
        writer.markdown("```rust\nlet x = 1;\n```", &theme);
        let line = &writer.rows[1];
        let text: String = line.iter().map(|(text, _)| text.as_str()).collect();
        assert_eq!(text, "  let x = 1;");
        assert!(
            line.iter()
                .all(|(_, s)| s.background == theme.code_background)
        );
    }

    #[test]
    fn line_ranges_cover_their_line() {
        let range = line_range(3, 10);
        assert_eq!((range.start.line, range.start.byte), (3, 0));
        assert_eq!((range.end.line, range.end.byte), (3, 9));
    }
}
