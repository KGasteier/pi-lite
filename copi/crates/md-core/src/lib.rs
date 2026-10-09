//! md-core – GUI-freier Markdown-Kern.
//!
//! Wandelt Markdown (CommonMark + GFM-Tabellen, Durchstreichen, Aufgabenlisten)
//! in eine kleine, stabile Zwischendarstellung (IR) um. Renderer (z. B. `md-egui`)
//! arbeiten nur gegen diese IR und müssen pulldown-cmark nicht kennen.
//! Dadurch kann dieselbe IR später auch im Outliner-Projekt genutzt werden.
//!
//! Bewusste Vereinfachungen:
//! * Rohes HTML wird als Text übernommen (nie interpretiert).
//! * Bilder werden zu Links mit ihrem Alt-Text.
//! * Weiche Zeilenumbrüche werden zu Leerzeichen, harte zu `\n`.

pub mod highlight;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Inline-Formatierung eines Textstücks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    /// Linkziel, falls das Stück Teil eines Links ist.
    pub link: Option<String>,
}

/// Ein zusammenhängendes Textstück mit einheitlichem Stil.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

pub type Inlines = Vec<Span>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItem {
    /// `Some(true/false)` bei Aufgabenlisten (`- [x]`), sonst `None`.
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Paragraph(Inlines),
    Heading { level: u8, text: Inlines },
    Code { lang: String, text: String },
    List { start: Option<u64>, items: Vec<ListItem> },
    Quote(Vec<Block>),
    Table { align: Vec<Align>, head: Vec<Inlines>, rows: Vec<Vec<Inlines>> },
    Rule,
}

/// Ein geparstes Dokument.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Doc {
    pub blocks: Vec<Block>,
}

impl Doc {
    /// Reiner Text des Dokuments (z. B. für „Kopieren“ oder Suche).
    pub fn plain_text(&self) -> String {
        let mut out = String::new();
        for b in &self.blocks {
            block_text(b, &mut out);
        }
        out.trim_end().to_string()
    }
}

