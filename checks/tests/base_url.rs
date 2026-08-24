//! The site's base URL is declared twice. This is the test that keeps the two
//! copies the same.
//!
//! The kernel reads `SITE_URL` from the environment; a plugin cannot read the
//! environment, and reads the `site_base_url` site variable instead. Both feed
//! absolute URLs — canonicals, Open Graph, feed self-links from one; the sitemap
//! from the other — so a site whose two copies disagree emits two different
//! answers to the same question and nothing anywhere complains.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()))
}

/// The default in `${SITE_URL:-…}` in the compose file.
fn compose_default() -> String {
    let compose = read("docker-compose.yml");
    let line = compose
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("SITE_URL:"))
        .expect("docker-compose.yml sets SITE_URL");
    let start = line
        .find("${SITE_URL:-")
        .expect("SITE_URL carries a default")
        + "${SITE_URL:-".len();
    let rest = &line[start..];
    rest[..rest.find('}').expect("the default is closed")].to_string()
}

/// The `value:` in the site_base_url config file.
///
/// The file is named for the namespaced key, because `variables_get` prefixes
/// every lookup with the calling plugin's name.
fn config_value() -> String {
    let config = read("config/variable.plugin.trovato_site.site_base_url.yml");
    config
        .lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("value:"))
        .expect("the file sets a value")
        .trim()
        .to_string()
}

#[test]
fn the_two_copies_of_the_base_url_agree() {
    assert_eq!(
        compose_default(),
        config_value(),
        "SITE_URL in docker-compose.yml and site_base_url in config/ have drifted apart"
    );
}

#[test]
fn the_base_url_is_the_domain_this_site_is_for() {
    assert_eq!(config_value(), "https://trovato.rs");
}

#[test]
fn the_base_url_has_no_trailing_slash() {
    // The plugin trims one, and the kernel's resolve_url handles one, but a
    // value that needs handling on both sides is a value waiting to produce
    // `https://trovato.rs//news` from whichever side forgets.
    assert!(!config_value().ends_with('/'));
}

#[test]
fn the_env_example_documents_the_same_value() {
    let example = read(".env.example");
    assert!(
        example.contains(&format!("SITE_URL={}", config_value())),
        ".env.example does not document the base URL the site actually uses"
    );
}
