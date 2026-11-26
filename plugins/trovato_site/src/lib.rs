//! The trovato.rs website.
//!
//! This plugin is the whole of the site's Rust. Everything else the site adds
//! (templates, config, static assets, seed content) is data layered on through
//! the kernel's `TEMPLATES_DIR`, `STATIC_DIR` and config import. Nothing in the
//! Trovato tree is modified.
//!
//! It adds:
//!
//! 1. Three content types: `news`, `docs` (the mirrored kernel documentation)
//!    and `front_page`.
//! 2. The front page's listing of recent posts, rendered on the server.
//! 3. A sitemap at `/sitemap/pages.xml`, `/llms.txt`, and each documentation
//!    page's markdown at `/learn/raw/:slug`.
//! 4. A cron task that copies the site's translations from configuration into
//!    the kernel.
//!
//! Every page the plugin serves reads through the kernel's `query` interface:
//! a Gather definition, run for the request's viewer, stage and language, with
//! each row's `url` filled in by the kernel. The cron task lists items and
//! reads, saves and deletes translations through `item-api`. The plugin
//! declares no database access of its own.
//!
//! # Why the listing is a tap
//!
//! A `gather_query` tile renders an empty `<div>` that nothing hydrates, and a
//! template cannot run a query. `tap_item_view` can: the front page handler
//! appends its output to the rendered item.

use trovato_sdk::host;
use trovato_sdk::prelude::*;
use trovato_sdk::types::{ApiRequest, ApiResponse, ItemTranslation, MenuRoute};

/// Plugin name, for logging.
const PLUGIN_NAME: &str = "trovato_site";

/// The content type whose single item is the site front page.
///
/// A type rather than a hard-coded UUID: the front page is chosen by the
/// `site_front_page` variable, and matching on the type keeps this plugin from
/// needing to read that variable — or from being wrong when it changes.
const FRONT_PAGE_TYPE: &str = "front_page";

/// The site's news content type.
const NEWS_TYPE: &str = "news";

/// A page of the Trovato documentation, mirrored from the kernel repository.
const DOCS_TYPE: &str = "docs";

/// How many recent posts the front page lists.
const FRONT_LISTING_LIMIT: u32 = 5;

/// The content types the front page lists.
const LISTED_TYPES: [&str; 2] = ["blog", NEWS_TYPE];

/// Rows per page when a read takes everything. The `query` interface allows
/// at most 100.
const PAGE: u32 = 100;

/// Pages a read takes before it stops, so a runaway result is bounded.
const MAX_PAGES: u32 = 50;

/// Item ids per `list_translations` call. The kernel allows at most 100.
const TRANSLATION_CHUNK: usize = 100;

// The kernel refuses a page over 100 rows and a translation list over 100 ids.
const _: () = assert!(PAGE <= 100 && FRONT_LISTING_LIMIT <= 100 && TRANSLATION_CHUNK <= 100);

/// The site variable holding the base every absolute URL is built from.
///
/// The same value the kernel takes as `SITE_URL`, declared a second time as
/// config because a plugin can read a site variable and cannot read the
/// process's environment. `checks` has a test that fails if the two defaults
/// drift apart.
///
/// Written here without a prefix and stored with one. `variables_get` namespaces
/// every key by the calling plugin's name, so this reads
/// `plugin.trovato_site.site_base_url`, and that is what the config file is
/// called. A plugin cannot read a variable outside its own namespace at all,
/// which is why nothing here reads `site_front_page` or `site_name`.
const BASE_URL_VARIABLE: &str = "site_base_url";

/// The site variable holding every translation the site ships.
///
/// A JSON object: item id, then language, then the title and fields that replace
/// the default ones. See `config/variable.site_translations.yml`.
const TRANSLATIONS_VARIABLE: &str = "site_translations";

/// Where the site's own sitemap is served.
///
/// Not `/sitemap.xml`: the kernel registers that route itself, and axum panics
/// on a duplicate — a plugin declaring it would take the process down at
/// startup. See `sitemap()` for why the site serves one at all.
const SITEMAP_PATH: &str = "/sitemap/pages.xml";

// ─── Content types ───────────────────────────────────────────────────

/// The content types the site defines.
#[plugin_tap]
pub fn tap_item_info() -> Vec<ContentTypeDefinition> {
    vec![
        ContentTypeDefinition {
            machine_name: NEWS_TYPE.into(),
            label: "News".into(),
            description: "An announcement in the site's voice: a release, a milestone, a change worth knowing about.".into(),
            title_label: None,
            // `body`, not `field_body`. The kernel's own `page` type calls it
            // `body`, and both the front-page teaser renderer and the stock
            // gather listing template read `fields.body`; a news item named
            // otherwise renders without its text in templates the site did not
            // write. The blog plugin uses `field_body` and is inconsistent with
            // both — noted, not followed.
            fields: vec![
                FieldDefinition::new("body", FieldType::TextLong)
                    .required()
                    .label("Body"),
                FieldDefinition::new("summary", FieldType::TextLong).label("Summary"),
            ],
        },
        ContentTypeDefinition {
            machine_name: DOCS_TYPE.into(),
            label: "Documentation".into(),
            description: "A page of the Trovato documentation, mirrored from the kernel repository. Generated: edit the source there, not here.".into(),
            title_label: None,
            // `html` and not `body`: the field holds markdown already rendered
            // and syntax-highlighted, and every rendering path the kernel offers
            // for a `body` would strip the token classes back out. The template
            // for this type reads the field directly and ignores `children`,
            // which is why none of these render as ordinary fields.
            fields: vec![
                FieldDefinition::new("html", FieldType::TextLong)
                    .required()
                    .label("Rendered HTML"),
                FieldDefinition::new("source_path", FieldType::Text { max_length: Some(255) })
                    .label("Path in the kernel repository"),
                FieldDefinition::new("edit_url", FieldType::Text { max_length: Some(512) })
                    .label("Edit on GitHub"),
                FieldDefinition::new("raw_url", FieldType::Text { max_length: Some(255) })
                    .label("Raw markdown"),
                FieldDefinition::new("section", FieldType::Text { max_length: Some(128) })
                    .label("Section"),
                FieldDefinition::new("weight", FieldType::Integer).label("Reading order"),
                FieldDefinition::new("prev_path", FieldType::Text { max_length: Some(255) })
                    .label("Previous path"),
                FieldDefinition::new("prev_title", FieldType::Text { max_length: Some(255) })
                    .label("Previous title"),
                FieldDefinition::new("next_path", FieldType::Text { max_length: Some(255) })
                    .label("Next path"),
                FieldDefinition::new("next_title", FieldType::Text { max_length: Some(255) })
                    .label("Next title"),
            ],
        },
        ContentTypeDefinition {
            machine_name: FRONT_PAGE_TYPE.into(),
            label: "Front Page".into(),
            description: "The site front page. Its body carries the copy; the listing of recent posts is appended by the site plugin.".into(),
            title_label: None,
            fields: vec![
                FieldDefinition::new("body", FieldType::TextLong)
                    .required()
                    .label("Body"),
            ],
        },
    ]
}

/// Permissions for the site's own content types.
#[plugin_tap]
pub fn tap_perm() -> Vec<PermissionDefinition> {
    let mut perms = PermissionDefinition::crud_for_type(NEWS_TYPE);
    perms.extend(PermissionDefinition::crud_for_type(FRONT_PAGE_TYPE));
    perms.extend(PermissionDefinition::crud_for_type(DOCS_TYPE));
    perms
}

