//! Mirror the kernel's documentation into this site's config.
//!
//!     cargo run -p tools --bin docs-import
//!     cargo run -p tools --bin docs-import -- --check
//!
//! Fetches each document from the public Trovato repository at the pinned tag,
//! renders it, and writes the result into `config/docs/` and `static/docs/`. The
//! output is committed, so a deploy imports it like any other config and
//! production needs nothing from the machine that ran this.
//!
//! **Never from a local checkout.** The source is `raw.githubusercontent.com` at
//! an exact tag. A generator that reads the working tree next door produces a
//! site that matches whatever that tree happened to contain, which is how a
//! documentation mirror quietly stops matching the release it claims to mirror.
//!
//! Idempotent: same tag in, same bytes out. `--check` runs the whole thing and
//! reports whether anything would change, without writing, which is what CI runs.
#![allow(clippy::print_stderr, clippy::print_stdout)]

mod docs;
mod highlight;
mod render;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use docs::Doc;

/// The release the site mirrors. One line, and the only thing to change when the
/// kernel releases.
const TAG: &str = "v0.101.0";

const REPO: &str = "jeremyandrews/trovato";

/// The UUID namespace the site's generated items are named in.
///
/// A document's id is `uuid5(NAMESPACE, slug)`, so it is the same on every
/// machine and in every database, and re-importing updates a row rather than
/// adding one beside it. Randomly generated once, then fixed forever.
const NAMESPACE: uuid::Uuid = uuid::uuid!("0193b000-0006-7000-8000-000000000000");

/// The live stage.
const LIVE_STAGE: &str = "0193a5a0-0000-7000-8000-000000000001";

/// Fixed timestamp for generated items.
///
/// Not "now": a generator whose output depends on when it ran cannot be checked
/// for idempotence, and every re-run would rewrite every file. The date a
/// document was written is the kernel repository's business; what this site
/// records is the release it was mirrored from, which is on the page.
const GENERATED_AT: i64 = 1_785_542_400; // 2026-08-01T00:00:00Z

fn main() -> std::process::ExitCode {
    let check_only = std::env::args().any(|a| a == "--check");
    let root = repo_root();

    let config_dir = root.join("config/docs");

    let mut fetched: BTreeMap<&'static str, String> = BTreeMap::new();
    for doc in docs::all() {
        let url = raw_url(doc.source);
        match fetch(&url) {
            Ok(body) => {
                println!("  fetched {} ({} bytes)", doc.source, body.len());
                fetched.insert(doc.slug, body);
            }
            Err(e) => {
                eprintln!("could not fetch {url}: {e}");
                return std::process::ExitCode::FAILURE;
            }
        }
    }

    let mut files: BTreeMap<PathBuf, String> = BTreeMap::new();
    let ordered = docs::all();
    let mut internal_links = 0usize;
    let mut outbound_links = 0usize;

    for (index, doc) in ordered.iter().enumerate() {
        let Some(markdown) = fetched.get(doc.slug) else {
            continue;
        };

        let rendered = render::render(markdown, &|link| resolve_link(link));
        for link in &rendered.links {
            match resolve_link(link) {
                Some(target) if target.starts_with("/learn/") => internal_links += 1,
                Some(_) => outbound_links += 1,
                None => {}
            }
        }
        let previous = index.checked_sub(1).and_then(|i| ordered.get(i)).copied();
        let next = ordered.get(index + 1).copied();

        files.insert(
            config_dir.join(format!("item.{}.yml", item_id(doc.slug))),
            item_yaml(doc, &rendered.html, markdown, previous, next),
        );
        files.insert(
            config_dir.join(format!("url_alias.{}.yml", alias_id(doc.slug))),
            alias_yaml(doc),
        );
    }

    files.insert(
        root.join("config/gather_query.trovato_site.docs_index.yml"),
        docs_index_query(),
    );
    // `canonical_url` on a gather query only creates the redirect from
    // `/gather/{id}`; the friendly path itself is served by an alias.
    // Named by its own id, like every other exported entity: `config import`
    // warns when a filename and the id inside it disagree.
    files.insert(
        root.join(format!("config/url_alias.{}.yml", index_alias_id())),
        index_alias(),
    );

    let changed = write_all(&files, check_only);

    // Anything left over from a document that used to be mirrored and is not any
    // more. A generator that only writes leaves the site serving pages that its
    // own manifest no longer lists.
    let stale = find_stale(&config_dir, &files);
    for path in &stale {
        println!("  stale: {}", relative(&root, path));
        if !check_only {
            let _ = std::fs::remove_file(path);
        }
    }

    println!(
        "\n{} documents from {REPO} at {TAG}\n\
         {} link(s) rewritten to this site, {} left pointing at GitHub\n\
         {} file(s) written, {} stale file(s)",
        ordered.len(),
        internal_links,
        outbound_links,
        changed.len(),
        stale.len()
    );

    if check_only && (!changed.is_empty() || !stale.is_empty()) {
        eprintln!("\nthe import is not up to date. Run it and commit the result:");
        for path in changed.iter().take(20) {
            eprintln!("  {}", relative(&root, path));
        }
        return std::process::ExitCode::FAILURE;
    }

    std::process::ExitCode::SUCCESS
}

