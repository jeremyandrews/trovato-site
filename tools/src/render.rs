//! Markdown to HTML, with the code highlighted and the links rewritten.

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use crate::highlight;

/// A rendered document.
pub struct Rendered {
    pub html: String,
    /// Every link the document points at, for the link check.
    pub links: Vec<String>,
}

/// Render one document.
///
/// `resolve` maps a link as written in the kernel repository to where it lives on
/// this site, or returns `None` to leave it pointing at GitHub.
pub fn render(markdown: &str, resolve: &dyn Fn(&str) -> Option<String>) -> Rendered {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);

    let mut links = Vec::new();
    let mut out = String::with_capacity(markdown.len() * 2);

    // Code fences are collected whole and handed to the highlighter, so the
    // events inside them never reach the HTML writer.
    let mut fence: Option<(String, String)> = None;

    let parser = Parser::new_ext(markdown, options);
    let mut events: Vec<Event> = Vec::new();

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                fence = Some((lang.to_string(), String::new()));
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Indented)) => {
                fence = Some((String::new(), String::new()));
            }
            Event::Text(text) if fence.is_some() => {
                if let Some((_, body)) = fence.as_mut() {
                    body.push_str(&text);
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((lang, body)) = fence.take() {
                    out.push_str(&highlight::block(&lang, &body));
                }
            }
            Event::Start(Tag::Link { ref dest_url, .. }) => {
                links.push(dest_url.to_string());
                let target = resolve(dest_url).unwrap_or_else(|| dest_url.to_string());
                let mut rewritten = event.clone();
                if let Event::Start(Tag::Link {
                    ref mut dest_url, ..
                }) = rewritten
                {
                    *dest_url = target.into();
                }
                events.push(rewritten);
                flush(&mut events, &mut out);
            }
            other => {
                events.push(other);
                flush(&mut events, &mut out);
            }
        }
    }

    flush(&mut events, &mut out);

    Rendered { html: out, links }
}

/// Write the buffered events out as HTML.
fn flush(events: &mut Vec<Event>, out: &mut String) {
    if events.is_empty() {
        return;
    }
    pulldown_cmark::html::push_html(out, events.drain(..));
}