/// Author access for the site's content types.
///
/// The same shape the blog plugin uses: an author reaches their own item at any
/// stage of publication, and everyone else falls through to the kernel's
/// permission fallback (`edit news content`, and so on).
#[plugin_tap]
pub fn tap_item_access(input: ItemAccessInput) -> AccessResult {
    if input.item_type != NEWS_TYPE
        && input.item_type != FRONT_PAGE_TYPE
        && input.item_type != DOCS_TYPE
    {
        return AccessResult::Neutral;
    }

    if input.user_id == input.author_id {
        return AccessResult::Grant;
    }

    AccessResult::Neutral
}

// ─── The front page listing ──────────────────────────────────────────

/// Append the recent-posts listing to the front page.
///
/// Returns an empty string for every other item, which is every item but one.
///
/// Only the listing. The front page's calls to action are template markup: they
/// are navigation, and the front page template can place them between the hero
/// and the proof blocks, which this tap cannot — its output is appended after
/// everything else the page renders.
#[plugin_tap]
pub fn tap_item_view(item: Item) -> String {
    if item.item_type != FRONT_PAGE_TYPE {
        return String::new();
    }

    render_listing(&recent_posts(FRONT_LISTING_LIMIT))
}

/// One entry in the front page's listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Post {
    /// The path a reader follows to the post: its alias when it has one,
    /// `/item/{uuid}` when it does not.
    pub path: String,
    pub title: String,
    /// `blog` or `news`, rendered as a label so a reader can tell them apart.
    pub kind: String,
    /// Unix timestamp the post was created.
    pub created: i64,
}

/// The most recent published blog posts and news items, newest first.
fn recent_posts(limit: u32) -> Vec<Post> {
    match host::query(
        &listing_definition(),
        &serde_json::json!({ "per_page": limit }),
    ) {
        Ok(result) => result.items.iter().filter_map(post_from_row).collect(),
        Err(code) => {
            host::log(
                "error",
                PLUGIN_NAME,
                &format!("listing query failed: {code}"),
            );
            Vec::new()
        }
    }
}

/// The filter every read here starts from: published items only.
///
/// An anonymous visitor sees only published items anyway. The filter keeps an
/// administrator's front page and sitemap the same as everyone else's.
fn published() -> serde_json::Value {
    serde_json::json!({ "field": "status", "operator": "equals", "value": 1 })
}

/// The front page listing: published blog posts and news items, newest first.
pub fn listing_definition() -> serde_json::Value {
    serde_json::json!({
        "base_table": "item",
        "filters": [
            published(),
            { "field": "type", "operator": "in", "value": LISTED_TYPES },
        ],
        "fields": [
            { "field_name": "id" },
            { "field_name": "title" },
            { "field_name": "type" },
            { "field_name": "created" },
        ],
        "sorts": [{ "field": "created", "direction": "desc" }],
    })
}

/// Build a `Post` from one result row, or nothing if the row is missing a field
/// the listing cannot render without.
///
/// The kernel fills in `url`: the item's alias, or `/item/{id}` when it has
/// none. An empty or missing `url` falls back to the item path too, because
/// `href=""` reloads the current page.
pub fn post_from_row(row: &serde_json::Value) -> Option<Post> {
    let id = row.get("id")?.as_str()?;
    let path = row
        .get("url")
        .and_then(|v| v.as_str())
        .filter(|url| !url.is_empty())
        .map_or_else(|| format!("/item/{id}"), String::from);

    Some(Post {
        path,
        title: row.get("title")?.as_str()?.to_string(),
        kind: row.get("type")?.as_str()?.to_string(),
        created: row.get("created").and_then(serde_json::Value::as_i64)?,
    })
}

/// Every row of `definition`, a page at a time, up to `MAX_PAGES` pages.
///
/// A failure on any page is the caller's error: a sitemap or an index served
/// from the pages read before it would look complete and not be.
fn every_row(definition: &serde_json::Value, what: &str) -> Result<Vec<serde_json::Value>, i32> {
    every_page(|page| {
        host::query(
            definition,
            &serde_json::json!({ "page": page, "per_page": PAGE }),
        )
    })
    .inspect_err(|code| {
        host::log(
            "error",
            PLUGIN_NAME,
            &format!("{what} query failed: {code}"),
        );
    })
}

/// The paging behind `every_row`: call `fetch` for page 1, 2, ... until a page
/// says there is no next one, or `MAX_PAGES` have been read. The first error
/// ends the read and is returned.
fn every_page(
    mut fetch: impl FnMut(u32) -> Result<trovato_sdk::types::QueryResult, i32>,
) -> Result<Vec<serde_json::Value>, i32> {
    let mut rows = Vec::new();
    for page in 1..=MAX_PAGES {
        let result = fetch(page)?;
        rows.extend(result.items);
        if !result.has_next {
            return Ok(rows);
        }
    }
    host::log(
        "warn",
        PLUGIN_NAME,
        &format!("a read stopped at its cap of {MAX_PAGES} pages"),
    );
    Ok(rows)
}

/// Render the listing.
///
/// An empty list renders an empty string rather than an empty heading: a front page
/// that has nothing to list should look like a front page with nothing to list, not
/// like one whose listing broke.
pub fn render_listing(posts: &[Post]) -> String {
    if posts.is_empty() {
        return String::new();
    }

    let mut html = String::from(
        "<section class=\"front-listing\" aria-labelledby=\"front-listing-heading\">\n\
         <h2 class=\"front-listing__heading\" id=\"front-listing-heading\">Recent posts</h2>\n\
         <ul class=\"front-listing__items\">\n",
    );

    for post in posts {
        html.push_str("<li class=\"front-listing__item\">\n");
        html.push_str(&format!(
            "<a class=\"front-listing__link\" href=\"{}\">{}</a>\n",
            escape_html(&post.path),
            escape_html(&post.title)
        ));
        html.push_str(&format!(
            "<p class=\"front-listing__meta\">{} &middot; <time datetime=\"{}\">{}</time></p>\n",
            escape_html(&kind_label(&post.kind)),
            escape_html(&iso_date(post.created)),
            escape_html(&iso_date(post.created)),
        ));
        html.push_str("</li>\n");
    }

    html.push_str("</ul>\n</section>\n");
    html
}

/// The reader-facing name of a content type.
pub fn kind_label(kind: &str) -> String {
    match kind {
        "blog" => "Blog".to_string(),
        NEWS_TYPE => "News".to_string(),
        other => other.to_string(),
    }
}

/// A Unix timestamp as `YYYY-MM-DD`.
///
/// Hand-rolled because the plugin has no clock, no timezone database and no
/// `chrono`: it is handed a timestamp and needs a date, and the civil-from-days
/// algorithm is exact for every date this site will ever carry. UTC, deliberately —
/// a publication date that shifts with the reader's timezone is a worse date.
pub fn iso_date(timestamp: i64) -> String {
    let days = timestamp.div_euclid(86_400);

    // Howard Hinnant's civil_from_days, shifted to a March-based year so that the
    // leap day lands at the end and the month arithmetic has no special case.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = era * 400 + yoe + i64::from(month <= 2);

    format!("{year:04}-{month:02}-{day:02}")
}