/// Where a document's markdown is fetched from.
fn raw_url(source: &str) -> String {
    format!("https://raw.githubusercontent.com/{REPO}/{TAG}/{source}")
}

/// Where a reader edits it.
fn edit_url(source: &str) -> String {
    format!("https://github.com/{REPO}/blob/{TAG}/{source}")
}

fn fetch(url: &str) -> Result<String, String> {
    let mut response = ureq::get(url).call().map_err(|e| e.to_string())?;
    response
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())
}

/// Rewrite a link written for the kernel repository into one for this site.
///
/// A link to a document the site mirrors becomes the site's path. A link to
/// anything else in the repository becomes an absolute GitHub URL, because a
/// relative path that made sense in `docs/` means nothing under `/learn`. An
/// anchor or an external URL is left alone.
fn resolve_link(link: &str) -> Option<String> {
    if link.starts_with('#') || link.contains("://") || link.starts_with("mailto:") {
        return None;
    }

    let (path, fragment) = match link.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (link, None),
    };

    let normalized = normalize(path);

    if let Some(doc) = docs::all().iter().find(|d| d.source == normalized) {
        return Some(match fragment {
            Some(f) => format!("/learn/{}#{f}", doc.slug),
            None => format!("/learn/{}", doc.slug),
        });
    }

    Some(format!("https://github.com/{REPO}/blob/{TAG}/{normalized}"))
}

/// Resolve a repository-relative link against the `docs/` directory most of them
/// are written from.
///
/// The documents live at several depths and their links are written relative to
/// wherever each one sits, so `../../INSTALL.md` from `docs/tutorial/` and
/// `design/Overview.md` from `docs/` both have to land on the same repository
/// path. Rather than track each document's directory, both spellings of every
/// mirrored file are tried against the manifest, which is small and exact.
fn normalize(path: &str) -> String {
    let cleaned = path.trim_start_matches("./");
    let bare = cleaned.rsplit('/').next().unwrap_or(cleaned);

    for candidate in [
        cleaned.to_string(),
        format!("docs/{cleaned}"),
        cleaned.trim_start_matches("../").to_string(),
        cleaned.trim_start_matches("../../").to_string(),
    ] {
        if docs::all().iter().any(|d| d.source == candidate) {
            return candidate;
        }
    }

    // A last resort for a link written with a depth this does not model: match on
    // the file name alone, which is unique across the manifest.
    if let Some(doc) = docs::all().iter().find(|d| {
        d.source
            .rsplit('/')
            .next()
            .is_some_and(|name| name == bare && bare.ends_with(".md"))
    }) {
        return doc.source.to_string();
    }

    cleaned.trim_start_matches("../").to_string()
}

fn item_id(slug: &str) -> uuid::Uuid {
    uuid::Uuid::new_v5(&NAMESPACE, slug.as_bytes())
}

fn alias_id(slug: &str) -> uuid::Uuid {
    uuid::Uuid::new_v5(&NAMESPACE, format!("alias:{slug}").as_bytes())
}

/// One document, as a config item.
fn item_yaml(
    doc: &Doc,
    html: &str,
    markdown: &str,
    previous: Option<&Doc>,
    next: Option<&Doc>,
) -> String {
    let mut fields = serde_json::Map::new();
    fields.insert("html".into(), serde_json::Value::String(html.to_string()));
    fields.insert(
        "markdown".into(),
        serde_json::Value::String(markdown.to_string()),
    );
    fields.insert(
        "source_path".into(),
        serde_json::Value::String(doc.source.to_string()),
    );
    fields.insert(
        "edit_url".into(),
        serde_json::Value::String(edit_url(doc.source)),
    );
    fields.insert(
        "raw_url".into(),
        serde_json::Value::String(format!("/learn/raw/{}", doc.slug)),
    );
    fields.insert(
        "section".into(),
        serde_json::Value::String(doc.section.title().to_string()),
    );
    fields.insert(
        "section_blurb".into(),
        serde_json::Value::String(doc.section.blurb().to_string()),
    );
    fields.insert(
        "weight".into(),
        serde_json::Value::from(order_of(doc.slug) as i64),
    );
    if let Some(p) = previous {
        fields.insert(
            "prev_path".into(),
            serde_json::Value::String(format!("/learn/{}", p.slug)),
        );
        fields.insert(
            "prev_title".into(),
            serde_json::Value::String(p.title.to_string()),
        );
    }
    if let Some(n) = next {
        fields.insert(
            "next_path".into(),
            serde_json::Value::String(format!("/learn/{}", n.slug)),
        );
        fields.insert(
            "next_title".into(),
            serde_json::Value::String(n.title.to_string()),
        );
    }

    let item = serde_json::json!({
        "id": item_id(doc.slug).to_string(),
        "type": "docs",
        "title": doc.title,
        "language": "en",
        "status": 1,
        "created": GENERATED_AT,
        "changed": GENERATED_AT,
        "fields": serde_json::Value::Object(fields),
    });

    let body = serde_yml::to_string(&item).unwrap_or_default();
    format!(
        "# Generated by `cargo run -p tools --bin docs-import`. Do not edit.\n\
         # Mirrored from {REPO} {source} at {TAG}.\n{body}",
        source = doc.source
    )
}

