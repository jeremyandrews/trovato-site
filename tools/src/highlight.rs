//! Syntax highlighting, done once when a document is imported.
//!
//! syntect is used as a lexer and its themes are not used at all. It emits
//! `<span class="tok-keyword tok-source tok-rust">` and the site's own stylesheet
//! decides what those look like.
//!
//! Two reasons not to bake a theme into the HTML. The site has a dark scheme, and
//! colors written into the markup at import time cannot follow the reader's
//! setting. And a page whose colors come from `static/css/` is a page whose
//! contrast the `checks` crate can verify; a page whose colors are inline styles
//! in a database row is not.

use std::sync::OnceLock;

use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

/// The class prefix, so the site's token classes cannot collide with anything
/// else on the page.
const CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "tok-" };

fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Render one fenced code block.
///
/// An unknown or absent language is emitted as escaped text in the same markup,
/// so a fence the site cannot lex still looks like code.
pub fn block(lang: &str, body: &str) -> String {
    let language = lang
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();

    let mut html = String::from("<figure class=\"code\">");
    if !language.is_empty() {
        html.push_str(&format!(
            "<figcaption class=\"code__lang\">{}</figcaption>",
            escape(&language)
        ));
    }
    // `tabindex="0"` because the block scrolls sideways when a line is longer
    // than the column, and a scrollable region that cannot take focus cannot be
    // scrolled without a mouse. `aria-label` so the stop in the tab order has a
    // name rather than being a mystery.
    //
    // Deliberately not `role="region"`. That was the first version, and it makes
    // every code block a landmark: a mirrored tutorial has more than a hundred of
    // them, so the landmark list became unusable and axe reported every one as a
    // duplicate name. A scrollable region has to be focusable; it does not have
    // to be a landmark.
    html.push_str(&format!(
        "<pre class=\"code__body\" tabindex=\"0\" aria-label=\"{} code\"><code",
        escape(if language.is_empty() {
            "Example"
        } else {
            &language
        })
    ));
    if !language.is_empty() {
        html.push_str(&format!(" class=\"language-{}\"", escape(&language)));
    }
    html.push('>');
    html.push_str(&tokens(&language, body));
    html.push_str("</code></pre></figure>");
    html
}

/// The body of a code block: escaped text wrapped in token spans.
fn tokens(language: &str, body: &str) -> String {
    let set = syntaxes();
    let Some(syntax) = find_syntax(set, language) else {
        return escape(body);
    };

    let mut generator = ClassedHTMLGenerator::new_with_class_style(syntax, set, CLASS_STYLE);
    for line in LinesWithEndings::from(body) {
        // A line the lexer refuses is a highlighting problem, not a content
        // problem: the document still has to render. Falling back to the escaped
        // source for the whole block keeps it readable rather than half-marked.
        if generator
            .parse_html_for_line_which_includes_newline(line)
            .is_err()
        {
            return escape(body);
        }
    }
    generator.finalize()
}

/// The syntax for a fence's language tag.
///
/// The names are the ones a writer actually types in a fence, mapped to what
/// syntect calls them.
fn find_syntax<'a>(set: &'a SyntaxSet, language: &str) -> Option<&'a SyntaxReference> {
    let name = match language {
        "rust" | "rs" => "Rust",
        "toml" => "TOML",
        "yaml" | "yml" => "YAML",
        "json" => "JSON",
        "sql" => "SQL",
        "sh" | "bash" | "shell" | "console" | "zsh" => "Bourne Again Shell (bash)",
        "html" => "HTML",
        "css" => "CSS",
        "js" | "javascript" => "JavaScript",
        "xml" => "XML",
        "diff" | "patch" => "Diff",
        "" | "text" | "txt" | "plain" | "output" | "env" => return None,
        other => {
            return set
                .find_syntax_by_token(other)
                .or_else(|| set.find_syntax_by_extension(other));
        }
    };
    set.find_syntax_by_name(name)
}

/// Escape text for HTML.
pub fn escape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for c in raw.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn rust_is_tokenised() {
        let html = block("rust", "fn main() { let x = 1; }\n");
        assert!(html.contains("class=\"language-rust\""));
        assert!(html.contains("tok-"), "no token spans in: {html}");
        assert!(html.contains("main"));
    }

    #[test]
    fn an_unknown_language_still_renders_as_code() {
        let html = block("brainfuck-9000", "++++.\n");
        assert!(html.contains("<pre class=\"code__body\""));
        assert!(html.contains("++++."));
    }

    #[test]
    fn a_code_block_can_be_reached_and_named_by_a_keyboard() {
        // The block scrolls sideways when a line is longer than the column, and
        // a scrollable region that cannot take focus cannot be scrolled without a
        // mouse. axe reports it as `scrollable-region-focusable`.
        let html = block("rust", "fn f() {}\n");
        assert!(html.contains("tabindex=\"0\""));
        assert!(html.contains("aria-label=\"rust code\""));
        // Not a landmark: a page of mirrored documentation has more than a
        // hundred code blocks, and a hundred landmarks is not a landmark list.
        assert!(!html.contains("role=\"region\""));

        // A fence with no language still gets a name, because a tab stop with no
        // name is worse than one with a dull name.
        assert!(block("", "output\n").contains("aria-label=\"Example code\""));
    }

    #[test]
    fn a_fence_with_no_language_has_no_caption() {
        let html = block("", "some output\n");
        assert!(!html.contains("code__lang"));
        assert!(html.contains("some output"));
    }

    #[test]
    fn markup_in_a_code_block_cannot_escape_it() {
        let html = block("html", "<script>alert(1)</script>\n");
        // The lexer splits the tag across token spans — `&lt;` in one, `script`
        // in the next — so the escaped form is not contiguous and cannot be
        // asserted as one string. What matters is that no tag survives: every
        // angle bracket in the source is an entity, and the only `<` in the
        // output opens a span the generator wrote.
        assert!(!html.contains("<script"), "{html}");
        assert!(!html.contains("script>"), "{html}");
        assert!(html.contains("&lt;"), "{html}");
        assert!(html.contains("&gt;"), "{html}");
        assert!(html.contains("alert"), "{html}");
    }

    #[test]
    fn markup_in_an_unlexed_block_cannot_escape_it_either() {
        let html = block("nothing-real", "<script>alert(1)</script>\n");
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn the_language_tag_cannot_inject_an_attribute() {
        let html = block("rust\" onload=\"evil()", "fn f() {}\n");
        assert!(!html.contains("onload"));
    }
}