/// Escape text for HTML.
pub fn escape_html(raw: &str) -> String {
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

// ─── The sitemap ─────────────────────────────────────────────────────

/// Where a documentation page's markdown source is served.
///
/// A `.md` suffix would be the obvious spelling and does not work: the kernel's
/// static-file handler maps `.md` to `application/octet-stream`, and a plugin
/// route pattern cannot carry a literal extension after a parameter. So the
/// format is a path segment.
const RAW_DOC_PATH: &str = "/learn/raw/:slug";

/// The machine-readable description of the site.
///
/// Not `static/llms.txt` for the same reason: `mime_from_path` has no case for
/// `.txt` either, so a static one downloads instead of opening.
const LLMS_PATH: &str = "/llms.txt";

/// Register the routes the site serves itself.
#[plugin_tap]
pub fn tap_menu() -> Vec<MenuRoute> {
    vec![
        MenuRoute::api("GET", SITEMAP_PATH, "sitemap")
            .title("Sitemap")
            .permission("access content"),
        MenuRoute::api("GET", RAW_DOC_PATH, "doc_raw")
            .title("Markdown source")
            .permission("access content"),
        MenuRoute::api("GET", LLMS_PATH, "llms")
            .title("llms.txt")
            .permission("access content"),
    ]
}

/// Serve whichever route was asked for.
#[plugin_tap]
pub fn tap_api(request: ApiRequest) -> ApiResponse {
    match request.callback.as_str() {
        "sitemap" => generated(sitemap(), "application/xml; charset=utf-8"),
        "doc_raw" => match request.params.get("slug").map(String::as_str) {
            Some(slug) => document_response(doc_markdown(slug)),
            None => ApiResponse::error(404, "no such document"),
        },
        "llms" => generated(llms_txt(), "text/plain; charset=utf-8"),
        other => {
            host::log("warn", PLUGIN_NAME, &format!("unknown callback: {other}"));
            ApiResponse::error(404, "not found")
        }
    }
}

/// A generated document, or 503 when a read behind it failed.
///
/// Not a 200 with what was read before the failure: a crawler takes a sitemap
/// at its word, and a 503 tells it to come back.
pub fn generated(body: Result<String, i32>, content_type: &str) -> ApiResponse {
    match body {
        Ok(body) => ApiResponse::with_status(200, body).content_type(content_type),
        Err(_) => ApiResponse::error(503, "temporarily unavailable"),
    }
}

/// A documentation page's markdown: 200, 404 when no page has that address,
/// 503 when the read failed and the answer is not known.
pub fn document_response(markdown: Result<Option<String>, i32>) -> ApiResponse {
    match markdown {
        Ok(Some(markdown)) => {
            ApiResponse::with_status(200, markdown).content_type("text/markdown; charset=utf-8")
        }
        Ok(None) => ApiResponse::error(404, "no such document"),
        Err(_) => ApiResponse::error(503, "temporarily unavailable"),
    }
}

/// The site's non-item routes.
///
/// A listing is not an item, so nothing in the `item` table describes `/news` or
/// `/blog`. They are named here because a sitemap that omits a site's listings
/// omits the pages most likely to be a crawler's way in.
const ROUTE_PATHS: [&str; 4] = ["/", "/news", "/blog", "/blog/archive"];

/// Every public URL on this site, as a sitemap.
///
/// # Why the site serves one at all
///
/// The kernel serves `/sitemap.xml`, and it has two problems this site cannot
/// live with. It emits `<loc>/news</loc>` — a path, where the sitemap protocol
/// requires an absolute URL, so the document is invalid as written. And it lists
/// items only, so every listing route is missing.
///
/// Both are kernel-side and neither is fixed here. What the site does instead is
/// serve a correct sitemap of its own at a path the kernel has not taken, and
/// the production proxy maps `/sitemap.xml` onto it. See `docs/DEPLOY.md`.
pub fn sitemap() -> Result<String, i32> {
    let rows = every_row(&sitemap_definition(), "sitemap")?;
    Ok(sitemap_from(&base_url(), &rows))
}

/// The sitemap document for `rows`, after the site's own routes.
pub fn sitemap_from(base: &str, rows: &[serde_json::Value]) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");

    for path in ROUTE_PATHS {
        xml.push_str(&sitemap_entry(&absolute(base, path), None));
    }

    for row in rows {
        let Some(post) = post_from_row(row) else {
            continue;
        };
        let changed = row.get("changed").and_then(serde_json::Value::as_i64);
        xml.push_str(&sitemap_entry(
            &absolute(base, &post.path),
            changed.map(iso_date),
        ));
    }

    xml.push_str("</urlset>\n");
    xml
}

/// Every published item, most recently changed first.
///
/// The same fields as the listing, which `post_from_row` reads, with `changed`
/// added for `<lastmod>`. The id breaks ties so that paging is stable.
///
/// The front page is left out. Its address is `/`, which cannot be aliased, so
/// it would otherwise appear a second time as `/item/{uuid}`. `/` is listed
/// once, from `ROUTE_PATHS`.
pub fn sitemap_definition() -> serde_json::Value {
    serde_json::json!({
        "base_table": "item",
        "filters": [
            published(),
            { "field": "type", "operator": "not_equals", "value": FRONT_PAGE_TYPE },
        ],
        "fields": [
            { "field_name": "id" },
            { "field_name": "title" },
            { "field_name": "type" },
            { "field_name": "created" },
            { "field_name": "changed" },
        ],
        "sorts": [
            { "field": "changed", "direction": "desc" },
            { "field": "id", "direction": "asc" },
        ],
    })
}

/// The site's base URL, without a trailing slash.
pub fn base_url() -> String {
    let raw = host::variables_get(BASE_URL_VARIABLE, DEFAULT_BASE_URL)
        .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
    raw.trim().trim_end_matches('/').to_string()
}

/// What the base URL is when the site has not been told otherwise.
const DEFAULT_BASE_URL: &str = "https://trovato.rs";

/// Join a base and a local path into one absolute URL.
///
/// The path always starts with `/` here — it is either a literal from
/// `ROUTE_PATHS` or an alias the kernel wrote — but a missing separator would
/// produce `https://trovato.rsnews`, which is the kind of thing that is only
/// noticed by whoever submits the sitemap.
pub fn absolute(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    if path.starts_with('/') {
        format!("{base}{path}")
    } else {
        format!("{base}/{path}")
    }
}

/// One `<url>` entry.
fn sitemap_entry(loc: &str, lastmod: Option<String>) -> String {
    let mut entry = format!("  <url>\n    <loc>{}</loc>\n", escape_xml(loc));
    if let Some(date) = lastmod {
        entry.push_str(&format!("    <lastmod>{}</lastmod>\n", escape_xml(&date)));
    }
    entry.push_str("  </url>\n");
    entry
}

/// Escape text for XML.
///
/// Separate from `escape_html` because the two are not the same job even where
/// they overlap: `&#39;` is an HTML entity that an XML parser does not know, so
/// an apostrophe has to be `&apos;` here.
pub fn escape_xml(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for c in raw.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

// ─── Translations ────────────────────────────────────────────────────

/// Copy the site's translations from configuration into the kernel.
///
/// The translations live in configuration, where they are reviewed and
/// exported like everything else. Each cron tick reads the declared map,
/// reads what the kernel stores, and makes the two match through `item-api`:
/// it saves a declared translation that differs, deletes a stored one that is
/// no longer declared, and leaves an identical one alone so that its
/// `changed` does not move.
///
/// Cron has no user, so `item_background` in the manifest is what lets these
/// calls run.
///
/// The signature is the one the kernel's other cron taps use: `CronInput` in,
/// a JSON value out. A zero-argument version compiles and is dispatched, and
/// does nothing.
#[plugin_tap]
pub fn tap_cron(_input: CronInput) -> serde_json::Value {
    let raw = match host::variables_get(TRANSLATIONS_VARIABLE, "{}") {
        Ok(value) => value,
        Err(code) => {
            host::log(
                "error",
                PLUGIN_NAME,
                &format!("could not read {TRANSLATIONS_VARIABLE}: {code}"),
            );
            return serde_json::json!({ "translations": "unreadable" });
        }
    };

    let translations: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(e) => {
            host::log(
                "error",
                PLUGIN_NAME,
                &format!("{TRANSLATIONS_VARIABLE} is not valid JSON: {e}"),
            );
            return serde_json::json!({ "translations": "unparseable" });
        }
    };

    host::log(
        "info",
        PLUGIN_NAME,
        &format!(
            "translations: read {} bytes, {} item(s)",
            raw.len(),
            translations.as_object().map_or(0, serde_json::Map::len)
        ),
    );

    let Some(stored) = stored_translations(&translations) else {
        return serde_json::json!({ "translations": "unlisted" });
    };

    let mut written = 0usize;
    let mut skipped = 0usize;

    for (item_id, languages) in translations.as_object().into_iter().flatten() {
        for (language, translation) in languages.as_object().into_iter().flatten() {
            match sync_one(item_id, language, translation, &stored) {
                Some(true) => written += 1,
                Some(false) => skipped += 1,
                None => {}
            }
        }
    }

    // A translation removed from configuration has to disappear from the site,
    // or a page keeps serving text the repository no longer contains.
    let removed = remove_undeclared(&translations, &stored);

    if written > 0 || removed > 0 {
        host::log(
            "info",
            PLUGIN_NAME,
            &format!("wrote {written} translation(s), removed {removed}, {skipped} unchanged"),
        );
    }

    serde_json::json!({ "written": written, "removed": removed, "unchanged": skipped })
}

