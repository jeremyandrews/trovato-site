//! The trovato.rs website.
//!
//! This plugin is the whole of the site's Rust. Everything else the site adds —
//! templates, config, static assets, seed content — is data layered on through the
//! kernel's `TEMPLATES_DIR`, `STATIC_DIR` and config import. Nothing in the Trovato
//! tree is modified, and nothing here is a copy of something the kernel already does.
//!
//! It contributes two things:
//!
//! 1. The `news` content type. Announcements written in the site's own voice, kept
//!    separate from `blog` so that a release note and an essay can be listed, styled
//!    and syndicated apart from each other.
//!
//! 2. The front page's listing of recent posts, server-rendered.
//!
//! # Why the listing is here and not in a template or a tile
//!
//! The obvious places to put a "recent posts" list on a front page do not work on
//! 0.101.0, and both failures were found by trying them:
//!
//! - A `gather_query` **tile** renders `<div class="tile-gather" data-query-id="…">`
//!   and stops. Nothing in the kernel's JavaScript hydrates that element, so the tile
//!   is an empty div wherever it is placed.
//! - A **template** cannot reach Gather. The theme engine registers filters, not
//!   functions, and none of them run a query.
//!
//! What does work is `tap_item_view`, whose output the front-page handler appends to
//! the rendered item just as the item route does. So the listing is a plugin that
//! declares `db`, reads published content, and returns HTML. That is more honest than
//! the alternatives anyway: the site's front page is a live demonstration of the
//! sandbox model it describes three paragraphs further down the same page.
//!
//! The plugin reads two tables and declares exactly those two. It writes nothing.

use trovato_sdk::host;
use trovato_sdk::prelude::*;
use trovato_sdk::types::{ApiRequest, ApiResponse, MenuRoute};

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
const FRONT_LISTING_LIMIT: i64 = 5;

/// The live stage. Every published page on this site is in it.
const LIVE_STAGE: &str = "0193a5a0-0000-7000-8000-000000000001";

/// The site variable holding the base every absolute URL is built from.
///
/// The same value the kernel takes as `SITE_URL`, declared a second time as
/// config because a plugin can read a site variable and cannot read the
/// process's environment. `checks` has a test that fails if the two defaults
/// drift apart.
const BASE_URL_VARIABLE: &str = "site_base_url";

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
            description: "A page of the Trovato documentation, mirrored from the kernel repository. Generated — edit the source there, not here.".into(),
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

/// Append the calls to action and the recent-posts listing to the front page.
///
/// Returns an empty string for every other item, which is every item but one.
///
/// The two links are here rather than in the front page's body because
/// `filtered_html` strips every attribute a link would need to be a button, and
/// rather than in the template because the template cannot get between the body
/// and this output — the kernel hands an item template one blob containing both.
/// They are navigation, so a plugin is a defensible place for them; that they
/// are also the only copy in this file is why they are named as constants.
#[plugin_tap]
pub fn tap_item_view(item: Item) -> String {
    if item.item_type != FRONT_PAGE_TYPE {
        return String::new();
    }

    let mut html = render_actions();
    html.push_str(&render_listing(&recent_posts(FRONT_LISTING_LIMIT)));
    html
}

/// The front page's two calls to action.
const ACTIONS: [(&str, &str, &str); 2] = [
    ("/get-started", "Get started", "button"),
    ("/why", "Why Trovato", "button button--secondary"),
];

