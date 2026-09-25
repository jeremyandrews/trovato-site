//! The pinned Trovato release is declared in eleven places. This is the test that
//! keeps every copy equal to the one that is authored.
//!
//! `kernel-release.toml` is the authored one. Everything else is written from it
//! by `scripts/sync-kernel-release.sh`: the `rev` on the SDK dependency, which
//! cargo insists on as a literal; the plugin's `api_version`, which the kernel
//! refuses at load if it exceeds its own; the image tag in the environment files
//! and in the `${TROVATO_VERSION:-…}` fallbacks that make the documented commands
//! work without an exported variable; the tag the docs importer mirrors from; and
//! the generated blocks in `README.md` and `static/brand/PROVENANCE.md`.
//!
//! A disagreement here is not cosmetic. The site would build its plugin against
//! one kernel and run it on another, and the failure surfaces as a plugin refused
//! at load, long after the edit that caused it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()))
}

/// A bare `key = "value"` from the contract file.
fn field(key: &str) -> String {
    let contract = read("kernel-release.toml");
    let prefix = format!("{key} = \"");
    contract
        .lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix(&prefix)?.strip_suffix('"'))
        .unwrap_or_else(|| panic!("kernel-release.toml declares {key}"))
        .to_string()
}

/// The version as authored: the single fact this whole test is about.
fn version() -> String {
    field("version")
}

/// The commit the release tag names.
fn rev() -> String {
    field("rev")
}

/// The plugin API version, which is the release's major and minor.
fn api_version() -> String {
    let v = version();
    let (major_minor, _patch) = v
        .rsplit_once('.')
        .unwrap_or_else(|| panic!("{v} is a three part version"));
    major_minor.to_string()
}

/// Every place that must agree, as (file, the string it has to contain).
fn expectations() -> Vec<(&'static str, String)> {
    let v = version();
    vec![
        ("Cargo.toml", format!("rev = \"{}\"", rev())),
        (
            "plugins/trovato_site/trovato_site.info.toml",
            format!("api_version = \"{}\"", api_version()),
        ),
        (".env.example", format!("TROVATO_VERSION={v}")),
        (".env.production.example", format!("TROVATO_VERSION={v}")),
        ("docker-compose.yml", format!("${{TROVATO_VERSION:-{v}}}")),
        (
            "scripts/check-production.sh",
            format!("${{TROVATO_VERSION:-{v}}}"),
        ),
        (
            "tools/src/docs_import.rs",
            format!("const TAG: &str = \"v{v}\";"),
        ),
    ]
}

#[test]
fn every_copy_of_the_pin_matches_the_one_that_is_authored() {
    let mut failures = Vec::new();
    for (file, expected) in expectations() {
        if !read(file).contains(&expected) {
            failures.push(format!(
                "{file} does not contain `{expected}`; run scripts/sync-kernel-release.sh"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "the pinned release disagrees with kernel-release.toml:\n{}",
        failures.join("\n")
    );
}

/// The contents of the `<!-- kernel-release:begin -->` block in a file.
fn generated_block(relative: &str) -> String {
    let text = read(relative);
    let (_, rest) = text
        .split_once("<!-- kernel-release:begin -->")
        .unwrap_or_else(|| panic!("{relative} carries a kernel-release block"));
    let (block, _) = rest
        .split_once("<!-- kernel-release:end -->")
        .unwrap_or_else(|| panic!("{relative}'s kernel-release block is closed"));
    block.to_string()
}

#[test]
fn the_generated_documentation_blocks_name_the_pinned_release() {
    let v = version();
    for file in ["README.md", "static/brand/PROVENANCE.md"] {
        let block = generated_block(file);
        assert!(
            block.contains(&v),
            "the generated block in {file} does not name {v}; \
             run scripts/sync-kernel-release.sh"
        );
    }
    // The README block carries the derived values too, so that a reader who never
    // opens kernel-release.toml still sees the api_version the plugin declares.
    let readme = generated_block("README.md");
    for expected in [
        format!("v{v}"),
        format!("api_version = \"{}\"", api_version()),
        format!("ghcr.io/jeremyandrews/trovato:{v}"),
    ] {
        assert!(
            readme.contains(&expected),
            "the generated block in README.md does not name `{expected}`; \
             run scripts/sync-kernel-release.sh"
        );
    }
}

/// The rev is a forty character hex commit, not a tag or a branch. A branch name
/// here would build a different kernel on different days and the lockfile would
/// be the only record of which.
#[test]
fn the_pinned_rev_is_a_commit() {
    let rev = rev();
    assert_eq!(rev.len(), 40, "rev {rev} is not a full commit sha");
    assert!(
        rev.chars().all(|c| c.is_ascii_hexdigit()),
        "rev {rev} is not hexadecimal"
    );
}