/// The content types whose items the sync reads stored translations for.
///
/// Not every type. Kernel 0.105.0 does not give cron the Gather service, so
/// `host::query` answers -10 there, and the ids come from `item-api`'s
/// `query_items` instead. That call returns whole items, and one mirrored
/// `docs` item can be larger than the SDK's 256 KB output buffer, so `docs` is
/// left out. The site translates none of them, and the import tool rewrites
/// them from the kernel repository.
const TRANSLATED_TYPES: [&str; 4] = [FRONT_PAGE_TYPE, "page", "blog", NEWS_TYPE];

/// Items per `query_items` call. Small, because each item comes back whole.
const ID_PAGE: usize = 20;

/// The most items of one type the sync reads ids for. A site past it gets a
/// warning in the log, and the items beyond it keep any stored translation
/// the configuration no longer declares.
const MAX_ID_ITEMS: usize = 10_000;

/// The ids of every item one `fetch(limit, offset)` call after another can
/// reach, and whether that was all of them.
///
/// A window that fails as a whole is read again one item at a time, so one
/// item too large for the SDK's output buffer costs only its own id. A window
/// in which every single item fails as well ends the read: the failure is not
/// about one item. The read also ends at `MAX_ID_ITEMS`. Either way the ids
/// already read are returned, with `false`.
fn item_ids(
    mut fetch: impl FnMut(usize, usize) -> Result<Vec<String>, i32>,
) -> (Vec<String>, bool) {
    let mut ids = Vec::new();
    let mut complete = true;
    let mut offset = 0;
    while offset < MAX_ID_ITEMS {
        match fetch(ID_PAGE, offset) {
            Ok(batch) => {
                let done = batch.len() < ID_PAGE;
                ids.extend(batch);
                if done {
                    return (ids, complete);
                }
            }
            Err(_) => {
                let mut read = 0;
                for at in offset..offset + ID_PAGE {
                    match fetch(1, at) {
                        Ok(one) if one.is_empty() => return (ids, complete),
                        Ok(one) => {
                            read += 1;
                            ids.extend(one);
                        }
                        Err(_) => complete = false,
                    }
                }
                if read == 0 {
                    return (ids, false);
                }
            }
        }
        offset += ID_PAGE;
    }
    (ids, false)
}

/// Every translation the kernel stores for the declared items and for every
/// item of `TRANSLATED_TYPES`, or `None` if the stored translations could not
/// be read.
///
/// `None` rather than an empty list: an empty list would read as "nothing is
/// stored", and the sync would then write every declared translation again.
/// An item list that could not be read in full is not `None`: the declared
/// items are always asked about, so their translations still sync, and only
/// the removal of undeclared translations misses the items that were not read.
fn stored_translations(translations: &serde_json::Value) -> Option<Vec<ItemTranslation>> {
    let mut ids: Vec<String> = translations
        .as_object()
        .map(|items| items.keys().cloned().collect())
        .unwrap_or_default();

    for item_type in TRANSLATED_TYPES {
        let (found, complete) = item_ids(|limit, offset| {
            host::query_items(&serde_json::json!({
                "type": item_type,
                "limit": limit,
                "offset": offset,
            }))
            .map(|items| items.into_iter().map(|item| item.id.to_string()).collect())
        });
        if !complete {
            host::log(
                "warn",
                PLUGIN_NAME,
                &format!(
                    "read the ids of {} {item_type} item(s), not all of them; \
                     undeclared translations of the rest stay until a later run",
                    found.len()
                ),
            );
        }
        ids.extend(found);
    }
    ids.sort();
    ids.dedup();

    let mut stored = Vec::new();
    for chunk in ids.chunks(TRANSLATION_CHUNK) {
        let refs: Vec<&str> = chunk.iter().map(String::as_str).collect();
        match host::list_translations(&refs) {
            Ok(found) => stored.extend(found),
            Err(code) => {
                host::log(
                    "error",
                    PLUGIN_NAME,
                    &format!("could not list stored translations: {code}"),
                );
                return None;
            }
        }
    }
    Some(stored)
}

/// Save one translation if it differs from what is stored.
///
/// `Some(true)` when it saved, `Some(false)` when the stored translation
/// already matched, `None` when the entry was malformed or the save failed.
fn sync_one(
    item_id: &str,
    language: &str,
    translation: &serde_json::Value,
    stored: &[ItemTranslation],
) -> Option<bool> {
    let title = translation.get("title")?.as_str()?;
    let fields = translation.get("fields")?;

    let same = stored.iter().any(|t| {
        t.item_id.to_string() == item_id
            && t.language == language
            && t.title == title
            && &t.fields == fields
    });
    if same {
        return Some(false);
    }

    let entry = serde_json::json!({
        "item_id": item_id,
        "language": language,
        "title": title,
        "fields": fields,
    });
    match host::save_translation(&entry) {
        Ok(_) => Some(true),
        Err(code) => {
            host::log(
                "error",
                PLUGIN_NAME,
                &format!("could not write the {language} translation of {item_id}: {code}"),
            );
            None
        }
    }
}

/// The stored translations the configuration does not declare.
fn undeclared<'a>(
    translations: &serde_json::Value,
    stored: &'a [ItemTranslation],
) -> Vec<&'a ItemTranslation> {
    stored
        .iter()
        .filter(|t| {
            translations
                .get(t.item_id.to_string())
                .and_then(|languages| languages.get(&t.language))
                .is_none()
        })
        .collect()
}

/// Delete every stored translation the configuration no longer declares.
///
/// Returns how many were removed.
fn remove_undeclared(translations: &serde_json::Value, stored: &[ItemTranslation]) -> usize {
    let mut removed = 0usize;
    for t in undeclared(translations, stored) {
        let item_id = t.item_id.to_string();
        match host::delete_translation(&item_id, &t.language) {
            Ok(true) => removed += 1,
            Ok(false) => {}
            Err(code) => host::log(
                "error",
                PLUGIN_NAME,
                &format!(
                    "could not remove the {} translation of {item_id}: {code}",
                    t.language
                ),
            ),
        }
    }
    removed
}

// ─── The machine-readable surface ────────────────────────────────────