fn alias_yaml(doc: &Doc) -> String {
    let alias = serde_json::json!({
        "id": alias_id(doc.slug).to_string(),
        "source": format!("/item/{}", item_id(doc.slug)),
        "alias": format!("/learn/{}", doc.slug),
        "language": "en",
        "stage_id": LIVE_STAGE,
        "created": GENERATED_AT,
    });
    let body = serde_yml::to_string(&alias).unwrap_or_default();
    format!("# Generated by `cargo run -p tools --bin docs-import`. Do not edit.\n{body}")
}

/// The listing behind `/learn`.
fn docs_index_query() -> String {
    format!(
        "# Generated by `cargo run -p tools --bin docs-import`. Do not edit.\n\
         #\n\
         # The documentation index. Sorted by the weight the generator assigns from\n\
         # the order of the manifest in tools/src/docs.rs, so the index reads in the\n\
         # order the documents are meant to be read rather than alphabetically.\n\
         query_id: trovato_site.docs_index\n\
         label: Learn\n\
         description: The Trovato documentation, mirrored from the kernel repository at {TAG}.\n\
         definition:\n\
         \x20 base_table: item\n\
         \x20 item_type: docs\n\
         \x20 fields: []\n\
         \x20 filters: []\n\
         \x20 includes: {{}}\n\
         \x20 relationships: []\n\
         \x20 sorts:\n\
         \x20 - direction: asc\n\
         \x20   field: fields.weight\n\
         \x20   nulls: null\n\
         \x20 stage_aware: true\n\
         display:\n\
         \x20 canonical_url: /learn\n\
         \x20 empty_text: The documentation has not been imported yet.\n\
         \x20 footer: null\n\
         \x20 format: list\n\
         \x20 header: null\n\
         \x20 items_per_page: 200\n\
         \x20 pager:\n\
         \x20   enabled: false\n\
         \x20   show_count: false\n\
         \x20   style: full\n\
         plugin: trovato_site\n"
    )
}

/// The alias that makes `/learn` the documentation index.
fn index_alias_id() -> uuid::Uuid {
    uuid::Uuid::new_v5(&NAMESPACE, b"alias:docs-index")
}

fn index_alias() -> String {
    let alias = serde_json::json!({
        "id": index_alias_id().to_string(),
        "source": "/gather/trovato_site.docs_index",
        "alias": "/learn",
        "language": "en",
        "stage_id": LIVE_STAGE,
        "created": GENERATED_AT,
    });
    let body = serde_yml::to_string(&alias).unwrap_or_default();
    format!("# Generated by `cargo run -p tools --bin docs-import`. Do not edit.\n{body}")
}

/// A document's position in the reading order, which is its sort weight.
fn order_of(slug: &str) -> usize {
    docs::all()
        .iter()
        .position(|d| d.slug == slug)
        .unwrap_or(usize::MAX)
        * 10
}

/// Write every file whose content would change, and return those paths.
fn write_all(files: &BTreeMap<PathBuf, String>, check_only: bool) -> Vec<PathBuf> {
    let mut changed = Vec::new();
    for (path, content) in files {
        let current = std::fs::read_to_string(path).ok();
        if current.as_deref() == Some(content.as_str()) {
            continue;
        }
        changed.push(path.clone());
        if check_only {
            continue;
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(path, content) {
            eprintln!("could not write {}: {e}", path.display());
        }
    }
    changed
}

/// Files in the generated directories that this run did not produce.
fn find_stale(config_dir: &Path, written: &BTreeMap<PathBuf, String>) -> Vec<PathBuf> {
    let mut stale = Vec::new();
    for dir in [config_dir] {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() && !written.contains_key(&path) {
                stale.push(path);
            }
        }
    }
    stale.sort();
    stale
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}
