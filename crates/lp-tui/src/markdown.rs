//! Markdown to styled, word-wrapped ratatui lines.
//!
//! Palette follows the Claude Code `DESIGN.md` from awesome-tui-design:
//! terracotta accent for headings, pink for inline code, dim gutters.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const ACCENT: Color = Color::Rgb(0xD7, 0x77, 0x57);
const CODE: Color = Color::Rgb(0xFD, 0x5D, 0xB1);
const LINK: Color = Color::Cyan;

const MATH: [(&str, &str); 9] = [
    ("\\Rightarrow", "⟹"),
    ("\\land", "∧"),
    ("\\to", "→"),
    ("\\leq", "≤"),
    ("\\geq", "≥"),
    ("\\neq", "≠"),
    ("\\alpha", "α"),
    ("\\lambda", "λ"),
    ("\\sum", "∑"),
];

fn math(s: &str) -> String {
    MATH.iter()
        .fold(s.replace('$', ""), |acc, (tex, uni)| acc.replace(tex, uni))
}

fn dim() -> Style {
    Style::new().fg(Color::DarkGray)
}

/// Render `md` into lines no wider than `width` columns.
pub fn render(md: &str, width: usize) -> Vec<Line<'static>> {
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut w = Writer::new(width.max(8));
    for ev in Parser::new_ext(md, opts) {
        w.event(ev);
    }
    w.flush();
    w.out
}

struct Level {
    ordered: Option<u64>,
    indent: usize,
}

#[derive(Default)]
struct Table {
    rows: Vec<Vec<String>>,
    row: Vec<String>,
    cell: String,
}

struct Writer {
    width: usize,
    out: Vec<Line<'static>>,
    cur: Vec<Span<'static>>,
    cur_w: usize,
    has_content: bool,
    styles: Vec<Style>,
    quote: usize,
    lists: Vec<Level>,
    marker: Option<String>,
    item_fresh: bool,
    code: bool,
    link: Option<String>,
    table: Option<Table>,
}

impl Writer {
    fn new(width: usize) -> Self {
        Self {
            width,
            out: Vec::new(),
            cur: Vec::new(),
            cur_w: 0,
            has_content: false,
            styles: vec![Style::new()],
            quote: 0,
            lists: Vec::new(),
            marker: None,
            item_fresh: false,
            code: false,
            link: None,
            table: None,
        }
    }

    fn style(&self) -> Style {
        self.styles.last().copied().unwrap_or_default()
    }

    fn push_style(&mut self, f: impl FnOnce(Style) -> Style) {
        let s = f(self.style());
        self.styles.push(s);
    }

