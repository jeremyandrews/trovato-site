//! The site's search page loads the kernel's in-browser search, and every AI
//! stage is off.
//!
//! `templates/search.html` overrides the kernel's template, so it has to carry
//! the three things the kernel's own template adds for the in-browser search:
//! the stylesheet link, the `scolta-config` data block and the `scolta.js`
//! script. Without the script, `search-init.js` returns at once and the browser
//! never asks for the index. The first test reads the kernel's template at the
//! pinned tag, from `raw.githubusercontent.com` as the documentation mirror
//! does (`tools/src/docs_import.rs`), and fails if the site's copy of any of
//! the three is missing or differs. It needs network access to GitHub.
//!
//! The other tests need no network: the site loads the kernel's files rather
//! than copies of them, and its search settings switch every AI stage off.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read(relative: &str) -> String {
    let path = root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()))
}

/// The pinned release, from the file that authors it.
fn version() -> String {
    read("kernel-release.toml")
        .lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("version = \"")?.strip_suffix('"'))
        .expect("kernel-release.toml declares version")
        .to_string()
}

/// The kernel's `templates/search.html` at the pinned tag.
fn kernel_template() -> String {
    let url = format!(
        "https://raw.githubusercontent.com/jeremyandrews/trovato/v{}/templates/search.html",
        version()
    );
    let mut response = ureq::get(&url)
        .call()
        .unwrap_or_else(|e| panic!("{url} is reachable: {e}"));
    response
        .body_mut()
        .read_to_string()
        .unwrap_or_else(|e| panic!("{url} reads as text: {e}"))
}

/// The lines from the first one that starts with `start` through the first
/// one after it that starts with `end`, each trimmed, joined with newlines.
fn element(template: &str, start: &str, end: &str) -> Option<String> {
    let lines: Vec<&str> = template.lines().map(str::trim).collect();
    let first = lines.iter().position(|l| l.starts_with(start))?;
    let last = first + lines[first..].iter().position(|l| l.starts_with(end))?;
    Some(lines[first..=last].join("\n"))
}

/// The three elements, as (what it is, where it starts, where it ends).
const ELEMENTS: &[(&str, &str, &str)] = &[
    (
        "the search stylesheet link",
        "<link rel=\"stylesheet\" href=\"/static/css/scolta.css\"",
        "<link rel=\"stylesheet\" href=\"/static/css/scolta.css\"",
    ),
    (
        "the scolta-config data block",
        "<script id=\"scolta-config\"",
        "</script>",
    ),
    (
        "the scolta.js script",
        "<script src=\"/static/js/scolta.js\"",
        "<script src=\"/static/js/scolta.js\"",
    ),
];

#[test]
fn the_site_search_template_carries_the_kernels_in_browser_search() {
    let kernel = kernel_template();
    let site = read("templates/search.html");
    let mut failures = Vec::new();
    for (what, start, end) in ELEMENTS {
        let expected = element(&kernel, start, end).unwrap_or_else(|| {
            panic!(
                "the kernel's templates/search.html at v{} has {what}",
                version()
            )
        });
        match element(&site, start, end) {
            None => failures.push(format!("templates/search.html has no {what}")),
            Some(actual) if actual != expected => failures.push(format!(
                "{what} differs from the kernel's.\n  kernel: {expected}\n  site:   {actual}"
            )),
            Some(_) => {}
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_configuration_and_scolta_js_come_before_search_init_js() {
    let site = read("templates/search.html");
    let at = |needle: &str| {
        site.find(needle)
            .unwrap_or_else(|| panic!("templates/search.html contains {needle}"))
    };
    let config = at("<script id=\"scolta-config\"");
    let script = at("<script src=\"/static/js/scolta.js\"");
    let init = at("<script src=\"/static/js/search-init.js\"");
    assert!(
        config < script && script < init,
        "the order is the configuration, then scolta.js, then search-init.js"
    );
}

#[test]
fn the_site_loads_the_kernels_search_files_and_ships_no_copy() {
    for file in [
        "static/js/scolta.js",
        "static/js/search-init.js",
        "static/css/scolta.css",
    ] {
        assert!(
            !root().join(file).exists(),
            "{file} is the kernel's; a copy in the site's static/ would shadow it"
        );
    }
}

#[test]
fn every_ai_stage_and_analytics_is_off() {
    let config: serde_yml::Value =
        serde_yml::from_str(&read("config/variable.search.yml")).expect("the file is YAML");
    assert_eq!(config["key"].as_str(), Some("search"));
    let value = &config["value"];
    for switch in [
        "expand",
        "summarize",
        "followup",
        "analytics_enabled",
        "sentiment_enabled",
    ] {
        assert_eq!(
            value[switch].as_bool(),
            Some(false),
            "config/variable.search.yml sets {switch} to false"
        );
    }
}