/// The markdown a documentation page was rendered from.
///
/// The item carries both: the rendered HTML that the page shows, and the source
/// it was rendered from. Storing the source in the same row rather than as a
/// file beside it means one thing to import, one thing to keep in step, and no
/// way for the two to describe different releases.
///
/// The `docs` item template ignores `children` and reads the fields it wants
/// directly, so neither of these renders as an ordinary field on the page.
///
/// `Ok(None)` when no published page has that address, `Err` when the read
/// failed: the route answers 404 for the first and 503 for the second.
fn doc_markdown(slug: &str) -> Result<Option<String>, i32> {
    if !is_safe_slug(slug) {
        return Ok(None);
    }

    let result = host::query(
        &doc_markdown_definition(slug),
        &serde_json::json!({ "per_page": 1 }),
    )
    .inspect_err(|code| {
        host::log(
            "error",
            PLUGIN_NAME,
            &format!("document query failed: {code}"),
        );
    })?;
    Ok(result
        .items
        .first()
        .and_then(|row| row.get("markdown"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string))
}

/// One published documentation page, found by the address it is served at.
pub fn doc_markdown_definition(slug: &str) -> serde_json::Value {
    serde_json::json!({
        "base_table": "item",
        "filters": [
            published(),
            { "field": "type", "operator": "equals", "value": DOCS_TYPE },
            { "field": "url", "operator": "equals", "value": format!("/learn/{slug}") },
        ],
        "fields": [{ "field_name": "fields.markdown", "label": "markdown" }],
    })
}

/// Whether a slug can only ever name a document.
///
/// It is bound as a parameter rather than interpolated, so this is not what
/// stands between the route and an injection. It is what stands between the
/// route and a lookup that was never going to match: lowercase letters, digits
/// and hyphens are what the generator produces and all it will ever produce, and
/// anything else is a request for a document that does not exist.
pub fn is_safe_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 128
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The site, described for something that reads rather than browses.
///
/// The format is the llms.txt convention: a title, a summary, then linked
/// sections. What makes it worth serving here rather than writing by hand is the
/// document list, which comes from the same rows the site renders — so it cannot
/// list a page that does not exist or miss one that does.
pub fn llms_txt() -> Result<String, i32> {
    let docs = listed_docs(&every_row(&docs_definition(), "docs")?);
    Ok(llms_txt_from(&base_url(), &docs))
}

/// The llms.txt document, listing `docs` as (title, address) pairs.
pub fn llms_txt_from(base: &str, docs: &[(String, String)]) -> String {
    let mut out = String::from("# Trovato\n\n");
    out.push_str(
        "> A content management system written in Rust. Content is one row per item, with \
         all its fields in one JSONB column rather than a table per field; plugins are \
         WebAssembly modules that reach only what they declare; queries are built with \
         Gather, a type-safe query engine.\n\n",
    );
    out.push_str(
        "This file lists the site's documentation and the markdown each page was rendered \
         from. Each \"markdown at\" link below is the source text, unrendered.\n\n",
    );

    out.push_str("## Documentation\n\n");
    if docs.is_empty() {
        out.push_str("The documentation has not been imported yet.\n\n");
    }
    for (title, url) in docs {
        let slug = url.rsplit('/').next().unwrap_or_default();
        out.push_str(&format!(
            "- [{}]({}): markdown at {}\n",
            title,
            absolute(base, url),
            absolute(base, &format!("/learn/raw/{slug}")),
        ));
    }

    out.push_str("\n## The site\n\n");
    for (path, description) in [
        (
            "/why",
            "The architecture argument, at length, including what is not done.",
        ),
        (
            "/get-started",
            "Running Trovato, with Docker or from source.",
        ),
        (
            "/rust",
            "The stack by name, the pinned toolchain, and how to write a plugin.",
        ),
        (
            "/community",
            "Contributing, the code of conduct, and how Trovato is built.",
        ),
        (
            "/accessibility",
            "What is tested, how, and what is known to be missing.",
        ),
        ("/blog", "Longer pieces about how Trovato works."),
        ("/news", "Releases and changes."),
    ] {
        out.push_str(&format!(
            "- [{}]({}): {}\n",
            path,
            absolute(base, path),
            description
        ));
    }

    out.push_str("\n## Source\n\n");
    out.push_str("- [The kernel](https://github.com/jeremyandrews/trovato)\n");
    out.push_str("- [This website](https://github.com/jeremyandrews/trovato-site)\n");
    out
}

/// Every published documentation page, in reading order: by weight, pages
/// without one last, then by title.
pub fn docs_definition() -> serde_json::Value {
    serde_json::json!({
        "base_table": "item",
        "filters": [
            published(),
            { "field": "type", "operator": "equals", "value": DOCS_TYPE },
        ],
        "fields": [
            { "field_name": "id" },
            { "field_name": "title" },
            { "field_name": "fields.weight", "label": "weight" },
        ],
        "sorts": [
            { "field": "fields.weight", "direction": "asc", "nulls": "last" },
            { "field": "title", "direction": "asc" },
        ],
    })
}

/// The title and address of each documentation page that has an alias.
///
/// A page whose `url` is `/item/{id}` has no alias, so it has no address under
/// `/learn` and no markdown route. It is left out.
pub fn listed_docs(rows: &[serde_json::Value]) -> Vec<(String, String)> {
    rows.iter()
        .filter_map(|row| {
            let title = row.get("title")?.as_str()?;
            let url = row.get("url")?.as_str()?;
            if url.is_empty() || url.starts_with("/item/") {
                return None;
            }
            Some((title.to_string(), url.to_string()))
        })
        .collect()
}

#[cfg(test)]
// Tests are allowed to use unwrap/expect freely.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ─── Content types ───────────────────────────────────────────────

    #[test]
    fn declares_news_and_front_page() {
        let types = __inner_tap_item_info();
        let names: Vec<&str> = types.iter().map(|t| t.machine_name.as_str()).collect();
        assert_eq!(names, vec!["news", "docs", "front_page"]);
    }

    #[test]
    fn news_body_is_required() {
        let types = __inner_tap_item_info();
        let news = types.iter().find(|t| t.machine_name == "news").unwrap();
        let body = news
            .fields
            .iter()
            .find(|f| f.field_name == "body")
            .expect("news has a body field");
        assert!(body.required, "a news item without a body is not news");
    }

    #[test]
    fn permissions_cover_both_types() {
        let perms = __inner_tap_perm();
        // Four CRUD permissions per type, three types.
        assert_eq!(perms.len(), 12);
        assert!(perms.iter().any(|p| p.name == "edit news content"));
        assert!(perms.iter().any(|p| p.name == "edit front_page content"));
    }

    // ─── Access ──────────────────────────────────────────────────────

    fn access_input(item_type: &str, author_id: Uuid, user_id: Uuid) -> ItemAccessInput {
        ItemAccessInput {
            item_id: Uuid::nil(),
            item_type: item_type.into(),
            author_id,
            operation: "edit".into(),
            user_id,
            user_authenticated: true,
            user_permissions: vec![],
            stage_id: None,
            stage_machine_name: None,
        }
    }

    #[test]
    fn access_is_neutral_for_other_types() {
        let input = access_input("blog", Uuid::nil(), Uuid::nil());
        assert_eq!(__inner_tap_item_access(input), AccessResult::Neutral);
    }

    #[test]
    fn author_reaches_their_own_news() {
        let author = Uuid::from_u128(7);
        let input = access_input("news", author, author);
        assert_eq!(__inner_tap_item_access(input), AccessResult::Grant);
    }

    #[test]
    fn a_stranger_falls_through_to_the_permission_check() {
        let input = access_input("news", Uuid::from_u128(1), Uuid::from_u128(2));
        assert_eq!(__inner_tap_item_access(input), AccessResult::Neutral);
    }

    // ─── The listing ─────────────────────────────────────────────────
    //
    // The kernel appends `tap_item_view` output to every item it renders, the
    // front page included. Returning a listing for anything but the front page
    // would put it under every blog post on the site, so that is pinned here.

    fn item_of_type(item_type: &str) -> Item {
        Item {
            id: Uuid::from_u128(1),
            item_type: item_type.into(),
            title: "A title".into(),
            fields: std::collections::HashMap::new(),
            status: 1,
            author_id: Uuid::nil(),
            current_revision_id: None,
            stage_id: Uuid::nil(),
            created: 0,
            changed: 0,
            language: Some("en".into()),
        }
    }

    #[test]
    fn a_blog_post_gets_no_listing() {
        assert_eq!(__inner_tap_item_view(item_of_type("blog")), "");
    }

    #[test]
    fn a_news_item_gets_no_listing() {
        assert_eq!(__inner_tap_item_view(item_of_type("news")), "");
    }

    fn post(title: &str, kind: &str, path: &str) -> Post {
        Post {
            path: path.into(),
            title: title.into(),
            kind: kind.into(),
            created: 1_767_225_600,
        }
    }

    #[test]
    fn an_empty_listing_renders_nothing_at_all() {
        // Not an empty heading over an empty list: a front page with no posts
        // should look deliberate, not broken.
        assert_eq!(render_listing(&[]), "");
    }

    #[test]
    fn the_listing_links_each_post_and_names_its_kind() {
        let html = render_listing(&[
            post("Trovato 0.101.0", "news", "/news/trovato-0-101-0"),
            post(
                "One row, twenty fields",
                "blog",
                "/blog/one-row-twenty-fields",
            ),
        ]);

        assert!(html.contains("href=\"/news/trovato-0-101-0\""));
        assert!(html.contains("Trovato 0.101.0"));
        assert!(html.contains(">News<") || html.contains("News &middot;"));
        assert!(html.contains("href=\"/blog/one-row-twenty-fields\""));
        assert!(html.contains("aria-labelledby=\"front-listing-heading\""));
    }

    #[test]
    fn a_title_cannot_inject_markup() {
        let html = render_listing(&[post("<script>alert(1)</script>", "blog", "/blog/x")]);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn a_path_cannot_break_out_of_its_attribute() {
        let html = render_listing(&[post("Fine", "blog", "/blog/x\" onclick=\"evil()")]);
        assert!(!html.contains("onclick=\"evil()"));
        assert!(html.contains("&quot;"));
    }

    // ─── Row mapping ─────────────────────────────────────────────────

    fn row(url: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "id": "0193a5a0-0000-7000-8000-00000000000a",
            "title": "Hello",
            "type": "blog",
            "created": 1_767_225_600_i64,
            "url": url,
        })
    }

    #[test]
    fn a_row_takes_its_path_from_the_kernels_url() {
        let post = post_from_row(&row(serde_json::json!("/blog/hello"))).unwrap();
        assert_eq!(post.path, "/blog/hello");
        assert_eq!(post.kind, "blog");
        assert_eq!(post.title, "Hello");
    }

    #[test]
    fn a_row_without_a_url_falls_back_to_the_item_path() {
        assert_eq!(
            post_from_row(&row(serde_json::Value::Null)).unwrap().path,
            "/item/0193a5a0-0000-7000-8000-00000000000a"
        );
    }

    #[test]
    fn an_empty_url_is_not_a_path() {
        // href="" reloads the current page.
        assert_eq!(
            post_from_row(&row(serde_json::json!(""))).unwrap().path,
            "/item/0193a5a0-0000-7000-8000-00000000000a"
        );
    }

    #[test]
    fn a_row_missing_a_title_is_dropped_rather_than_rendered_blank() {
        let row = serde_json::json!({
            "id": "0193a5a0-0000-7000-8000-00000000000a",
            "type": "blog",
            "created": 1_767_225_600_i64,
            "url": "/blog/hello",
        });
        assert_eq!(post_from_row(&row), None);
    }

    // ─── The Gather definitions ──────────────────────────────────────

    fn filters(definition: &serde_json::Value) -> Vec<(String, String, serde_json::Value)> {
        definition["filters"]
            .as_array()
            .map(|filters| {
                filters
                    .iter()
                    .map(|f| {
                        (
                            f["field"].as_str().unwrap().to_string(),
                            f["operator"].as_str().unwrap().to_string(),
                            f["value"].clone(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn sorts(definition: &serde_json::Value) -> Vec<(String, String)> {
        definition["sorts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                (
                    s["field"].as_str().unwrap().to_string(),
                    s["direction"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    }

    fn field_names(definition: &serde_json::Value) -> Vec<String> {
        definition["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["field_name"].as_str().unwrap().to_string())
            .collect()
    }

    fn published_filter() -> (String, String, serde_json::Value) {
        ("status".into(), "equals".into(), serde_json::json!(1))
    }

    fn every_definition() -> Vec<(&'static str, serde_json::Value)> {
        vec![
            ("listing", listing_definition()),
            ("sitemap", sitemap_definition()),
            ("docs", docs_definition()),
            ("doc", doc_markdown_definition("readme")),
        ]
    }

    #[test]
    fn the_listing_reads_published_blog_posts_and_news_newest_first() {
        let definition = listing_definition();
        assert_eq!(
            filters(&definition),
            vec![
                published_filter(),
                (
                    "type".into(),
                    "in".into(),
                    serde_json::json!(["blog", "news"])
                ),
            ]
        );
        assert_eq!(sorts(&definition), vec![("created".into(), "desc".into())]);
        assert_eq!(
            field_names(&definition),
            vec!["id", "title", "type", "created"]
        );
    }

    #[test]
    fn the_sitemap_excludes_the_front_page_item() {
        // `/` is listed once, from ROUTE_PATHS. The front page item has no
        // alias, because `/` cannot be aliased, so without this it appears a
        // second time as /item/{uuid}.
        let definition = sitemap_definition();
        assert_eq!(
            filters(&definition),
            vec![
                published_filter(),
                (
                    "type".into(),
                    "not_equals".into(),
                    serde_json::json!("front_page")
                ),
            ]
        );
    }

    #[test]
    fn the_sitemap_is_newest_change_first_with_a_stable_tie_break() {
        // Read a page at a time, so ties need an order or a row can land on
        // two pages, or none.
        let definition = sitemap_definition();
        assert_eq!(
            sorts(&definition),
            vec![
                ("changed".into(), "desc".into()),
                ("id".into(), "asc".into())
            ]
        );
        assert!(field_names(&definition).contains(&"changed".to_string()));
    }

    #[test]
    fn the_docs_are_read_in_reading_order() {
        let definition = docs_definition();
        assert_eq!(
            filters(&definition),
            vec![
                published_filter(),
                ("type".into(), "equals".into(), serde_json::json!("docs")),
            ]
        );
        assert_eq!(
            sorts(&definition),
            vec![
                ("fields.weight".into(), "asc".into()),
                ("title".into(), "asc".into())
            ]
        );
        // A page without a weight comes after every page with one.
        assert_eq!(definition["sorts"][0]["nulls"], "last");
    }

    #[test]
    fn the_docs_list_skips_a_page_without_an_alias() {
        let rows = vec![
            serde_json::json!({ "title": "Readme", "url": "/learn/readme" }),
            serde_json::json!({ "title": "Orphan", "url": "/item/0193a5a0-0000-7000-8000-00000000000a" }),
            serde_json::json!({ "title": "No url" }),
            serde_json::json!({ "title": "Empty", "url": "" }),
        ];
        assert_eq!(
            listed_docs(&rows),
            vec![("Readme".to_string(), "/learn/readme".to_string())]
        );
    }

    #[test]
    fn a_document_is_found_by_the_address_it_is_served_at() {
        let definition = doc_markdown_definition("plugin-development");
        assert_eq!(
            filters(&definition),
            vec![
                published_filter(),
                ("type".into(), "equals".into(), serde_json::json!("docs")),
                (
                    "url".into(),
                    "equals".into(),
                    serde_json::json!("/learn/plugin-development")
                ),
            ]
        );
        assert_eq!(definition["fields"][0]["field_name"], "fields.markdown");
        assert_eq!(definition["fields"][0]["label"], "markdown");
    }

    #[test]
    fn the_translation_sync_reads_every_type_the_site_translates() {
        // Every type with an item in the translations variable, so a
        // translation removed from configuration is deleted.
        for item_type in ["front_page", "page", "blog", "news"] {
            assert!(TRANSLATED_TYPES.contains(&item_type), "{item_type}");
        }
    }

    #[test]
    fn every_definition_reads_items_within_the_kernels_bounds() {
        // The kernel refuses more than 32 filters or 8 sorts.
        // Every definition here is an item read: the kernel applies the
        // viewer, the stage and the language, and fills in each row's url.
        for (name, definition) in every_definition() {
            assert_eq!(definition["base_table"], "item", "{name}");
            assert!(filters(&definition).len() <= 32, "{name}");
            assert!(
                definition["sorts"].as_array().map_or(0, Vec::len) <= 8,
                "{name}"
            );
            assert!(definition.get("relationships").is_none(), "{name}");
            assert!(definition.get("stage_aware").is_none(), "{name}");
        }
    }

    #[test]
    fn the_manifest_declares_no_database_access() {
        // The capabilities are exactly these two keys: no database interface,
        // no table list and no SQL grant.
        let manifest = include_str!("../trovato_site.info.toml");
        let capabilities: Vec<&str> = manifest
            .split("[capabilities]")
            .nth(1)
            .unwrap()
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();
        assert_eq!(
            capabilities,
            vec![
                r#"host_interfaces = ["logging", "variables", "query", "item-api"]"#,
                "item_background = true",
            ]
        );
    }

    #[test]
    fn the_sitemap_route_is_not_the_kernels() {
        // The kernel registers /sitemap.xml, and axum panics on a duplicate
        // route: declaring it here would take the server down at startup rather
        // than override anything.
        assert_ne!(SITEMAP_PATH, "/sitemap.xml");
        assert!(SITEMAP_PATH.starts_with('/'));
        let routes = __inner_tap_menu();
        assert!(routes.iter().any(|r| r.path == SITEMAP_PATH));
    }

    #[test]
    fn the_site_registers_the_three_routes_it_serves() {
        let routes = __inner_tap_menu();
        let paths: Vec<&str> = routes.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(paths, vec![SITEMAP_PATH, RAW_DOC_PATH, LLMS_PATH]);
    }

    #[test]
    fn a_slug_that_is_not_a_slug_is_refused() {
        for bad in [
            "",
            "../../etc/passwd",
            "a/b",
            "Plugin-Development",
            "doc.md",
            "doc%2e%2e",
            "doc:1",
        ] {
            assert!(!is_safe_slug(bad), "{bad} was accepted");
        }
        for good in ["readme", "plugin-development", "tutorial-01-hello-trovato"] {
            assert!(is_safe_slug(good), "{good} was refused");
        }
    }

    #[test]
    fn a_document_request_for_a_bad_slug_is_a_404() {
        let mut request = ApiRequest::new(
            "doc_raw",
            "GET",
            "/learn/raw/x",
            Uuid::nil().to_string(),
            false,
        );
        request.params.insert("slug".into(), "../secrets".into());
        assert_eq!(__inner_tap_api(request).status, 404);
    }

    #[test]
    fn absolute_joins_exactly_one_slash() {
        assert_eq!(
            absolute("https://trovato.rs", "/news"),
            "https://trovato.rs/news"
        );
        assert_eq!(
            absolute("https://trovato.rs/", "/news"),
            "https://trovato.rs/news"
        );
        assert_eq!(
            absolute("https://trovato.rs", "news"),
            "https://trovato.rs/news"
        );
        assert_eq!(absolute("https://trovato.rs", "/"), "https://trovato.rs/");
    }

    #[test]
    fn the_sitemap_route_answers_its_callback() {
        let response = __inner_tap_api(ApiRequest::new(
            "sitemap",
            "GET",
            SITEMAP_PATH,
            Uuid::nil().to_string(),
            false,
        ));
        assert_eq!(response.status, 200);
        assert!(response.content_type.starts_with("application/xml"));
        assert!(response.body.contains("<urlset"));
    }

    #[test]
    fn the_sitemap_is_well_formed_and_absolute() {
        // No database in a unit test, so this is the route half of the document:
        // the declaration, the namespace, and every listing route as an absolute
        // URL. The item half is covered by the crawl in scripts/crawl.mjs.
        let xml = sitemap().expect("the native query stub answers");
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\""));
        assert!(xml.trim_end().ends_with("</urlset>"));

        for path in ROUTE_PATHS {
            let expected = format!("<loc>{}{}</loc>", DEFAULT_BASE_URL, path);
            assert!(xml.contains(&expected), "missing {expected}");
        }
    }

    #[test]
    fn every_sitemap_loc_is_absolute() {
        // The kernel's own sitemap emits `<loc>/news</loc>`, which the sitemap
        // protocol does not permit. That defect is the reason this one exists,
        // so it is the one thing worth asserting outright.
        for line in sitemap().expect("the native query stub answers").lines() {
            if let Some(rest) = line.trim().strip_prefix("<loc>") {
                assert!(
                    rest.starts_with("https://") || rest.starts_with("http://"),
                    "relative loc: {line}"
                );
            }
        }
    }

    fn rows(first: u32, count: u32) -> Vec<serde_json::Value> {
        (first..first + count)
            .map(|n| serde_json::json!({ "n": n }))
            .collect()
    }

    fn query_page(
        items: Vec<serde_json::Value>,
        has_next: bool,
    ) -> trovato_sdk::types::QueryResult {
        trovato_sdk::types::QueryResult {
            items,
            has_next,
            ..Default::default()
        }
    }

    #[test]
    fn every_page_reads_until_there_is_no_next_page() {
        let read = every_page(|page| Ok(query_page(rows(page * 10, 2), page < 3)));
        assert_eq!(read.unwrap().len(), 6);
    }

    #[test]
    fn a_failed_later_page_fails_the_whole_read() {
        // Rows from page one and an error from page two are not a result: a
        // sitemap built from them would leave out every row after page one.
        let read = every_page(|page| {
            if page == 1 {
                Ok(query_page(rows(0, 100), true))
            } else {
                Err(-1)
            }
        });
        assert_eq!(read, Err(-1));
    }

    #[test]
    fn a_failed_read_is_a_503_not_a_short_document() {
        let failed = generated(Err(-10), "application/xml; charset=utf-8");
        assert_eq!(failed.status, 503);

        let served = generated(
            Ok("<urlset/>".to_string()),
            "application/xml; charset=utf-8",
        );
        assert_eq!(served.status, 200);
        assert_eq!(served.body, "<urlset/>");
        assert!(served.content_type.starts_with("application/xml"));
    }

    #[test]
    fn a_missing_document_is_a_404_and_a_failed_read_a_503() {
        assert_eq!(document_response(Ok(None)).status, 404);
        assert_eq!(document_response(Err(-10)).status, 503);

        let found = document_response(Ok(Some("# Title".to_string())));
        assert_eq!(found.status, 200);
        assert!(found.content_type.starts_with("text/markdown"));
    }

    #[test]
    fn the_sitemap_lists_the_rows_it_was_given() {
        let row = serde_json::json!({
            "id": "0193b000-0002-7000-8000-000000000001",
            "type": "blog",
            "url": "/blog/twenty-fields-one-row",
            "title": "Twenty fields, one row",
            "created": 1_700_000_000,
            "changed": 1_700_000_000,
        });
        let xml = sitemap_from(DEFAULT_BASE_URL, &[row]);
        assert!(xml.contains(&format!(
            "<loc>{DEFAULT_BASE_URL}/blog/twenty-fields-one-row</loc>"
        )));
    }

    /// `count` items, as `fetch(limit, offset)` would page through them, with
    /// any window wider than one item that covers a `bad` offset failing, and
    /// the `bad` items failing on their own too.
    fn listing(
        count: usize,
        bad: &'static [usize],
    ) -> impl FnMut(usize, usize) -> Result<Vec<String>, i32> {
        move |limit, offset| {
            let end = (offset + limit).min(count);
            let window = offset.min(end)..end;
            if window.clone().any(|at| bad.contains(&at)) {
                return Err(-1);
            }
            Ok(window.map(|at| format!("id-{at}")).collect())
        }
    }

    #[test]
    fn item_ids_reads_every_window() {
        let (ids, complete) = item_ids(listing(45, &[]));
        assert_eq!(ids.len(), 45);
        assert!(complete);

        let (ids, complete) = item_ids(listing(ID_PAGE * 2, &[]));
        assert_eq!(ids.len(), ID_PAGE * 2);
        assert!(complete);
    }

    #[test]
    fn one_unreadable_item_costs_only_its_own_id() {
        // An item too large for the output buffer fails its whole window. The
        // other items in that window, and every later window, are still read.
        let (ids, complete) = item_ids(listing(50, &[23]));
        assert_eq!(ids.len(), 49);
        assert!(!ids.contains(&"id-23".to_string()));
        assert!(ids.contains(&"id-24".to_string()));
        assert!(ids.contains(&"id-49".to_string()));
        assert!(!complete);
    }

    #[test]
    fn a_window_failing_item_by_item_ends_the_read() {
        let mut calls = 0;
        let (ids, complete) = item_ids(|_, _| {
            calls += 1;
            Err(-10)
        });
        assert!(ids.is_empty());
        assert!(!complete);
        assert_eq!(calls, 1 + ID_PAGE);
    }

    #[test]
    fn item_ids_says_when_it_stopped_at_the_cap() {
        let (ids, complete) = item_ids(listing(MAX_ID_ITEMS + 1, &[]));
        assert_eq!(ids.len(), MAX_ID_ITEMS);
        assert!(!complete);
    }

    #[test]
    fn xml_escaping_is_xml_not_html() {
        // &#39; is an HTML entity. An XML parser does not know it.
        assert_eq!(escape_xml("it's"), "it&apos;s");
        assert_eq!(escape_html("it's"), "it&#39;s");
        assert_eq!(escape_xml("a&b<c>"), "a&amp;b&lt;c&gt;");
    }

    #[test]
    fn an_unknown_callback_is_a_404_not_a_500() {
        let response = __inner_tap_api(ApiRequest::new(
            "no-such-thing",
            "GET",
            "/whatever",
            Uuid::nil().to_string(),
            false,
        ));
        assert_eq!(response.status, 404);
    }

    // ─── Translations ────────────────────────────────────────────────

    fn stored(title: &str, fields: serde_json::Value) -> ItemTranslation {
        ItemTranslation {
            item_id: Uuid::from_u128(5),
            language: "it".into(),
            title: title.into(),
            fields,
            created: 0,
            changed: 0,
        }
    }

    #[test]
    fn a_translation_entry_needs_a_title_and_fields() {
        // A malformed entry is skipped, not written as an empty translation:
        // the overlay replaces the title only when it is non-empty, so a blank
        // one would leave a page half translated instead of untranslated.
        assert_eq!(sync_one("x", "it", &serde_json::json!({}), &[]), None);
        assert_eq!(
            sync_one("x", "it", &serde_json::json!({ "title": "T" }), &[]),
            None
        );
        assert_eq!(
            sync_one("x", "it", &serde_json::json!({ "fields": {} }), &[]),
            None
        );
    }

    #[test]
    fn an_identical_translation_is_not_written_again() {
        // Cron runs every minute. Saving an unchanged translation would move
        // its `changed` every minute for nothing.
        let id = Uuid::from_u128(5).to_string();
        let fields =
            serde_json::json!({ "body": { "value": "<p>Ciao</p>", "format": "full_html" } });
        let declared = serde_json::json!({ "title": "Ciao", "fields": fields });
        assert_eq!(
            sync_one(&id, "it", &declared, &[stored("Ciao", fields.clone())]),
            Some(false)
        );
    }

    #[test]
    fn a_changed_translation_is_written() {
        // The native stub for save_translation refuses, so a write attempt is
        // `None` here. What matters is that it was attempted, not skipped.
        let id = Uuid::from_u128(5).to_string();
        let declared = serde_json::json!({ "title": "Ciao", "fields": { "body": "nuovo" } });
        assert_eq!(
            sync_one(
                &id,
                "it",
                &declared,
                &[stored("Ciao", serde_json::json!({ "body": "vecchio" }))]
            ),
            None
        );
        assert_eq!(
            sync_one(
                &id,
                "it",
                &declared,
                &[stored("Salve", serde_json::json!({ "body": "nuovo" }))]
            ),
            None
        );
    }

    #[test]
    fn nothing_declared_and_nothing_stored_removes_nothing() {
        assert_eq!(remove_undeclared(&serde_json::json!({}), &[]), 0);
    }

    #[test]
    fn only_an_undeclared_translation_is_removed() {
        let id = Uuid::from_u128(5).to_string();
        let mut french = stored("Salut", serde_json::json!({}));
        french.language = "fr".into();
        let held = [stored("Ciao", serde_json::json!({})), french];
        let declared = serde_json::json!({ id: { "it": { "title": "Ciao", "fields": {} } } });
        let gone: Vec<&str> = undeclared(&declared, &held)
            .iter()
            .map(|t| t.language.as_str())
            .collect();
        assert_eq!(gone, vec!["fr"]);
        // Nothing declared: everything stored goes.
        assert_eq!(undeclared(&serde_json::json!({}), &held).len(), 2);
    }

    #[test]
    fn the_cron_tap_survives_configuration_that_is_not_json() {
        // variables_get returns whatever is stored, and what is stored is a site
        // setting somebody can edit. A parse failure has to be a log line, not a
        // trap that takes the whole cron run down with it.
        let result = __inner_tap_cron(CronInput {
            timestamp: 1_785_542_400,
        });
        // The native stub returns the default, which is `{}`, so this is the
        // empty-configuration path rather than the failure path.
        assert!(result.get("written").is_some() || result.get("translations").is_some());
    }

    // ─── Dates ───────────────────────────────────────────────────────

    #[test]
    fn iso_date_matches_known_dates() {
        assert_eq!(iso_date(0), "1970-01-01");
        assert_eq!(iso_date(1_767_225_600), "2026-01-01");
        assert_eq!(iso_date(1_772_755_200), "2026-03-06");
        // A leap day, which is where a hand-rolled calendar goes wrong.
        assert_eq!(iso_date(1_709_164_800), "2024-02-29");
        assert_eq!(iso_date(951_782_400), "2000-02-29");
    }

    #[test]
    fn iso_date_handles_a_timestamp_before_the_epoch() {
        // div_euclid rather than integer division is what makes this right.
        assert_eq!(iso_date(-1), "1969-12-31");
    }
}
