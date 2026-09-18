//! Checks over the site's content set that no single file can make on its own.
//!
//! The one that caused this file to exist: a page's copy was written into a new
//! item rather than into the item the alias points at, so `/why` kept serving a
//! placeholder while the real copy sat in the database at a URL nothing linked
//! to. Every individual file was valid. The set was wrong.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn config_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("config")
}

/// Every `.yml` in `config/` and in `config/docs/`.
fn config_files() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for dir in [config_dir(), config_dir().join("docs")] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yml")
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into();
                out.push((name, text));
            }
        }
    }
    out.sort();
    out
}

/// `key: value` from a flat YAML file, top level only.
fn scalar(text: &str, key: &str) -> Option<String> {
    text.lines()
        .map(str::trim_end)
        .find(|l| l.starts_with(&format!("{key}:")) && !l.starts_with(' '))
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.trim().trim_matches('\'').trim_matches('"').to_string())
}

/// One `url_alias.*.yml`: the item it points at, the path it claims, and the
/// language it claims it in. A page translated into a second language carries one
/// row per language, which is what the table's unique key is shaped for.
struct Alias {
    source: String,
    alias: String,
    language: String,
}

struct Content {
    items: HashMap<String, String>,
    aliases: Vec<Alias>,
}

fn content() -> Content {
    let mut items = HashMap::new();
    let mut aliases = Vec::new();

    for (name, text) in config_files() {
        if name.starts_with("item.") {
            let id = scalar(&text, "id").unwrap_or_default();
            let title = scalar(&text, "title").unwrap_or_default();
            items.insert(id, title);
        } else if name.starts_with("url_alias.") {
            aliases.push(Alias {
                source: scalar(&text, "source").unwrap_or_default(),
                alias: scalar(&text, "alias").unwrap_or_default(),
                language: scalar(&text, "language").unwrap_or_default(),
            });
        }
    }

    Content { items, aliases }
}

#[test]
fn every_alias_points_at_something_that_exists() {
    let c = content();
    let mut broken = Vec::new();
    for entry in &c.aliases {
        if let Some(id) = entry.source.strip_prefix("/item/")
            && !c.items.contains_key(id)
        {
            broken.push(format!(
                "{} points at /item/{id}, which no file declares",
                entry.alias
            ));
        }
    }
    assert!(broken.is_empty(), "\n{}\n", broken.join("\n"));
}

#[test]
fn no_item_is_orphaned() {
    // An item with no alias is still reachable at /item/{uuid} and still appears
    // in the sitemap, so it is a page nobody linked to that a crawler will find.
    // The front page is the exception: its address is `/`, which cannot be
    // aliased.
    let c = content();
    let aliased: HashSet<&str> = c
        .aliases
        .iter()
        .filter_map(|entry| entry.source.strip_prefix("/item/"))
        .collect();

    let front_page = "0193b000-0000-7000-8000-000000000001";
    let orphans: Vec<&String> = c
        .items
        .iter()
        .filter(|(id, _)| id.as_str() != front_page && !aliased.contains(id.as_str()))
        .map(|(_, title)| title)
        .collect();

    assert!(
        orphans.is_empty(),
        "items with no alias: {orphans:?}\nEither give each one a url_alias or delete it."
    );
}

#[test]
fn no_two_items_claim_the_same_title() {
    // Two items with one title is what a copy written into the wrong id looks
    // like: the old one keeps the alias and the new one keeps the words.
    let c = content();
    let mut by_title: HashMap<&str, usize> = HashMap::new();
    for title in c.items.values() {
        *by_title.entry(title.as_str()).or_default() += 1;
    }
    let duplicates: Vec<&str> = by_title
        .iter()
        .filter(|(_, n)| **n > 1)
        .map(|(t, _)| *t)
        .collect();
    assert!(duplicates.is_empty(), "duplicate titles: {duplicates:?}");
}