/// Text einer Inline-Folge ohne Formatierung.
pub fn inline_text(spans: &[Span]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

fn block_text(b: &Block, out: &mut String) {
    match b {
        Block::Paragraph(t) | Block::Heading { text: t, .. } => {
            out.push_str(&inline_text(t));
            out.push_str("\n\n");
        }
        Block::Code { text, .. } => {
            out.push_str(text);
            out.push('\n');
        }
        Block::List { items, .. } => {
            for it in items {
                for b in &it.blocks {
                    block_text(b, out);
                }
            }
        }
        Block::Quote(bs) => bs.iter().for_each(|b| block_text(b, out)),
        Block::Table { head, rows, .. } => {
            let row = |r: &Vec<Inlines>| r.iter().map(|c| inline_text(c)).collect::<Vec<_>>().join("\t");
            out.push_str(&row(head));
            out.push('\n');
            for r in rows {
                out.push_str(&row(r));
                out.push('\n');
            }
            out.push('\n');
        }
        Block::Rule => out.push_str("---\n\n"),
    }
}

enum Container {
    Blocks(Vec<Block>),
    Quote(Vec<Block>),
    List { start: Option<u64>, items: Vec<ListItem> },
    Item { task: Option<bool>, blocks: Vec<Block> },
    Table { align: Vec<Align>, head: Vec<Inlines>, rows: Vec<Vec<Inlines>>, row: Vec<Inlines> },
}

struct Builder {
    stack: Vec<Container>,
    inl: Inlines,
    style: Style,
    /// Zähler für verschachtelte Hervorhebungen (für korrektes Ende).
    bold: u32,
    italic: u32,
    strike: u32,
    links: Vec<Option<String>>,
    code: Option<(String, String)>,
    heading: Option<u8>,
}

impl Builder {
    fn push_text(&mut self, text: &str, code: bool) {
        if let Some((_, buf)) = self.code.as_mut() {
            buf.push_str(text);
            return;
        }
        if text.is_empty() {
            return;
        }
        let mut style = self.style.clone();
        style.code = code;
        if let Some(last) = self.inl.last_mut() {
            if last.style == style {
                last.text.push_str(text);
                return;
            }
        }
        self.inl.push(Span { text: text.to_string(), style });
    }

    fn sync_style(&mut self) {
        self.style.bold = self.bold > 0;
        self.style.italic = self.italic > 0;
        self.style.strike = self.strike > 0;
        self.style.link = self.links.iter().rev().flatten().next().cloned();
    }

    fn push_block(&mut self, b: Block) {
        match self.stack.last_mut() {
            Some(Container::Blocks(v)) | Some(Container::Quote(v)) | Some(Container::Item { blocks: v, .. }) => v.push(b),
            // In Listen ohne offenes Item oder in Tabellen dürfen keine Blöcke landen –
            // dann hängen wir es defensiv an das äußerste Block-Ziel.
            _ => {
                for c in self.stack.iter_mut().rev() {
                    if let Container::Blocks(v) | Container::Quote(v) | Container::Item { blocks: v, .. } = c {
                        v.push(b);
                        return;
                    }
                }
            }
        }
    }

    /// Lose Inline-Reste (z. B. in engen Listen) als Absatz abschließen.
    fn flush_inline(&mut self) {
        if self.inl.is_empty() {
            return;
        }
        let mut inl = std::mem::take(&mut self.inl);
        trim_inlines(&mut inl);
        if !inl.is_empty() {
            self.push_block(Block::Paragraph(inl));
        }
    }

    fn take_inline(&mut self) -> Inlines {
        let mut inl = std::mem::take(&mut self.inl);
        trim_inlines(&mut inl);
        inl
    }
}

fn trim_inlines(inl: &mut Inlines) {
    if let Some(f) = inl.first_mut() {
        if !f.style.code {
            let t = f.text.trim_start().to_string();
            f.text = t;
        }
    }
    if let Some(l) = inl.last_mut() {
        if !l.style.code {
            let t = l.text.trim_end().to_string();
            l.text = t;
        }
    }
    inl.retain(|s| !s.text.is_empty());
}

fn align(a: &Alignment) -> Align {
    match a {
        Alignment::None => Align::None,
        Alignment::Left => Align::Left,
        Alignment::Center => Align::Center,
        Alignment::Right => Align::Right,
    }
}

fn level(l: HeadingLevel) -> u8 {
    match l {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Markdown parsen.
pub fn parse(src: &str) -> Doc {
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut b = Builder {
        stack: vec![Container::Blocks(Vec::new())],
        inl: Vec::new(),
        style: Style::default(),
        bold: 0,
        italic: 0,
        strike: 0,
        links: Vec::new(),
        code: None,
        heading: None,
    };
    for ev in Parser::new_ext(src, opts) {
        match ev {
            Event::Start(tag) => match tag {
                Tag::Paragraph => b.flush_inline(),
                Tag::Heading { level: l, .. } => {
                    b.flush_inline();
                    b.heading = Some(level(l));
                }
                Tag::BlockQuote(_) => {
                    b.flush_inline();
                    b.stack.push(Container::Quote(Vec::new()));
                }
                Tag::CodeBlock(kind) => {
                    b.flush_inline();
                    let lang = match kind {
                        CodeBlockKind::Fenced(info) => info.split_whitespace().next().unwrap_or("").to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                    b.code = Some((lang, String::new()));
                }
                Tag::HtmlBlock => b.flush_inline(),
                Tag::List(start) => {
                    b.flush_inline();
                    b.stack.push(Container::List { start, items: Vec::new() });
                }
                Tag::Item => b.stack.push(Container::Item { task: None, blocks: Vec::new() }),
                Tag::Table(al) => {
                    b.flush_inline();
                    b.stack.push(Container::Table { align: al.iter().map(align).collect(), head: Vec::new(), rows: Vec::new(), row: Vec::new() });
                }
                Tag::TableHead | Tag::TableRow => {
                    if let Some(Container::Table { row, .. }) = b.stack.last_mut() {
                        row.clear();
                    }
                }
                Tag::TableCell => b.inl.clear(),
                Tag::Emphasis => {
                    b.italic += 1;
                    b.sync_style();
                }
                Tag::Strong => {
                    b.bold += 1;
                    b.sync_style();
                }
                Tag::Strikethrough => {
                    b.strike += 1;
                    b.sync_style();
                }
                Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                    b.links.push(Some(dest_url.to_string()));
                    b.sync_style();
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => {
                    let inl = b.take_inline();
                    if !inl.is_empty() {
                        b.push_block(Block::Paragraph(inl));
                    }
                }
                TagEnd::Heading(_) => {
                    let text = b.take_inline();
                    let level = b.heading.take().unwrap_or(1);
                    b.push_block(Block::Heading { level, text });
                }
                TagEnd::BlockQuote(_) => {
                    b.flush_inline();
                    if let Some(Container::Quote(v)) = b.stack.pop() {
                        b.push_block(Block::Quote(v));
                    }
                }
                TagEnd::CodeBlock => {
                    if let Some((lang, mut text)) = b.code.take() {
                        if text.ends_with('\n') {
                            text.pop();
                        }
                        b.push_block(Block::Code { lang, text });
                    }
                }
                TagEnd::HtmlBlock => b.flush_inline(),
                TagEnd::List(_) => {
                    b.flush_inline();
                    if let Some(Container::List { start, items }) = b.stack.pop() {
                        b.push_block(Block::List { start, items });
                    }
                }
                TagEnd::Item => {
                    b.flush_inline();
                    if let Some(Container::Item { task, blocks }) = b.stack.pop() {
                        if let Some(Container::List { items, .. }) = b.stack.last_mut() {
                            items.push(ListItem { task, blocks });
                        }
                    }
                }
                TagEnd::TableCell => {
                    let cell = b.take_inline();
                    if let Some(Container::Table { row, .. }) = b.stack.last_mut() {
                        row.push(cell);
                    }
                }
                TagEnd::TableHead => {
                    if let Some(Container::Table { head, row, .. }) = b.stack.last_mut() {
                        *head = std::mem::take(row);
                    }
                }
                TagEnd::TableRow => {
                    if let Some(Container::Table { rows, row, .. }) = b.stack.last_mut() {
                        rows.push(std::mem::take(row));
                    }
                }
                TagEnd::Table => {
                    if let Some(Container::Table { align, head, rows, .. }) = b.stack.pop() {
                        b.push_block(Block::Table { align, head, rows });
                    }
                }
                TagEnd::Emphasis => {
                    b.italic = b.italic.saturating_sub(1);
                    b.sync_style();
                }
                TagEnd::Strong => {
                    b.bold = b.bold.saturating_sub(1);
                    b.sync_style();
                }
                TagEnd::Strikethrough => {
                    b.strike = b.strike.saturating_sub(1);
                    b.sync_style();
                }
                TagEnd::Link | TagEnd::Image => {
                    b.links.pop();
                    b.sync_style();
                }
                _ => {}
            },
            Event::Text(t) => b.push_text(&t, false),
            Event::Code(t) => b.push_text(&t, true),
            Event::InlineMath(t) | Event::DisplayMath(t) => b.push_text(&t, true),
            Event::Html(t) | Event::InlineHtml(t) => b.push_text(&t, false),
            Event::FootnoteReference(t) => b.push_text(&format!("[^{t}]"), false),
            Event::SoftBreak => b.push_text(" ", false),
            Event::HardBreak => b.push_text("\n", false),
            Event::Rule => {
                b.flush_inline();
                b.push_block(Block::Rule);
            }
            Event::TaskListMarker(done) => {
                if let Some(Container::Item { task, .. }) = b.stack.last_mut() {
                    *task = Some(done);
                }
            }
        }
    }
    // Unvollständige Eingabe (Streaming): offene Container sauber schließen.
    if let Some((lang, text)) = b.code.take() {
        b.push_block(Block::Code { lang, text });
    }
    b.flush_inline();
    while b.stack.len() > 1 {
        match b.stack.pop() {
            Some(Container::Quote(v)) => b.push_block(Block::Quote(v)),
            Some(Container::List { start, items }) => b.push_block(Block::List { start, items }),
            Some(Container::Item { task, blocks }) => {
                if let Some(Container::List { items, .. }) = b.stack.last_mut() {
                    items.push(ListItem { task, blocks });
                }
            }
            Some(Container::Table { align, head, rows, .. }) => b.push_block(Block::Table { align, head, rows }),
            Some(Container::Blocks(v)) => v.into_iter().for_each(|x| b.push_block(x)),
            None => {}
        }
    }
    match b.stack.pop() {
        Some(Container::Blocks(blocks)) => Doc { blocks },
        _ => Doc::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(t: &str) -> Span {
        Span { text: t.into(), style: Style::default() }
    }

    #[test]
    fn absatz_und_formatierung() {
        let d = parse("Hallo **fett** und *kursiv* mit `code` ~~weg~~.");
        let Block::Paragraph(s) = &d.blocks[0] else { panic!() };
        assert_eq!(s[0], plain("Hallo "));
        assert!(s[1].style.bold && s[1].text == "fett");
        assert!(s[3].style.italic);
        assert!(s[5].style.code && s[5].text == "code");
        assert!(s[7].style.strike);
    }

    #[test]
    fn ueberschrift_code_regel() {
        let d = parse("## Titel\n\n```rust\nfn main() {}\n```\n\n---\n");
        assert_eq!(d.blocks[0], Block::Heading { level: 2, text: vec![plain("Titel")] });
        assert_eq!(d.blocks[1], Block::Code { lang: "rust".into(), text: "fn main() {}".into() });
        assert_eq!(d.blocks[2], Block::Rule);
    }

    #[test]
    fn enge_und_aufgabenliste() {
        let d = parse("- [x] erledigt\n- [ ] offen\n  - verschachtelt\n");
        let Block::List { start, items } = &d.blocks[0] else { panic!() };
        assert_eq!(*start, None);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].task, Some(true));
        assert_eq!(items[1].task, Some(false));
        assert!(matches!(items[1].blocks[1], Block::List { .. }));
    }

    #[test]
    fn nummerierte_liste_und_zitat() {
        let d = parse("3. drei\n4. vier\n\n> zitat\n> weiter");
        assert!(matches!(d.blocks[0], Block::List { start: Some(3), .. }));
        let Block::Quote(q) = &d.blocks[1] else { panic!() };
        assert_eq!(q[0], Block::Paragraph(vec![plain("zitat weiter")]));
    }

    #[test]
    fn tabelle() {
        let d = parse("| a | b |\n|:--|--:|\n| 1 | **2** |\n");
        let Block::Table { align, head, rows } = &d.blocks[0] else { panic!() };
        assert_eq!(align, &vec![Align::Left, Align::Right]);
        assert_eq!(inline_text(&head[1]), "b");
        assert!(rows[0][1][0].style.bold);
    }

    #[test]
    fn links_bilder_html() {
        let d = parse("[pi](https://x.y) ![alt](bild.png) <b>roh</b>");
        let Block::Paragraph(s) = &d.blocks[0] else { panic!() };
        assert_eq!(s[0].style.link.as_deref(), Some("https://x.y"));
        assert!(s.iter().any(|x| x.text == "alt" && x.style.link.as_deref() == Some("bild.png")));
        assert!(inline_text(s).contains("<b>roh</b>"));
    }

    #[test]
    fn streaming_offener_codeblock() {
        let d = parse("Text\n\n```py\nprint(1)\n");
        assert_eq!(d.blocks[1], Block::Code { lang: "py".into(), text: "print(1)".into() });
    }

    #[test]
    fn harter_umbruch_und_plaintext() {
        let d = parse("a  \nb\n\n- x\n- y");
        assert_eq!(d.blocks[0], Block::Paragraph(vec![plain("a\nb")]));
        assert_eq!(d.plain_text(), "a\nb\n\nx\n\ny");
    }
}