    /// Spans for the left margin: quote bars, list indentation and, on the
    /// first line of an item, the bullet.
    fn prefix(&mut self) -> Vec<Span<'static>> {
        let mut p: Vec<Span<'static>> =
            (0..self.quote).map(|_| Span::styled("▎ ", dim())).collect();
        let n = self.lists.len();
        let outer: usize = self
            .lists
            .iter()
            .take(n.saturating_sub(1))
            .map(|l| l.indent)
            .sum();
        if outer > 0 {
            p.push(Span::raw(" ".repeat(outer)));
        }
        if let (Some(last), Some(m)) = (self.lists.last(), self.marker.take()) {
            p.push(Span::styled(m, Style::new().fg(ACCENT)));
            let _ = last;
        } else if let Some(last) = self.lists.last() {
            p.push(Span::raw(" ".repeat(last.indent)));
        }
        p
    }

    fn ensure_line(&mut self) {
        if self.cur.is_empty() {
            let p = self.prefix();
            self.cur_w = p.iter().map(|s| s.content.width()).sum();
            self.cur = p;
        }
    }

    fn flush(&mut self) {
        if self.has_content {
            self.out.push(Line::from(std::mem::take(&mut self.cur)));
        } else {
            self.cur.clear();
        }
        self.cur_w = 0;
        self.has_content = false;
    }

    /// Separate a new block from the previous one with a blank line.
    fn block_start(&mut self) {
        self.flush();
        let after_blank = self.out.last().is_none_or(|l| l.spans.is_empty());
        if !after_blank && !self.item_fresh {
            self.out.push(Line::raw(""));
        }
        self.item_fresh = false;
    }

    fn push_span(&mut self, s: &str, style: Style) {
        self.cur.push(Span::styled(s.to_owned(), style));
        self.cur_w += s.width();
        self.has_content = true;
    }

    fn text(&mut self, s: &str, style: Style) {
        if let Some(t) = &mut self.table {
            t.cell.push_str(s);
            return;
        }
        self.item_fresh = false;
        for tok in s.split_inclusive(char::is_whitespace) {
            let word = tok.trim_end();
            let w = word.width();
            if !self.has_content && word.is_empty() {
                continue;
            }
            self.ensure_line();
            if self.has_content && self.cur_w + w > self.width {
                self.flush();
                if word.is_empty() {
                    continue;
                }
                self.ensure_line();
            }
            if self.cur_w + w > self.width {
                // longer than a whole line: split by character
                let mut buf = String::new();
                for c in word.chars() {
                    let cw = c.width().unwrap_or(0);
                    if self.cur_w + buf.width() + cw > self.width {
                        self.push_span(&buf, style);
                        buf.clear();
                        self.flush();
                        self.ensure_line();
                    }
                    buf.push(c);
                }
                self.push_span(&buf, style);
            } else if !word.is_empty() {
                self.push_span(word, style);
            }
            if tok.len() > word.len() && self.cur_w < self.width {
                self.push_span(" ", style);
            }
        }
    }

    fn code_text(&mut self, s: &str) {
        let body = Style::new().fg(Color::Rgb(0xE6, 0xE6, 0xE6));
        for raw in s.lines() {
            self.flush();
            let mut chunks = vec![String::new()];
            let mut cw = 0;
            let room = self.width.saturating_sub(self.quote * 2 + 2).max(1);
            for c in raw.replace('\t', "    ").chars() {
                let w = c.width().unwrap_or(0);
                if cw + w > room {
                    chunks.push(String::new());
                    cw = 0;
                }
                if let Some(last) = chunks.last_mut() {
                    last.push(c);
                }
                cw += w;
            }
            for chunk in chunks {
                self.ensure_line();
                self.push_span("▌ ", dim());
                self.push_span(&chunk, body);
                self.flush();
            }
        }
    }

    fn event(&mut self, ev: Event<'_>) {
        match ev {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(t) if self.code => self.code_text(&t),
            Event::Text(t) => {
                let style = self.style();
                self.text(&math(&t), style);
            }
            Event::Code(c) => {
                let style = self.style().fg(CODE);
                self.text(&format!("`{c}`"), style);
            }
            Event::SoftBreak => {
                let style = self.style();
                self.text(" ", style);
            }
            Event::HardBreak => self.flush(),
            Event::Rule => {
                self.block_start();
                self.out
                    .push(Line::styled("─".repeat(self.width.min(40)), dim()));
            }
            Event::TaskListMarker(done) => {
                self.marker = Some(if done { "☑ " } else { "☐ " }.to_owned());
            }
            Event::Html(h) | Event::InlineHtml(h) => {
                let style = dim();
                self.text(&h, style);
            }
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.block_start(),
            Tag::Heading { level, .. } => {
                self.block_start();
                let base = Style::new().fg(ACCENT).add_modifier(Modifier::BOLD);
                self.styles.push(match level {
                    HeadingLevel::H1 => base.add_modifier(Modifier::UNDERLINED),
                    HeadingLevel::H2 => base,
                    _ => Style::new().add_modifier(Modifier::BOLD),
                });
                let hashes = match level {
                    HeadingLevel::H1 | HeadingLevel::H2 => "",
                    HeadingLevel::H3 => "▸ ",
                    _ => "· ",
                };
                if !hashes.is_empty() {
                    let style = self.style();
                    self.text(hashes, style);
                }
            }
            Tag::BlockQuote(_) => {
                self.block_start();
                self.quote += 1;
                self.push_style(|s| s.add_modifier(Modifier::ITALIC).fg(Color::Gray));
            }
            Tag::CodeBlock(kind) => {
                self.block_start();
                self.code = true;
                if let CodeBlockKind::Fenced(lang) = kind {
                    if !lang.is_empty() {
                        self.ensure_line();
                        self.push_span(&format!("▌ {lang}"), dim().add_modifier(Modifier::ITALIC));
                        self.flush();
                    }
                }
            }
            Tag::List(first) => {
                if self.lists.is_empty() {
                    self.block_start();
                } else {
                    self.flush();
                }
                self.lists.push(Level {
                    ordered: first,
                    indent: 2,
                });
            }
            Tag::Item => {
                self.flush();
                let Some(level) = self.lists.last_mut() else {
                    return;
                };
                let m = match &mut level.ordered {
                    Some(n) => {
                        let m = format!("{n}. ");
                        *n += 1;
                        m
                    }
                    None => "• ".to_owned(),
                };
                level.indent = m.width();
                self.marker = Some(m);
                self.item_fresh = true;
            }
            Tag::Emphasis => self.push_style(|s| s.add_modifier(Modifier::ITALIC)),
            Tag::Strong => self.push_style(|s| s.add_modifier(Modifier::BOLD)),
            Tag::Strikethrough => self.push_style(|s| s.add_modifier(Modifier::CROSSED_OUT)),
            Tag::Link { dest_url, .. } => {
                self.link = Some(dest_url.into_string());
                self.push_style(|s| s.fg(LINK).add_modifier(Modifier::UNDERLINED));
            }
            Tag::Table(_) => {
                self.block_start();
                self.table = Some(Table::default());
            }
            Tag::TableRow | Tag::TableHead => {}
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush(),
            TagEnd::Heading(_) => {
                self.styles.pop();
                self.flush();
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.styles.pop();
                self.quote = self.quote.saturating_sub(1);
            }
            TagEnd::CodeBlock => {
                self.code = false;
            }
            TagEnd::List(_) => {
                self.flush();
                self.lists.pop();
            }
            TagEnd::Item => self.flush(),
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.styles.pop();
            }
            TagEnd::Link => {
                self.styles.pop();
                if let Some(url) = self.link.take() {
                    self.text(&format!(" ({url})"), dim());
                }
            }
            TagEnd::TableCell => {
                if let Some(t) = &mut self.table {
                    let cell = std::mem::take(&mut t.cell);
                    t.row.push(cell);
                }
            }
            TagEnd::TableRow | TagEnd::TableHead => {
                if let Some(t) = &mut self.table {
                    let row = std::mem::take(&mut t.row);
                    t.rows.push(row);
                }
            }
            TagEnd::Table => {
                if let Some(t) = self.table.take() {
                    self.render_table(&t.rows);
                }
            }
            _ => {}
        }
    }

    fn render_table(&mut self, rows: &[Vec<String>]) {
        let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
        if cols == 0 {
            return;
        }
        let mut widths: Vec<usize> = (0..cols)
            .map(|c| {
                rows.iter()
                    .filter_map(|r| r.get(c))
                    .map(|s| s.width())
                    .max()
                    .unwrap_or(1)
                    .max(1)
            })
            .collect();
        let chrome = 3 * cols - 1;
        while widths.iter().sum::<usize>() + chrome > self.width {
            let Some(max) = widths.iter_mut().max() else {
                break;
            };
            if *max <= 3 {
                break;
            }
            *max -= 1;
        }
        let fit = |s: &str, w: usize| -> String {
            let mut out = String::new();
            let mut cw = 0;
            let clipped = s.width() > w;
            let limit = if clipped { w.saturating_sub(1) } else { w };
            for c in s.chars() {
                let k = c.width().unwrap_or(0);
                if cw + k > limit {
                    break;
                }
                out.push(c);
                cw += k;
            }
            if clipped {
                out.push('…');
                cw += 1;
            }
            out.push_str(&" ".repeat(w.saturating_sub(cw)));
            out
        };
        for (i, row) in rows.iter().enumerate() {
            let mut spans = Vec::new();
            for (c, w) in widths.iter().enumerate() {
                if c > 0 {
                    spans.push(Span::styled(" │ ", dim()));
                }
                let style = if i == 0 {
                    Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)
                } else {
                    Style::new()
                };
                spans.push(Span::styled(
                    fit(row.get(c).map_or("", String::as_str), *w),
                    style,
                ));
            }
            self.out.push(Line::from(spans));
            if i == 0 {
                let rule = widths
                    .iter()
                    .map(|w| "─".repeat(*w))
                    .collect::<Vec<_>>()
                    .join("─┼─");
                self.out.push(Line::styled(rule, dim()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(md: &str, w: usize) -> Vec<String> {
        render(md, w)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn headings_paragraphs_and_wrap() {
        let out = plain("# Title\n\nhello brave new world of rust", 12);
        assert_eq!(out[0], "Title");
        assert_eq!(out[1], "");
        assert!(out[2..].iter().all(|l| l.width() <= 12), "{out:?}");
        assert_eq!(out[2..].join(" ").split_whitespace().count(), 6);
    }

    #[test]
    fn lists_nested_and_ordered() {
        let out = plain("- a\n- b\n  - c\n\n1. x\n2. y", 40);
        assert_eq!(out, ["• a", "• b", "  • c", "", "1. x", "2. y"]);
    }

    #[test]
    fn code_block_keeps_lines_and_gutter() {
        let out = plain("```rust\nlet x = 1;\n  y\n```", 40);
        assert_eq!(out, ["▌ rust", "▌ let x = 1;", "▌   y"]);
    }

    #[test]
    fn inline_styles_and_links() {
        let lines = render("**bold** `c` [t](http://x)", 40);
        let spans = &lines[0].spans;
        assert!(spans[0].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(spans[2].style.fg, Some(CODE));
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text.trim_end(), "bold `c` t (http://x)");
    }

    #[test]
    fn table_aligns_and_fits() {
        let out = plain("| a | bb |\n|---|---|\n| ccc | d |", 40);
        assert_eq!(out, ["a   │ bb", "────┼───", "ccc │ d "]);
        let narrow = plain("| aaaaaaaaaa | bbbbbbbbbb |\n|--|--|\n| 1 | 2 |", 12);
        assert!(narrow.iter().all(|l| l.width() <= 12), "{narrow:?}");
    }

    #[test]
    fn quote_and_math() {
        let out = plain("> a $\\to$ b", 40);
        assert_eq!(out, ["▎ a → b"]);
    }

    #[test]
    fn long_word_is_split() {
        let out = plain("abcdefghijklmnopqrstuvwxyz", 10);
        assert!(
            out.iter().all(|l| l.width() <= 10) && out.len() == 3,
            "{out:?}"
        );
    }
}