#[test]
fn no_two_aliases_claim_the_same_path_in_one_language() {
    // The database has a unique constraint on (alias, language, stage_id), so the
    // second import silently replaces the first and one of the two pages becomes
    // unreachable without an error anywhere.
    //
    // Keyed on the pair the constraint is keyed on. One path in two languages is
    // the shape a translated page needs — `/why` in `en` and in `it`, pointing at
    // the same item — and is how the kernel resolves a language-prefixed address
    // back to a reader-facing one.
    let c = content();
    let mut seen: HashMap<(&str, &str), usize> = HashMap::new();
    for entry in &c.aliases {
        *seen
            .entry((entry.alias.as_str(), entry.language.as_str()))
            .or_default() += 1;
    }
    let duplicates: Vec<String> = seen
        .iter()
        .filter(|(_, n)| **n > 1)
        .map(|((alias, language), _)| format!("{alias} ({language})"))
        .collect();
    assert!(duplicates.is_empty(), "duplicate aliases: {duplicates:?}");
}

#[test]
fn every_translated_page_has_an_alias_in_its_own_language() {
    // The kernel resolves a page's canonical address with
    // `get_canonical_alias_with_context(source, stage, language)`, which filters
    // `url_alias` by the language exactly. The forward lookup
    // (`find_by_alias_with_context`) falls back to `en`; this reverse one does
    // not. So an item translated into Italian and aliased only in English
    // resolves no canonical on `/it/…` and falls back to `/item/{uuid}` — which
    // then becomes its canonical link, its `hreflang` alternates and the address
    // the language switcher offers.
    //
    // Declaring the alias in both languages is what the table's unique key is
    // shaped for, and it is the site's own omission rather than something the
    // kernel does wrong. The asymmetry between the two lookups is recorded as a
    // finding.
    let path = config_dir().join("variable.plugin.trovato_site.site_translations.yml");
    let text = std::fs::read_to_string(&path).expect("the translations file is readable");
    let json = text.split_once("value: ").expect("the file has a value").1;
    let translations: serde_json::Value =
        serde_json::from_str(json).expect("the value is valid JSON");

    let front_page = "0193b000-0000-7000-8000-000000000001";
    let c = content();
    let mut missing = Vec::new();

    for (item_id, languages) in translations.as_object().into_iter().flatten() {
        // The front page's address is `/`, which cannot be aliased; the front
        // route names `/` and `/{lang}/` itself.
        if item_id == front_page {
            continue;
        }
        let source = format!("/item/{item_id}");
        for language in languages.as_object().into_iter().flatten().map(|(l, _)| l) {
            let has = c
                .aliases
                .iter()
                .any(|a| a.source == source && &a.language == language);
            if !has {
                missing.push(format!(
                    "{item_id}: translated into {language}, no {language} alias"
                ));
            }
        }
    }
    assert!(missing.is_empty(), "\n{}\n", missing.join("\n"));
}

/// A file's reader-visible copy: comment lines removed, and nothing at all for a
/// mirrored document.
///
/// The comment stripping matters. A YAML comment is written for whoever edits the
/// file and is never served, so holding it to the rules that govern published
/// prose would fail the build over a note to a maintainer.
fn prose(name: &str, text: &str) -> Option<String> {
    if name.starts_with("item.") && text.contains("type: docs") {
        return None;
    }
    let without_comments: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    Some(without_code(&without_comments))
}

/// The same text with every `<code>` and `<pre>` span blanked out.
///
/// A double hyphen in prose is a dash somebody typed on a keyboard that has no
/// em dash. A double hyphen in `cargo build --release` is a flag. Only the first
/// is a writing problem, and the difference is which element it is inside.
fn without_code(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(open) = rest.find("<code").or_else(|| rest.find("<pre")) {
        out.push_str(&rest[..open]);
        let tail = &rest[open..];
        let closer = if tail.starts_with("<pre") {
            "</pre>"
        } else {
            "</code>"
        };
        match tail.find(closer) {
            Some(close) => rest = &tail[close + closer.len()..],
            // An unclosed element: keep the remainder rather than dropping the
            // rest of the file from the check.
            None => {
                rest = "";
                break;
            }
        }
    }

    out.push_str(rest);
    out
}

