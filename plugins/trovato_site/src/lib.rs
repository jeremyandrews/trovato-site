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

/// How many recent posts the front page lists.
const FRONT_LISTING_LIMIT: i64 = 5;

/// The live stage. Every published page on this site is in it.
const LIVE_STAGE: &str = "0193a5a0-0000-7000-8000-000000000001";

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
    perms
}

/// Author access for the site's content types.
///
/// The same shape the blog plugin uses: an author reaches their own item at any
/// stage of publication, and everyone else falls through to the kernel's
/// permission fallback (`edit news content`, and so on).
#[plugin_tap]
pub fn tap_item_access(input: ItemAccessInput) -> AccessResult {
    if input.item_type != NEWS_TYPE && input.item_type != FRONT_PAGE_TYPE {
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
        assert_eq!(names, vec!["news", "front_page"]);
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
        // Four CRUD permissions per type.
        assert_eq!(perms.len(), 8);
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