/// Render the calls to action.
pub fn render_actions() -> String {
    let mut html = String::from("<nav class=\"front-hero__actions\" aria-label=\"Get started\">\n");
    for (href, label, class) in ACTIONS {
        html.push_str(&format!(
            "<a class=\"{}\" href=\"{}\">{}</a>\n",
            escape_html(class),
            escape_html(href),
            escape_html(label)
        ));
    }
    html.push_str("</nav>\n");
    html
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
///
/// One query rather than two: the listing is a single reverse-chronological
/// sequence, so the database is the right place to merge and cut it. The alias
/// join is a LEFT JOIN because a post without an alias still belongs in the list —
/// it just links to its item path.
fn recent_posts(limit: i64) -> Vec<Post> {
    rows(
        LISTING_SQL,
        &[serde_json::json!(LIVE_STAGE), serde_json::json!(limit)],
    )
    .iter()
    .filter_map(post_from_row)
    .collect()
}

/// The listing query.
///
/// One stage parameter, compared as a uuid on both sides. `item.stage_id` and
/// `url_alias.stage_id` are both `uuid` columns on a migrated database — the
/// `VARCHAR(50) DEFAULT 'live'` in `20260216000004_create_url_alias.sql` is what
/// the column was created as, not what it is, and a later migration changed it.
/// Reading the create-table migration and stopping there is how the alias join
/// ends up comparing a uuid against the text `'live'`, which is not a mismatch
/// Postgres tolerates: it refuses the query outright, the tap logs and returns
/// nothing, and the front page renders with no listing at all.
///
/// The LEFT JOIN is what lets a post link to its friendly path. A post with no
/// alias still belongs in the list and falls back to `/item/{uuid}`.
const LISTING_SQL: &str = "SELECT i.id, i.title, i.type AS kind, i.created, a.alias \
     FROM item i \
     LEFT JOIN url_alias a \
       ON a.source = '/item/' || i.id::text \
      AND a.language = COALESCE(i.language, 'en') \
      AND a.stage_id = $1::uuid \
     WHERE i.status = 1 \
       AND i.type IN ('blog', 'news') \
       AND i.stage_id = $1::uuid \
     ORDER BY i.created DESC \
     LIMIT $2";

/// Build a `Post` from one result row, or nothing if the row is missing a field
/// the listing cannot render without.
pub fn post_from_row(row: &serde_json::Value) -> Option<Post> {
    let id = row.get("id")?.as_str()?;
    let path = row
        .get("alias")
        .and_then(|v| v.as_str())
        .filter(|a| !a.is_empty())
        .map_or_else(|| format!("/item/{id}"), String::from);

    Some(Post {
        path,
        title: row.get("title")?.as_str()?.to_string(),
        kind: row.get("kind")?.as_str()?.to_string(),
        created: row.get("created").and_then(serde_json::Value::as_i64)?,
    })
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
        "sitemap" => {
            ApiResponse::with_status(200, sitemap()).content_type("application/xml; charset=utf-8")
        }
        "doc_raw" => match request.params.get("slug").map(String::as_str) {
            Some(slug) => match doc_markdown(slug) {
                Some(markdown) => ApiResponse::with_status(200, markdown)
                    .content_type("text/markdown; charset=utf-8"),
                None => ApiResponse::error(404, "no such document"),
            },
            None => ApiResponse::error(404, "no such document"),
        },
        "llms" => {
            ApiResponse::with_status(200, llms_txt()).content_type("text/plain; charset=utf-8")
        }
        other => {
            host::log("warn", PLUGIN_NAME, &format!("unknown callback: {other}"));
            ApiResponse::error(404, "not found")
        }
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
pub fn sitemap() -> String {
    let base = base_url();
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");

    for path in ROUTE_PATHS {
        xml.push_str(&sitemap_entry(&absolute(&base, path), None));
    }

    for row in rows(SITEMAP_SQL, &[serde_json::json!(LIVE_STAGE)]).iter() {
        let Some(post) = post_from_row(row) else {
            continue;
        };
        let changed = row.get("changed").and_then(serde_json::Value::as_i64);
        xml.push_str(&sitemap_entry(
            &absolute(&base, &post.path),
            changed.map(iso_date),
        ));
    }

    xml.push_str("</urlset>\n");
    xml
}

/// Every published item in the live stage, with its alias when it has one.
///
/// The same shape `LISTING_SQL` returns — `post_from_row` reads both — with
/// `changed` added for `<lastmod>`.
///
/// The front page is excluded. It has no alias, because its address is `/` and
/// `/` cannot be aliased, so it would otherwise appear a second time as
/// `/item/{uuid}` — the same page at two URLs, in the document whose whole
/// purpose is telling a crawler which URLs exist. `/` is listed once, from
/// `ROUTE_PATHS`.
const SITEMAP_SQL: &str = "SELECT i.id, i.title, i.type AS kind, i.created, i.changed, a.alias \
     FROM item i \
     LEFT JOIN url_alias a \
       ON a.source = '/item/' || i.id::text \
      AND a.language = COALESCE(i.language, 'en') \
      AND a.stage_id = $1::uuid \
     WHERE i.status = 1 \
       AND i.stage_id = $1::uuid \
       AND i.type <> 'front_page' \
     ORDER BY i.changed DESC";

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
fn doc_markdown(slug: &str) -> Option<String> {
    if !is_safe_slug(slug) {
        return None;
    }

    let alias = format!("/learn/{slug}");
    rows(
        DOC_MARKDOWN_SQL,
        &[serde_json::json!(LIVE_STAGE), serde_json::json!(alias)],
    )
    .first()?
    .get("markdown")?
    .as_str()
    .map(str::to_string)
}

/// One documentation page's source, found by the alias it is served at.
const DOC_MARKDOWN_SQL: &str = "SELECT i.fields->>'markdown' AS markdown \
     FROM item i \
     JOIN url_alias a \
       ON a.source = '/item/' || i.id::text \
      AND a.language = COALESCE(i.language, 'en') \
      AND a.stage_id = $1::uuid \
     WHERE i.status = 1 \
       AND i.type = 'docs' \
       AND i.stage_id = $1::uuid \
       AND a.alias = $2 \
     LIMIT 1";

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
pub fn llms_txt() -> String {
    let base = base_url();
    let mut out = String::from("# Trovato\n\n");
    out.push_str(
        "> A content management system written in Rust. Content is one JSONB row per item \
         rather than a join per field; plugins are WebAssembly modules that reach only what \
         they declare; queries are built with Gather, a type-safe query engine.\n\n",
    );
    out.push_str(
        "This file lists the site's documentation and the markdown each page was rendered \
         from. Every `.md` link below is the source text, unrendered.\n\n",
    );

    out.push_str("## Documentation\n\n");
    let docs = rows(DOCS_SQL, &[serde_json::json!(LIVE_STAGE)]);
    if docs.is_empty() {
        out.push_str("The documentation has not been imported yet.\n\n");
    }
    for row in docs.iter() {
        let Some(title) = row.get("title").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(alias) = row.get("alias").and_then(|v| v.as_str()) else {
            continue;
        };
        let slug = alias.rsplit('/').next().unwrap_or_default();
        out.push_str(&format!(
            "- [{}]({}): markdown at {}\n",
            title,
            absolute(&base, alias),
            absolute(&base, &format!("/learn/raw/{slug}")),
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
            "Contributing, the code of conduct, and how this is built.",
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
            absolute(&base, path),
            description
        ));
    }

    out.push_str("\n## Source\n\n");
    out.push_str("- [The kernel](https://github.com/jeremyandrews/trovato)\n");
    out.push_str("- [This website](https://github.com/jeremyandrews/trovato-site)\n");
    out
}

/// Every documentation page, in reading order, with its alias.
const DOCS_SQL: &str = "SELECT i.title, a.alias, i.fields->>'weight' AS weight \
     FROM item i \
     JOIN url_alias a \
       ON a.source = '/item/' || i.id::text \
      AND a.language = COALESCE(i.language, 'en') \
      AND a.stage_id = $1::uuid \
     WHERE i.status = 1 \
       AND i.type = 'docs' \
       AND i.stage_id = $1::uuid \
     ORDER BY (i.fields->>'weight')::int NULLS LAST, i.title";

/// Run a query and return its rows, or an empty list and a log line on failure.
fn rows(sql: &str, params: &[serde_json::Value]) -> Vec<serde_json::Value> {
    match host::query_raw(sql, params) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(code) => {
            host::log("error", PLUGIN_NAME, &format!("query failed: {code}"));
            Vec::new()
        }
    }
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

    #[test]
    fn the_front_page_gets_the_calls_to_action_before_the_listing() {
        let html = __inner_tap_item_view(item_of_type("front_page"));
        assert!(html.contains("front-hero__actions"));
        assert!(html.contains("href=\"/get-started\""));
        assert!(html.contains("href=\"/why\""));
        // The listing itself needs a database, which a unit test has none of;
        // what is pinned here is that the actions render without one.
        assert!(html.starts_with("<nav class=\"front-hero__actions\""));
    }

    #[test]
    fn the_calls_to_action_are_a_labelled_nav() {
        // Two links in a row with no label is a nav a screen reader cannot tell
        // apart from the main menu.
        assert!(render_actions().contains("aria-label=\"Get started\""));
    }

    // ─── Row mapping ─────────────────────────────────────────────────

    #[test]
    fn a_row_with_an_alias_uses_it() {
        let row = serde_json::json!({
            "id": "0193a5a0-0000-7000-8000-00000000000a",
            "title": "Hello",
            "kind": "blog",
            "created": 1_767_225_600_i64,
            "alias": "/blog/hello",
        });
        assert_eq!(post_from_row(&row).unwrap().path, "/blog/hello");
    }

    #[test]
    fn a_row_without_an_alias_falls_back_to_the_item_path() {
        let row = serde_json::json!({
            "id": "0193a5a0-0000-7000-8000-00000000000a",
            "title": "Hello",
            "kind": "blog",
            "created": 1_767_225_600_i64,
            "alias": serde_json::Value::Null,
        });
        assert_eq!(
            post_from_row(&row).unwrap().path,
            "/item/0193a5a0-0000-7000-8000-00000000000a"
        );
    }

    #[test]
    fn an_empty_alias_is_not_a_path() {
        // A LEFT JOIN can hand back an empty string as readily as a null, and
        // href="" reloads the current page.
        let row = serde_json::json!({
            "id": "0193a5a0-0000-7000-8000-00000000000a",
            "title": "Hello",
            "kind": "blog",
            "created": 1_767_225_600_i64,
            "alias": "",
        });
        assert_eq!(
            post_from_row(&row).unwrap().path,
            "/item/0193a5a0-0000-7000-8000-00000000000a"
        );
    }

    #[test]
    fn a_row_missing_a_title_is_dropped_rather_than_rendered_blank() {
        let row = serde_json::json!({
            "id": "0193a5a0-0000-7000-8000-00000000000a",
            "kind": "blog",
            "created": 1_767_225_600_i64,
        });
        assert_eq!(post_from_row(&row), None);
    }

    // ─── The query ───────────────────────────────────────────────────

    #[test]
    fn the_listing_compares_both_stage_columns_against_one_parameter() {
        // url_alias.stage_id is text and item.stage_id is a uuid. Binding them
        // from two different literals is how the alias join silently matches
        // nothing and every link in the listing falls back to /item/{uuid}.
        assert!(LISTING_SQL.contains("a.stage_id = $1::uuid"));
        assert!(LISTING_SQL.contains("i.stage_id = $1::uuid"));
        assert!(
            !LISTING_SQL.contains("'live'"),
            "the alias stage must come from the parameter, not a literal"
        );
    }

    #[test]
    fn the_listing_is_limited_and_ordered_newest_first() {
        assert!(LISTING_SQL.contains("ORDER BY i.created DESC"));
        assert!(LISTING_SQL.contains("LIMIT $2"));
    }

    #[test]
    fn the_listing_only_reads_the_tables_the_manifest_declares() {
        // db_tables in trovato_site.info.toml is ["item", "url_alias"].
        // A table read here without being declared there is refused at load.
        for table in ["comment", "users", "session"] {
            assert!(
                !LISTING_SQL.contains(table),
                "the listing reads {table}, which the manifest does not declare"
            );
        }
    }

    // ─── The sitemap ─────────────────────────────────────────────────

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
    fn the_queries_are_written_as_one_line_each() {
        // A `\\` where a `\` belongs turns a Rust line continuation into a
        // literal backslash in the SQL, and Postgres answers `syntax error at or
        // near "\"`. It is silent: the tap logs, returns nothing, and the page
        // renders as though there were no rows.
        for (name, sql) in [
            ("LISTING_SQL", LISTING_SQL),
            ("SITEMAP_SQL", SITEMAP_SQL),
            ("DOCS_SQL", DOCS_SQL),
            ("DOC_MARKDOWN_SQL", DOC_MARKDOWN_SQL),
        ] {
            assert!(!sql.contains('\\'), "{name} carries a literal backslash");
            assert!(!sql.contains('\n'), "{name} carries a newline");
        }
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
        let xml = sitemap();
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
        for line in sitemap().lines() {
            if let Some(rest) = line.trim().strip_prefix("<loc>") {
                assert!(
                    rest.starts_with("https://") || rest.starts_with("http://"),
                    "relative loc: {line}"
                );
            }
        }
    }

    #[test]
    fn the_sitemap_excludes_the_front_page_item() {
        // `/` is listed once, from ROUTE_PATHS. The front page item has no alias
        // — `/` cannot be aliased — so without this it appears a second time as
        // /item/{uuid}: the same page at two URLs, in the one document whose job
        // is to say which URLs exist.
        assert!(SITEMAP_SQL.contains("i.type <> 'front_page'"));
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