#[test]
fn no_page_still_carries_a_placeholder() {
    let mut found = Vec::new();
    for (name, text) in config_files() {
        // This marker is the site's own and cannot legitimately appear anywhere,
        // a mirrored document included.
        if text.contains("PLACEHOLDER-PHASE-5") {
            found.push(format!("{name}: PLACEHOLDER-PHASE-5"));
        }
        let Some(body) = prose(&name, &text) else {
            continue;
        };
        for marker in ["PLACEHOLDER", "TODO", "Lorem ipsum", "FIXME", "XXX"] {
            if body.contains(marker) {
                found.push(format!("{name}: {marker}"));
            }
        }
    }
    assert!(found.is_empty(), "\n{}\n", found.join("\n"));
}

#[test]
fn the_copy_uses_no_dash_punctuation() {
    // An em dash, an en dash or a double hyphen in published prose reads as
    // machine writing, and this site is written in one person's voice.
    //
    // Two exemptions, both deliberate. A mirrored document is somebody else's
    // prose reproduced, and repunctuating it would make it a paraphrase. And a
    // YAML comment is written for whoever edits the file, never served, and
    // follows the kernel repository's own style, which uses them freely.
    let mut found = Vec::new();
    for (name, text) in config_files() {
        let Some(body) = prose(&name, &text) else {
            continue;
        };
        for (label, needle) in [
            ("em dash", "\u{2014}"),
            ("en dash", "\u{2013}"),
            ("double hyphen", "--"),
        ] {
            if let Some(at) = body.find(needle) {
                let line = body[..at].lines().count();
                found.push(format!("{name}:{line}: {label}"));
            }
        }
    }
    assert!(found.is_empty(), "\n{}\n", found.join("\n"));
}

#[test]
fn every_translation_names_an_item_that_exists() {
    // The translations are a config variable keyed by item id, so nothing checks
    // the reference. A typo produces a row in `item_translation` that overlays
    // nothing and is never noticed.
    let path = config_dir().join("variable.plugin.trovato_site.site_translations.yml");
    let text = std::fs::read_to_string(&path).expect("the translations file is readable");
    let json = text
        .split_once("value: ")
        .map(|(_, v)| v.to_string())
        .expect("the file has a value");
    let value: serde_json::Value = serde_json::from_str(&json).expect("the value is valid JSON");

    let c = content();
    let mut unknown = Vec::new();
    for item_id in value.as_object().into_iter().flatten().map(|(k, _)| k) {
        if !c.items.contains_key(item_id) {
            unknown.push(item_id.clone());
        }
    }
    assert!(
        unknown.is_empty(),
        "translations for items that do not exist: {unknown:?}"
    );
}

/// How a field is stored, in the two shapes this site's items use.
///
/// `{value, format}` is what a filtered field carries and what the renderer
/// looks for. A bare string is what the front page's `body` and `proof` carry on
/// purpose, so that the handler passes them through untouched and the template
/// places them.
fn field_shape(field: &serde_json::Value) -> &'static str {
    if field.get("value").and_then(|v| v.as_str()).is_some()
        && field.get("format").and_then(|v| v.as_str()).is_some()
    {
        "{value, format}"
    } else if field.is_string() {
        "a bare string"
    } else {
        "neither {value, format} nor a bare string"
    }
}

