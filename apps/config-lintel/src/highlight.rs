//! Syntax highlighting for the source editor.
//!
//! `egui_extras`' built-in highlighter covers YAML, TOML and JSON; XML isn't one of its languages,
//! so it gets a small tokenizer here that reuses the same [`CodeTheme`] colours.

use egui::text::LayoutJob;
use egui_extras::syntax_highlighting::{self, CodeTheme, TokenType};

use crate::config::Format;

pub fn highlight(
    ctx: &egui::Context,
    style: &egui::Style,
    theme: &CodeTheme,
    code: &str,
    format: Format,
) -> LayoutJob {
    let language = match format {
        Format::Yaml => "yaml",
        Format::Toml => "toml",
        Format::Json => "json",
        Format::Xml => return highlight_xml_cached(ctx, theme, code),
    };
    syntax_highlighting::highlight(ctx, style, theme, code, language)
}

#[derive(Default)]
struct XmlHighlighter;

impl egui::cache::ComputerMut<(&CodeTheme, &str), LayoutJob> for XmlHighlighter {
    fn compute(&mut self, (theme, code): (&CodeTheme, &str)) -> LayoutJob {
        highlight_xml(theme, code)
    }
}

/// Memoized like `egui_extras`' highlighter, so calling it every frame is cheap.
fn highlight_xml_cached(ctx: &egui::Context, theme: &CodeTheme, code: &str) -> LayoutJob {
    type XmlCache = egui::cache::FrameCache<LayoutJob, XmlHighlighter>;
    ctx.memory_mut(|mem| mem.caches.cache::<XmlCache>().get((theme, code)).clone())
}

fn highlight_xml(theme: &CodeTheme, code: &str) -> LayoutJob {
    let mut job = LayoutJob::default();
    let mut push = |text: &str, token: TokenType| {
        if !text.is_empty() {
            job.append(text, 0.0, theme.format(token).clone());
        }
    };

    let mut rest = code;
    while !rest.is_empty() {
        // Comments, CDATA, declarations and processing instructions run to their terminator.
        let special = [
            ("<!--", "-->", TokenType::Comment),
            ("<![CDATA[", "]]>", TokenType::StringLiteral),
            ("<?", "?>", TokenType::Keyword),
            ("<!", ">", TokenType::Keyword),
        ];
        if let Some((_, end, token)) = special.iter().find(|(start, ..)| rest.starts_with(start)) {
            let len = rest.find(end).map_or(rest.len(), |i| i + end.len());
            push(&rest[..len], *token);
            rest = &rest[len..];
            continue;
        }

        if rest.starts_with('<') {
            let len = tag_len(rest);
            highlight_tag(&rest[..len], &mut push);
            rest = &rest[len..];
            continue;
        }

        // Text content up to the next tag. (`Whitespace` would render it invisible.)
        let len = rest.find('<').unwrap_or(rest.len());
        push(&rest[..len], TokenType::Literal);
        rest = &rest[len..];
    }
    job
}

/// Length of the tag starting at `s[0] == '<'`, up to and including its `>` (quoted `>` don't end it).
fn tag_len(s: &str) -> usize {
    let mut quote = None;
    for (i, c) in s.char_indices().skip(1) {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '>') => return i + 1,
            _ => {}
        }
    }
    s.len()
}

/// `<name attr="value" …>`: punctuation, the element name as a keyword, attribute names as
/// functions and their values as strings.
fn highlight_tag(tag: &str, push: &mut impl FnMut(&str, TokenType)) {
    let mut rest = tag;
    let mut seen_name = false;
    while let Some(c) = rest.chars().next() {
        let len = if matches!(c, '"' | '\'') {
            let len = rest[1..].find(c).map_or(rest.len(), |i| i + 2);
            push(&rest[..len], TokenType::StringLiteral);
            len
        } else if c.is_whitespace() {
            let len = rest
                .find(|c: char| !c.is_whitespace())
                .unwrap_or(rest.len());
            push(&rest[..len], TokenType::Whitespace);
            len
        } else if is_name_char(c) {
            let len = rest.find(|c| !is_name_char(c)).unwrap_or(rest.len());
            let token = if seen_name {
                TokenType::Function
            } else {
                TokenType::Keyword
            };
            push(&rest[..len], token);
            seen_name = true;
            len
        } else {
            push(&rest[..c.len_utf8()], TokenType::Punctuation);
            c.len_utf8()
        };
        rest = &rest[len..];
    }
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_tokens_cover_the_whole_input() {
        let theme = CodeTheme::dark(12.0);
        let code = "<?xml version=\"1.0\"?>\n<!-- c -->\n<a x='1>2'>text<b/></a>\n<![CDATA[<x>]]>";
        assert_eq!(highlight_xml(&theme, code).text, code);
        // Unterminated constructs don't panic or drop text.
        for partial in ["<a x=\"", "<!-- open", "<", "<a"] {
            assert_eq!(highlight_xml(&theme, partial).text, partial);
        }
    }
}