#[test]
fn a_translated_field_has_the_shape_the_item_it_overlays_stores() {
    // apply_translation_overlay merges the translation's fields into the item's,
    // key for key, so the shape a translation stores has to be the shape that
    // item stores — not one shape for the whole site. A `{value, format}` field
    // translated as a bare string replaces a renderable field with one the
    // renderer skips; a bare string translated as `{value, format}` renders the
    // object's debug spelling into the page. Both are silent, and both are
    // visible only to somebody reading the translated page.
    //
    // Checked against each item's own file rather than against a constant,
    // because the front page's fields are deliberately bare strings and the
    // other four pages' are deliberately not.
    let path = config_dir().join("variable.plugin.trovato_site.site_translations.yml");
    let text = std::fs::read_to_string(&path).expect("the translations file is readable");
    let json = text.split_once("value: ").expect("the file has a value").1;
    let value: serde_json::Value = serde_json::from_str(json).expect("the value is valid JSON");

    let mut wrong = Vec::new();
    for (item_id, languages) in value.as_object().into_iter().flatten() {
        let item_path = config_dir().join(format!("item.{item_id}.yml"));
        let item_text =
            std::fs::read_to_string(&item_path).expect("the translated item's file is readable");
        let item: serde_json::Value =
            serde_yml::from_str(&item_text).expect("the item file is valid YAML");
        let item_fields = item.get("fields").and_then(|f| f.as_object());

        for (language, translation) in languages.as_object().into_iter().flatten() {
            let Some(fields) = translation.get("fields").and_then(|f| f.as_object()) else {
                wrong.push(format!("{item_id}/{language}: no fields"));
                continue;
            };
            for (name, field) in fields {
                let Some(stored) = item_fields.and_then(|f| f.get(name)) else {
                    wrong.push(format!(
                        "{item_id}/{language}/{name}: the item has no such field"
                    ));
                    continue;
                };
                let (want, got) = (field_shape(stored), field_shape(field));
                if want != got {
                    wrong.push(format!(
                        "{item_id}/{language}/{name}: the item stores {want}, the translation is {got}"
                    ));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "\n{}\n", wrong.join("\n"));
}

#[test]
fn the_translated_pages_are_the_ones_that_were_promised() {
    // Five: the four core pages and the front page. The front page was absent
    // until 0.102.0, because routes/front.rs rendered the configured front-page
    // item without applying a translation overlay and a front-page translation
    // was configuration nothing read. It applies the overlay now and serves the
    // result at `/it/`, which is also the address the switcher and the hreflang
    // alternates name.
    let path = config_dir().join("variable.plugin.trovato_site.site_translations.yml");
    let text = std::fs::read_to_string(&path).expect("the translations file is readable");
    let json = text.split_once("value: ").expect("the file has a value").1;
    let value: serde_json::Value = serde_json::from_str(json).expect("the value is valid JSON");

    let count = value.as_object().map_or(0, serde_json::Map::len);
    assert_eq!(count, 5, "expected five translated pages, found {count}");

    let front = "0193b000-0000-7000-8000-000000000001";
    assert!(
        value.get(front).is_some(),
        "the front page is translated from 0.102.0 on; see docs/LEDGER.md"
    );
}

#[test]
fn no_translation_hand_builds_its_own_language_links() {
    // Until 0.102.0 a template could not tell which language it was rendering
    // and could not work out a page's address in another language, so each
    // Italian page carried a paragraph of hand-written links to the other
    // Italian pages and back to English. The kernel supplies
    // `available_translations` and `hreflang_links` now, the footer builds a
    // real switcher from them, and the menus follow the language — so a page
    // that hand-builds them again is a page whose links will drift out of step
    // with the ones the kernel emits.
    let path = config_dir().join("variable.plugin.trovato_site.site_translations.yml");
    let text = std::fs::read_to_string(&path).expect("the translations file is readable");
    for needle in ["In italiano:", "hreflang="] {
        assert!(
            !text.contains(needle),
            "a translation carries a hand-built language link ({needle}); \
             the footer switcher and the kernel's hreflang_links cover this"
        );
    }
}
