# Build ledger

**Status:** Complete. All nine gates passed.
**Last updated:** 2026-08-25
**Where things stand:** The site is built, checked and reproducible. A clean-room rebuild
reaches a populated, themed, working site in 24 seconds from destroyed volumes, and every
gate passes against it. What stands between this repository and https://trovato.rs serving
it is DNS, a host, and the decisions in `docs/LAUNCH.md`.

This file is the run's memory. Each gate attempt is recorded below with its per-check
result, the loop count, any blockers, and the commit that closed the phase. A session
that starts cold reads this file and continues from the first unmet gate.

## Pinned facts

| Fact | Value |
|---|---|
| Kernel release | `v0.101.0` (commit `5304a68`) |
| Kernel image | `ghcr.io/jeremyandrews/trovato:0.101.0` (digest `sha256:cf9c7580b4e3…`) |
| Compose project | `trovato-site` |
| Site port (local) | `127.0.0.1:3080` |
| Proxy port (local, Phase 8) | `127.0.0.1:8443` |
| Base URL | `https://trovato.rs` |
| Scratch kernel clone | `../trovato-scratch`, read-only, never edited |

## Gate 0 — baseline and preflight

**Attempt 1 — 2026-08-24 — PASS (0 fix loops)**

| Check | Result |
|---|---|
| `gh auth status` succeeds | PASS — account `jeremyandrews`, scopes `gist, read:org, repo, workflow` |
| Repository exists with bootstrap commit pushed | PASS — created this run, see commit below |
| Newest release tag determined and recorded | PASS — `v0.101.0` |
| Image pulls from ghcr | PASS — `sha256:cf9c7580b4e3…`, arm64, 72 MB |
| Bare kernel serves against Postgres and Redis | PASS — `/health` 200 with `postgres:true, redis:true`; `/` 303 to `/install` |
| Project name and ports recorded and free | PASS — `trovato-site`, 3080 and 8443 both unbound |
| Scratch clone in place at the tag | PASS — `git describe` = `v0.101.0` |
| CI stubbed | PASS — `.github/workflows/ci.yml` |
| Ledger started and pushed | PASS — this file |

### Findings

- **GitHub Releases lags the tags.** `gh release list` shows `v0.99.0` as Latest, but
  `v0.100.0` and `v0.101.0` both exist as tags and `0.101.0` is published to ghcr.
  Determining the newest release from tags rather than from the Releases page is what
  found this. Kernel-side housekeeping, not a blocker.
- **Headless install works.** A fresh kernel redirects `/` to `/install`. The two
  installer POSTs (`/install/admin`, `/install/site`) carry no CSRF token, so the whole
  install is scriptable with `curl`. This is what makes the clean-room rebuild in Phase 9
  possible without a human at a browser. Verified against 0.101.0 during preflight.
- **No documentation of the overlay pattern.** `PLUGINS_DIR`, `TEMPLATES_DIR` and
  `STATIC_DIR` as colon-separated search paths appear in `crates/kernel/src/config.rs`
  and its tests, but in no file under `docs/`. An external site is the pattern's main
  consumer and has nothing to read. Kernel-side documentation gap; recorded, not fixed.


## Gate 1 — skeleton

**Attempt 1 — 2026-08-24 — PASS (2 fix loops)**

Run against a stack built from empty volumes by `./scripts/run.sh --fresh`.

| Check | Result |
|---|---|
| Site plugin enables on the released kernel | PASS — `trovato_site 0.1.0 enabled`, api_version `0.101` against kernel `0.101` |
| Config and seed content import cleanly | PASS — 19 entities: 1 gather_query, 6 items, 9 menu_links, 2 url_aliases, 1 variable |
| Front page renders the recent-posts listing | PASS — 4 items, newest first, news and blog interleaved, server-rendered |
| Front page renders the search box | PASS — `<form action="/search" method="get" role="search">`, no JavaScript involved |
| `/news` lists the seed posts | PASS — all three, newest first |
| A blog post renders with SEO tags in `<head>` | PASS — description, canonical, og:title, og:type, og:url, og:site_name, og:description, twitter:card |
| The contact page serves its no-JavaScript form | PASS — POST to `/contact` with `_token`, name, email, subject, message |
| An anonymous visitor gets 200s across the public page set | PASS — `/`, `/news`, `/blog`, `/why`, `/contact`, `/search`, `/health` all 200 |
| CI green | PASS — run on `348ba22`, fmt + clippy `-D warnings` + 21 tests + wasm build |

### The two fix loops

1. **`operator does not exist: uuid = text`.** The front-page listing joins `item`
   to `url_alias` on the stage. `20260216000004_create_url_alias.sql` creates
   `stage_id` as `VARCHAR(50) DEFAULT 'live'`, so the first version of the query
   compared it against text. On a migrated database the column is `uuid`; a later
   migration changed it and the create-table file is no longer what the schema
   says. Postgres refuses the comparison rather than coercing it, so the tap
   returned nothing and the front page rendered with no listing and no visible
   error. Fixed by comparing both sides as `uuid` from one parameter, and pinned
   by a test that fails if either side stops using it.

2. **Imported config is not live until the server reads it again.** After
   `config import`, `/news` answered `{"error":"query not found"}` and the main
   menu rendered empty. Gather queries and menu links are read into their
   registries at startup, so a query written to the database while the server is
   running is not routable. `scripts/run.sh` now restarts the site after import,
   which is why the whole thing is one command rather than a command plus a note
   telling somebody to remember this.

### Findings

Site-side decisions and kernel-side observations, none of them fixed in the
Trovato tree.

- **A `gather_query` tile is inert.** `TileService::render_tile_html` emits
  `<div class="tile-gather" data-query-id="…"></div>` and stops. No file under
  `static/js/` mentions `tile-gather`, so nothing ever fills it. A tile of this
  type renders an empty div in every region. *Trovato-side.*
- **A template cannot reach Gather.** `ThemeEngine` registers Tera filters and no
  functions, so there is no way to run a query from a template. Together with the
  tile above, this leaves `tap_item_view` as the only way to put a live listing on
  a page the site controls. *Trovato-side; the site works with it rather than
  around it.*
- **Config import cannot promote content.** `ConfigItem` has no `promote` or
  `sticky` field and `DirectConfigStorage::save_item` writes `0` for both. The
  kernel's default front page is the promoted-items listing, so a site whose
  content is defined in config can never reach it. This is why the front page is
  an item with a plugin-rendered listing rather than the built-in one.
  *Trovato-side.*
- **`trovato_blog` declares a tap it does not export.** Its `.info.toml` lists
  `tap_item_view` under `[taps].implements`, but `src/lib.rs` defines no such
  function. Every item view logs `tap invocation failed … tap 'tap_item_view' not
  exported` at ERROR. Harmless to render, noisy in logs, and it means an ERROR
  line in this kernel's log is not by itself a signal. *Trovato-side.*
- **Seventeen plugins log a route warning at startup.** `menu entry declares a
  callback but handler_type is not "api"; the kernel will never dispatch it and
  this path will 404` — from `trovato_blog`, `trovato_media`, `trovato_comments`
  and others, for paths including `/blog` itself. `/blog` nonetheless serves 200,
  because the path is reached through the blog plugin's gather query rather than
  through the callback. *Trovato-side.*
- **Field naming is inconsistent, and it shows.** The kernel's `page` type calls
  its field `body`; `trovato_blog` calls its field `field_body`. The kernel's own
  `gather/query--blog_listing.html` and its promoted-items renderer both read
  `fields.body`, so a blog post's teaser renders with a title, a date and no text
  at all. Verified on this site: `/blog` shows the post's title and none of its
  body. The site's own types use `body`, matching the kernel type and the
  templates rather than the blog plugin. *Trovato-side; the site sidesteps it.*
- **`SITE_URL` is the single base URL, and it already exists.** Canonicals, Open
  Graph URLs and everything else absolute resolve against it, defaulting to
  `http://localhost:{PORT}`. The compose file sets it to `https://trovato.rs`, so
  the local stack emits production URLs and there is no second setting to forget.
  Verified: a local blog post's canonical reads `https://trovato.rs/item/…`.

### Deferred to later phases

- Seed items link as `/item/{uuid}` in the front listing because they have no
  aliases yet. The LEFT JOIN that picks an alias up is in place and proven by the
  fallback; per-item aliases land in Phase 3.
- `/news` rows render titles and dates but no summaries: the stock
  `gather/row.html` reads `row.summary`, and a news item's summary is in
  `fields.summary`. The listing template override lands in Phase 3.


## Gate 2 — theme and brand

**Attempt 1 — 2026-08-24 — PASS (3 fix loops)**

| Check | Result |
|---|---|
| Key pages render at 360px and 1280px in both schemes | PASS — 28 screenshots (7 pages x 2 widths x 2 schemes), all 200 |
| Layout intact, nav usable at both widths | PASS — at 360px the nav is a `<details>` disclosure that opens on Enter and exposes all 5 links; at 1280px the links are shown and the toggle is hidden |
| No horizontal scroll at 360px | PASS — measured, `scrollWidth <= clientWidth` on all 28 |
| Dark mode actually dark everywhere | PASS — body background `rgb(26, 21, 18)` on every dark shot, and an automated sweep of every visible element found nothing carrying a background from the wrong scheme |
| Computed contrast of every token pairing passes WCAG AA | PASS — 7 tests in the `checks` crate over 24 declared pairings; the checker was verified to fail by breaking a token and by deleting a dark override |
| Fonts load from the site's own origin | PASS — `200 http://127.0.0.1:3080/static/fonts/Inter-{Regular,SemiBold}.woff2`, and a request interceptor recorded zero off-origin requests across all 28 loads |

### The three fix loops

1. **The main menu vanished on a desktop.** `page--front.html` overrode a
   `{% block main %}` nested inside `page.html`'s `content` block. Tera resolves a
   nested block overridden from a grandchild by dropping its siblings in the
   parent, so the front page rendered an empty `<header>` while every other page
   kept its own — silently, with the element present. Fixed by keeping each block
   one level of inheritance deep.

2. **Content bodies rendered as escaped source.** The seed items were written
   `format: full_html`. The item render path uses `FilterPipeline::for_format_safe`,
   which permits `plain_text` and `filtered_html` and downgrades anything else to
   `plain_text` — so the front page displayed its own markup as text. Fixed by
   using `filtered_html`, which then turned up the constraint below.

3. **Controls the site does not own were unthemed.** The contact form's submit was
   a browser-default white box on a dark page, and the search widget's button was
   navy `#1a3a5c` on cream. Both were found by the automated sweep rather than by
   looking. Fixed by styling bare `button`/`input[type=submit]` and by mapping the
   fourteen `--scolta-*` custom properties onto the site's tokens. The widget's
   button text is hard-coded white, which measures 2.65:1 on the dark scheme's
   Clay Light fill, so that one is overridden by id.

### Findings

- **Static assets are rate-limited as API calls, and the limits are not
  configurable.** `categorize_path` puts every GET that is not login, register,
  upload, search, comment or `/api/` into the `api` bucket — including
  `/static/*`. The bucket is 100 per minute per IP, hard-coded in
  `RateLimitConfig::default()` with no environment knob. Measured: the 101st
  consecutive request for `/static/css/site.css` returns `429` with
  `retry-after: 60`. A page of this site pulls about ten subresources, so a
  visitor reading ten pages in a minute has their fonts throttled and the page
  falls back to Times — which is exactly what happened here during a screenshot
  run. *Trovato-side.* It also settles a Phase 8 decision: the production front
  proxy serves `/static` itself, so static never reaches the kernel and never
  counts against anybody's budget.
- **The default CSP is looser than any page needs.** The kernel sends
  `script-src 'self' 'wasm-unsafe-eval' https://cdn.jsdelivr.net`,
  `style-src … https://fonts.googleapis.com` and
  `font-src 'self' https://fonts.gstatic.com`. This site requests nothing
  off-origin — verified across all 28 page loads — so three external origins are
  permitted that nothing uses. *Trovato-side;* Phase 8's proxy tightens it for
  this deployment.
- **A content body cannot carry a styling hook.** `filtered_html` allows `href`,
  `title` and `target` on a link, `src`/`alt`/`title`/`width`/`height` on an image
  and `colspan`/`rowspan` on a cell. Everything else, `class` included, is
  stripped. So the front page's lede is styled by position (`p:first-child`) and
  its two calls to action are rendered by the site plugin, because a link written
  in the body cannot be given a button class. Not a defect — it is what a
  filtered format is for — but it decides how a themed site is built.
- **Config import does not update an existing item's dates.** `save_item`'s
  `ON CONFLICT` clause updates `title`, `fields`, `status` and `language` and
  leaves `created` and `changed` as first written. Correcting a seed item's date
  in config and re-importing therefore changes nothing; it took a `--fresh`
  rebuild. *Trovato-side.* Consistent with the Gate 1 finding that import cannot
  set `promote` either: the item importer writes a subset of the row.
- **Clay on Cream measures 4.95:1, not the 3-to-4.5 the plan assumed.** It clears
  AA for body text. Body text is still Ink at 15.67:1, because a page of
  terracotta prose is unpleasant whatever it passes — but link color did not need
  to be darkened for the page background. It was darkened anyway: on
  `--surface-sunken` the lighter clay measures 4.42:1 and fails, and one link
  color that passes everywhere beats two that each pass in one place.

### What the theme is

Palette and lockups from `assets/brand/BRAND.md`, unchanged. Inter and JetBrains
Mono, self-hosted from `static/fonts/` with their licenses, five files and 548 KB
in total, `font-display: swap` and real fallback stacks behind them. A minor-third
type scale with the two largest steps fluid. Mobile-first, one breakpoint at 48rem.
Motion is a chevron rotation and a button hover, both behind
`prefers-reduced-motion`. No inline `<style>` anywhere: `templates/base.html` is
overridden precisely because the kernel's carries 350 lines of blue-palette CSS
that no block could reach.


## Gate 3 — content architecture and IA

**Attempt 1 — 2026-08-24 — PASS (2 fix loops)**

| Check | Result |
|---|---|
| Every IA route serves with the intended template | PASS — all 14 destinations plus 3 documents return 200; a crawl of 37 pages found no raw-dump fallbacks |
| Menus correct on every page | PASS — 7 main and 10 footer links from config, `aria-current="page"` on the active one, both navs labelled |
| Breadcrumbs correct on every page | PASS — `Home › News › Trovato 0.101.0` on an item, `Home › News` on a listing, `Home › Why Trovato` on a page |
| Feeds validate and autodiscover | PASS — `/rss/news.xml` and `/rss/blog.xml` are well-formed XML (`xmllint`), carry absolute URLs and correct `pubDate`s, and both appear as `<link rel="alternate">` in every page's head |
| Sitemap covers the public set with absolute URLs | PASS — 16 entries at `/sitemap/pages.xml`, every `<loc>` absolute under `https://trovato.rs`, well-formed XML |
| From a clean database, `config import` stands the whole structure up | PASS — `./scripts/run.sh --fresh` from empty volumes; 51 entities import and the site is complete |
| Export/import round-trip | PASS — `./scripts/check-roundtrip.sh`: all 51 declared entities appear in an export with every declared key unchanged |

### The two fix loops

1. **A breadcrumb template that did not parse served a raw dump.** The first
   version used a Tera object literal (`{% set m = {"News": "/news"} %}`). Tera
   has none, and a template that fails to parse is not an error: the kernel falls
   back to writing the item's fields into a bare `<html><body>` with no head, no
   navigation and no styling, and returns it with a 200. The page looked fine to
   a status-code check. This is why `scripts/crawl.mjs` checks for the shape of a
   document rather than only its status, and why it caught the second loop.

2. **Two broken links on the login page.** `/user/register` 404'd because
   registration was off, and "Forgot password?" pointed at `/user/password-reset`,
   which is registered POST-only — a GET returns 405. The human-facing recovery
   page is `/user/recover`. Fixed by enabling registration (pulled forward from
   Phase 6, see below) and by overriding the login template.

### Findings

- **The kernel's `sitemap.xml` emits relative URLs.** `<loc>/news</loc>`, where
  the sitemap protocol requires an absolute URL. It also lists items only, so
  every listing route is missing. *Trovato-side.* The site cannot fix it in place:
  the kernel registers `/sitemap.xml` and axum panics on a duplicate route, so a
  plugin declaring it would take the process down at startup. The site serves a
  correct one at `/sitemap/pages.xml` instead, and Phase 8's proxy maps
  `/sitemap.xml` onto it.
- **The kernel's login page ships an inline script its own CSP blocks.** The
  passkey ceremony is an inline `<script>`; the CSP is
  `script-src 'self' 'wasm-unsafe-eval' https://cdn.jsdelivr.net` with no
  `'unsafe-inline'`. Verified in a browser: the console reports the violation and
  the passkey button stays hidden. Passkey sign-in does not work on a default
  install. *Trovato-side.* The site's login override serves the same code from
  `/static/js/passkey-login.js`, where `'self'` covers it.
- **"Forgot password?" is a dead link in the kernel.** It points at
  `/user/password-reset`, which `password_reset::router` registers as `post` only.
  `/user/recover` is the page a person is meant to reach. *Trovato-side.*
- **Gather cannot link to a friendly URL.** A relationship joins on column
  equality, and the alias join is an expression
  (`url_alias.source = '/item/' || item.id::text`); `includes` matches on plain
  fields too, and no Tera filter resolves an alias. So every gather listing links
  to `/item/{uuid}` — the kernel's own `query--blog_listing.html` does exactly
  that. *Trovato-side.* What the site does about it: canonical URLs and every
  sitemap entry are the alias, so what a crawler indexes is the friendly path; and
  the front page's listing, which the site plugin renders rather than Gather,
  does link to aliases. That asymmetry is the demonstration that the data is
  there and only the query layer cannot reach it.
- **`elements/comments.html` is not in the image.** `trovato_comments` is enabled
  by default and the kernel logs `failed to render the comment thread … Template
  'elements/comments.html' not found` on every item view. *Trovato-side;* Phase 6
  supplies the template.
- **The kernel's field naming, again.** Its own `gather/row.html` reads
  `row.summary`, a top-level key no item has — a summary lives at
  `row.fields.summary`. Both listing templates are overridden partly for this.

### Decisions

- **`/learn` will use `trovato_book`.** It gives an ordered hierarchy with
  previous, next and up links, which is what a documentation series is, and the
  alternative is a docs content type that reimplements it. The tradeoff: a book's
  tree lives in the plugin's `book_page` table, which is not a config entity, so
  `/learn`'s structure is not reproducible by `config import` alone. It is
  reproduced by re-running the Phase 4 documentation refresh, which is idempotent
  and which has to run anyway — the pages themselves are generated from the kernel
  repository and could never have been config either.
- **Registration was enabled in this phase, not Phase 6.** One config variable,
  to close a 404 the site's own login page linked to. Phase 6 still owns what
  registration then does: verification posture, rate limits, and what a new
  account may post.
- **Author pages are a page, not a query.** A gather query filtered by author
  would need the author's UUID, which the installer assigns and which therefore
  differs on every install — it cannot appear in config or in a template. So
  `/authors/jeremy-andrews` is a page item and the blog byline links to it by path.
- **The blog archive groups by year in the template.** Gather has no `GROUP BY`
  and no aggregates. The rows arrive newest-first, so the template emits a heading
  when the year changes — a presentation decision made where presentation
  decisions belong.

### Deferred

- Page copy is `PLACEHOLDER-PHASE-5` on `/get-started`, `/learn`, `/rust`,
  `/community`, `/accessibility`, `/comment-policy`, `/authors/jeremy-andrews`
  and `/why`. Gate 5 greps for that marker.


## Gate 4 — documentation import and the machine-readable surface

**Attempt 1 — 2026-08-24 — PASS (2 fix loops)**

| Check | Result |
|---|---|
| All documentation pages render with highlighting | PASS — 36 pages; `bash`, `rust`, `json`, `sql`, `toml` and `yaml` all tokenised, an unknown language renders as plain code |
| Internal links between documentation pages resolve | PASS — a crawl of 181 pages found zero 404s; 62 links were rewritten to site paths and 54 left pointing at GitHub, and the crawl followed every one of the 62 |
| Raw-markdown URLs serve | PASS — `/learn/raw/{slug}` returns `text/markdown; charset=utf-8` for all 36 |
| The refresh job re-run produces no spurious diff | PASS — `docs-import --check` reports 0 files written, 0 stale; `git status config/docs` is clean after a second run |
| llms.txt serves and its links resolve | PASS — `text/plain; charset=utf-8`, 59 lines, 79 site URLs, all reached by the crawl |
| JSON-LD still emitted | PASS — the SEO plugin's `Article` block is unchanged on item pages |
| Config round-trip, including the generated set | PASS — 51 hand-written and 72 generated entities, every declared key unchanged |

### The two fix loops

1. **Two queries had a literal backslash where a line continuation belonged.**
   Postgres answered `syntax error at or near "\"`, the tap logged and returned
   nothing, and `/llms.txt` rendered "The documentation has not been imported
   yet" over a database holding 36 documents. Silent, and it looked like an empty
   result rather than a broken query. There is now a test asserting no query
   string contains a backslash or a newline.

2. **One paragraph pushed the page sideways at 360px.** A mirrored tutorial had a
   long unbreakable token in prose — an inline `code` holding a path — measuring
   455px inside a 328px column. `overflow-wrap: break-word` on the body and
   `anywhere` on inline code; code blocks keep `normal` and scroll inside their
   own box, so the source stays copyable.

### How the mirror works, and why

The mirror is a native Rust binary, `tools/src/docs_import.rs`, run by CI and by
hand. It fetches each document from `raw.githubusercontent.com` at the pinned tag,
renders it, and writes items and aliases into `config/docs/`, which is committed.
A deploy imports them like any other config. Production therefore depends on
nothing that runs on this machine, and `--check` in CI fails the build if the
mirror has drifted from the tag.

Three designs were tried and abandoned first, each for a concrete reason:

- **A `tap_cron` job in the site plugin.** The natural answer, and it needs to
  write items. The `item-api` host interface declares `get-item` and `save-item`,
  but the SDK ships no bindings for either and no plugin in the kernel tree uses
  them, so this would have meant hand-writing the ABI against a documented but
  unexercised surface. *Trovato-side finding.*
- **Rendering markdown in a template with the kernel's `markdown` filter.** The
  filter runs pulldown-cmark and then `ammonia::clean` with ammonia's defaults,
  which strip `class` from `<code>` and from every `<span>`. Verified directly
  against ammonia 4: `<pre><code class="language-rust">` comes back as
  `<pre><code>`. So the filter cannot produce highlighted code and cannot even
  preserve the language hint a client-side highlighter would need.
  *Trovato-side finding.*
- **Storing pre-rendered HTML in an item body.** `full_html` on the item render
  path is downgraded to `plain_text` and escaped; `filtered_html` strips the
  classes. And a field holding raw markdown cannot simply be ignored, because
  `routes/item.rs` renders *every* field, a bare string included, as
  `<div class="field"><strong>label</strong>: …`.

What works is that a template receives `item` and can read `item.fields` directly.
`elements/item--docs.html` ignores `children` entirely and renders
`item.fields.html | safe`. The consequence, worth knowing: a `tap_item_view`
output would not appear on a documentation page either.

Highlighting is syntect used as a lexer with its themes discarded —
`ClassedHTMLGenerator` emits `tok-`-prefixed scope classes and `static/css/code.css`
colors them from the site's tokens. So code follows the reader's color scheme, and
the seven code colors in each scheme are checked by the same `cargo test` as the
rest of the palette. A syntect theme baked into the HTML could do neither.

### Findings

- **`mime_from_path` has no case for `.md`, `.txt`, `.xml` or `.webp`.** They are
  served as `application/octet-stream`, which a browser downloads rather than
  opens. So a site cannot serve markdown, plain text or a feed from `static/` with
  a usable content type. *Trovato-side.* Both the raw-markdown URLs and `llms.txt`
  are plugin routes for this reason, not static files.
- **`item-api` has no SDK binding.** `get-item` and `save-item` are documented in
  `crates/plugin-sdk/src/host_errors.rs` and registered by
  `crates/kernel/src/host/item.rs`, and nothing in the SDK or in any shipped
  plugin calls them. *Trovato-side.*
- **The rate limiter makes the site hard to verify.** A 181-page crawl and a
  36-screenshot pass both tripped the 100-per-minute limit repeatedly, because
  static assets count. The crawler now fetches documents only and aborts every
  subresource, and both it and the screenshot pass honour `retry-after`. That is a
  reasonable thing for a crawler to do anyway; it is a workaround here.

### Decision reversed

**`/learn` does not use `trovato_book`.** Phase 3 chose it for the ordered
hierarchy. Once the documentation became generated items whose order is known at
generation time, the book bought nothing: the generator writes each page's
previous, next and up links into its own fields, so navigation is static, needs no
query, and — unlike `book_page` — round-trips through `config export`. The
hierarchy trovato_book would have provided is one level deep here, which is a list.


## Gate 5 — copy

**Attempt 1 — 2026-08-24 — PASS (1 fix loop)**

| Check | Result |
|---|---|
| Every page has real copy | PASS — nine pages written: the front page hero and its two proof blocks, `/why`, `/get-started`, `/rust`, `/community`, `/accessibility`, `/comment-policy`, `/authors/jeremy-andrews`, and `/search` |
| Zero lorem, zero TODO markers | PASS — a test over the whole config set fails on `PLACEHOLDER`, `TODO`, `Lorem ipsum`, `FIXME` or `XXX` in any reader-visible copy |
| Claims audit: superlatives | PASS — a grep across every page for `blazingly`, `enterprise-grade`, `batteries included`, `lightning`, `10x`, `world-class`, `cutting-edge`, `best-in-class`, `revolutionary` and `seamless` returns nothing |
| Claims audit: numbers | PASS — every number in the copy enumerated and traced. Release versions and dependency versions read from the kernel's `Cargo.toml`; PostgreSQL 16 and Redis 7 from its INSTALL; ports 3000 and 3001 from its README; 360 and 1280 are the widths the screenshot pass actually uses; WCAG 2.2 is the target named. No performance number appears anywhere |
| Register spot-check | PASS — read next to the kernel's README and CONTRIBUTING; first person where it is Jeremy's history, plain declaratives, and the honesty sections kept |
| Crawl still clean | PASS — 183 pages, no 404s, no raw dumps |

### The fix loop

**`/why` served a placeholder while its copy sat at a URL nothing linked to.**
The copy was written into a new item rather than into the item the `/why` alias
points at, so the site had two pages titled "Why Trovato": the old one with the
alias and the new one with the words. Every file involved was valid; the set was
wrong, and no per-file check could have seen it.

`checks/tests/content.rs` now holds six checks over the set as a whole: every
alias points at an item that exists, no item is orphaned (an item with no alias
is still reachable at `/item/{uuid}` and still appears in the sitemap), no two
items share a title, no two aliases share a path, no placeholder survives, and no
published prose carries dash punctuation.

### Decisions

- **No em dashes, en dashes or double hyphens in published copy.** They read as
  machine writing, and this site is written in one person's voice. Two exemptions,
  both in the test: a mirrored document is somebody else's prose reproduced, and
  repunctuating it would make it a paraphrase; and a YAML comment is written for
  whoever edits the file, never served, and follows the kernel repository's own
  style, which uses em dashes freely (96 of them in its CHANGELOG). A double
  hyphen inside `<code>` or `<pre>` is a command-line flag and is exempt for that
  reason, which the test implements by blanking code spans before it looks.
- **Search is called search.** The widget's class names carry a product name that
  belongs to Tag1; none of it reaches anything a reader sees. Whether the site
  should use that name in its copy is Jeremy's to decide and is in the completion
  report as a gate, not assumed either way.
- **The `/search` page says what actually leaves the browser.** Keyword search
  runs locally in WebAssembly against a static index and sends nothing anywhere.
  Without JavaScript the form submits to a server-side full-text query. The two AI
  stages run on the server and send the query and the retrieved snippets to the
  configured provider. The page says all three separately, and says that this site
  has no provider configured today, so those two stages do not run.
- **Code samples in CMS-authored copy are not syntax-highlighted.** `filtered_html`
  allows `pre` and `code` and strips every class, so a token span cannot survive.
  The mirrored documentation is highlighted because it is generated and rendered
  by the site's own template. The two look slightly different, and the difference
  is real rather than an oversight.

### Findings

- **An unmeasured claim in the plan turned out to be wrong.** The plan said Clay
  on Cream passes for large text and buttons only. Measured, it is 4.95:1, which
  clears AA for body text. Recorded at Gate 2, and worth repeating here: the
  claims audit is not only about the copy.


## Gate 6 — community: accounts, comments, moderation

**Attempt 1 — 2026-08-24 — PASS (2 fix loops)**

| Check | Result |
|---|---|
| A fresh registered account can comment | PASS — registered through the form, logged in, and the comment form is offered; an anonymous visitor gets the log-in prompt instead |
| The comment is held pending | PASS — stored at status 2, and the author is told it is waiting |
| Invisible to anonymous | PASS — the marker does not appear on the page for a visitor with no session |
| Visible in the admin queue | PASS — `/admin/content/comments` lists it with Approve and Spam controls |
| Approval publishes it into the rendered thread | PASS — status 1, and an anonymous visitor then sees it with its author's name |
| The fail-closed test passes | PASS — `./scripts/check-moderation.sh`; and it exits 1 when the guarantee is broken, verified by flipping `comment_default_status` to `published` and watching it catch that |
| Rate limits on registration | PASS — measured: the fourth registration from one IP returns 429 with `retry-after: 3600` |
| Rate limits on comment POSTs | PASS — measured: four comments succeed, the fifth returns 429 |

### How the moderation pipeline is put together

Everything except one template already shipped in the kernel image, so this phase
enabled and configured rather than built:

- `trovato_comments` provides the threaded comments, the admin queue and the
  permissions.
- `trovato_spam` provides the pipeline the plan describes, tap for tap:
  `tap_comment_insert` pushes a classification job, `tap_cron` drains it, and
  `tap_queue_worker` runs under the background principal
  (`ai_background = true`, `host_interfaces = ["logging", "queue", "ai-api", "db"]`,
  `db_tables = ["comment"]`, no `raw_sql`) and writes the verdict back.
- The trust ladder is the kernel's, with the threshold the plan asked for as its
  default: `comment_trust_threshold` is 3, and the classifier still runs on a
  trusted account's comments and can unpublish one afterwards.

What the site added: `comment_default_status: pending`, the trust threshold, the
`post comments` and `edit own comments` permissions on the registered-user role,
and `elements/comments.html`, which the kernel resolves for every item page and
does not ship.

### Fail closed, demonstrated rather than asserted

This stack has no AI provider configured, which is the failure the policy is about.
Observed: the job is queued, the queue worker traps, `plugin_queue` records
`tap_queue_worker failed (trap or error result)`, and the comment stays at status
2. Nothing publishes.

`scripts/check-moderation.sh` performs that sequence and asserts it: register (or
reuse), log in, post, drain the queue, then check three things — the comment was
stored, it is not published, and an anonymous visitor cannot see it. It was then
verified to fail: with `comment_default_status` flipped to `published` it exits 1
with "the comment was PUBLISHED after the classifier failed", and with the value
restored it exits 0.

### The two fix loops

1. **A role file without `created` takes the whole import down.** `ConfigItem` and
   the other entities give `created` a serde default; the role deserializer does
   not. The error is `missing field 'created'`, and because import validates the
   whole set before writing anything, one role file failed 54 entities.
   *Trovato-side.*

2. **The round-trip check failed on a permission list that had only been
   reordered.** The exporter writes permissions sorted; a hand-written file lists
   them in whatever order made sense. A role's permissions are a set, so the
   checker now compares that one key order-insensitively. Everything else keeps
   its order, because a gather query's `sorts` and `filters` do not mean the same
   thing rearranged.

### Findings

- **Email verification cannot be completed without SMTP, by design.** Registration
  creates a blocked account and logs `SMTP not configured; verification email not
  sent`. The database stores only the token's *hash*, so there is no way to
  recover the link from the server: the plaintext existed only in the email. That
  is correct, and it means the account activation in `check-moderation.sh` is the
  one step in that script a visitor could not perform. Real SMTP credentials are
  a Jeremy gate.
- **No AI provider is configured, so classification never returns a verdict.** The
  pipeline is enabled and inert, which is the state the plan asked for. Which
  provider, at what token budget, is a Jeremy gate. Until it is answered, every
  comment from an unproven account waits for a person, and the site says so on
  `/comment-policy`.
- **A registration attempt is rate-limited per IP at three an hour.** Reasonable
  for a public site and awkward for a check that registers: `check-moderation.sh`
  reuses one fixed account for that reason, and clears its comments first so the
  trust ladder cannot make the check pass for the wrong reason.


## Gate 7 — accessibility and performance posture

**Attempt 1 — 2026-08-25 — PASS (3 fix loops)**

| Check | Result |
|---|---|
| An axe job runs in CI against the compose site and passes on the full page set | PASS — a `site` job stands the stack up with `run.sh --fresh --proxy` and runs the crawl, axe, the render pass, the moderation check and the config round-trip; `npm run a11y` reports no violations on 18 page renders |
| Every template family covered | PASS — front page, content page, both listings, documentation index, documentation page, contact form, search, log-in; each in both colour schemes |
| Cache-control on anonymous pages, confirmed by header inspection | PASS — `public, max-age=60, stale-while-revalidate=600` with `Vary: Cookie`; a request carrying a session gets `private, no-store`; documents get an hour; the site's static files an hour and its fonts a week |
| Every claim on /accessibility is one this run verified | PASS — the page was rewritten against what the build now checks, and what it cannot check is listed as such |

### What axe found, and what was wrong

Sixteen violations on the first run, five distinct causes, all real:

1. **`link-name`, on nine page renders.** The site name was the alternative text
   of an image, and the branding link holds two images: one for each colour
   scheme, with the other hidden by CSS. In dark mode the stylesheet hid the only
   image that had a name, so the link had no accessible text at all. Fixed by
   putting the text in the link and making both images decorative. A name that
   depends on which stylesheet rule won is not a name.
2. **`empty-heading`, on the front page in dark mode.** The same mistake in the
   `<h1>`, which is the wordmark. Same fix.
3. **`page-has-heading-one`, on the contact page.** A themed plugin response hands
   the kernel a title and a body, and `render_page` puts the title in the context
   and leaves it to the theme. The kernel's own `page.html` does not render it
   either, so *every* themed plugin page on a stock install ships without a
   level-one heading. Fixed with `templates/page--contact.html`, which is what the
   theme engine resolves for that path. *Trovato-side finding.*
4. **`scrollable-region-focusable`, on documentation pages.** A code block that
   scrolls sideways could not be scrolled without a mouse. Fixed with
   `tabindex="0"` and a name.
5. **`landmark-unique`, twice.** Two search landmarks on the search page with no
   names, and then, after the fix above, a hundred code blocks each marked
   `role="region"` with the same name. Fixed by labelling the two search forms and
   by dropping `role="region"`: a scrollable region has to be focusable, it does
   not have to be a landmark, and a page with a hundred landmarks does not have a
   landmark list.

### The front proxy

Three things the kernel cannot do for itself, so the proxy does them, and it is
the same `deploy/Caddyfile` locally and in production with two environment
variables changed:

- **It serves the site's own `/static`.** Measured: 150 consecutive requests for a
  stylesheet through the proxy, all 200. Direct to the kernel the 101st is a 429.
  Anything the site does not carry falls through to the kernel, which is where the
  image's own css and js live.
- **It sets Cache-Control on HTML**, which the kernel sets on static files and on
  nothing else.
- **It maps `/sitemap.xml`** onto the one the site generates, since the kernel owns
  that route and emits relative `<loc>` values.

### The three fix loops

1. **The moderation check could not run after the other checks.** Its first
   request is a GET, and a crawl and a screenshot pass had already spent the
   hundred-a-minute budget. It now retries a 429 the way the header asks, as the
   crawler and the screenshot pass already did. Its first retry helper also
   swallowed the response body by owning `-o`, which cost three registration
   attempts against a three-an-hour limit before that was noticed.
2. **The proxy shared one rate-limit bucket with everybody.** Behind a proxy every
   request arrives from the proxy, so the whole site was one bucket of 100 a
   minute. Fixed with `TRUSTED_PROXIES` naming the proxy, which needs a fixed
   address, which needs a fixed subnet: all three are now in the compose files.
3. **A wrong conclusion, corrected.** The first reading of the evidence was that
   the kernel honours `X-Forwarded-For` from an untrusted peer, which would have
   been a security defect worth reporting. It does not. Requests reaching a
   container through a published port arrive from the network gateway rather than
   from `127.0.0.1`, so the peer really was trusted; and Caddy replaces a client's
   `X-Forwarded-For` with the address it saw, so the spoofed headers in the test
   were correctly discarded. Both halves behave as documented. What the test was
   measuring was one client with three names.

### Findings

- **A themed plugin page has no level-one heading.** See above. *Trovato-side.*
- **The contact form does not associate its errors with its fields.** A failed
  submission renders a `<ul class="contact-errors">` above the form; the fields
  carry no `aria-invalid` and no `aria-describedby`, and the list has no
  `role="alert"`, so a screen reader is told there is a problem and not which
  field has it. The markup belongs to `trovato_contact` and this site does not
  patch the kernel. *Trovato-side; scoped out and stated on /accessibility.*
- **The kernel measures request time and never sends it.**
  `middleware::query_profiler::track_request_timing` sets a `Server-Timing` header
  and logs slow requests, it is exported from `middleware::mod`, and nothing
  applies it to the router. Verified: no `Server-Timing` header is sent, on any
  response. *Trovato-side.*
- **A render-time footer is therefore not possible.** The template context carries
  fifteen keys and none of them is a duration, and the header that would carry one
  is not sent. Where that information belongs for an operator is the proxy's
  access log, which has it. No performance number appears anywhere in the copy.
- **`TRUSTED_PROXIES` takes addresses, not ranges.** Fine for a fixed proxy;
  awkward for anything autoscaled, and worth knowing before the hosting decision.
  *Trovato-side.*

### For the deployment

**The kernel's port must not be published in production.** Locally it is bound to
127.0.0.1 so that `run.sh` can drive the installer and the checks can reach it.
In production only the proxy listens, because a caller that can reach the kernel
directly is a caller whose forwarded address the kernel has no reason to doubt.
docs/DEPLOY.md carries this.


## Gate 8 — deployment for trovato.rs

**Attempt 1 — 2026-08-25 — PASS (2 fix loops)**

| Check | Result |
|---|---|
| The production profile boots locally end to end through the proxy | PASS — `./scripts/check-production.sh`: every page 200 over TLS, HSTS set, anonymous HTML cacheable for a minute, a page with a session marked private, fonts served by the proxy for a week, `/sitemap.xml` absolute, www redirecting 301 to the apex, plain HTTP redirecting to HTTPS |
| Only the proxy publishes a port | PASS — asserted against the merged production configuration, not against a running container |
| Every env knob is in the example file | PASS — 10 values marked `CHANGE ME`, every `${…}` the production compose files read is set |
| Every env knob is referenced in DEPLOY.md | PASS — a settings table naming each one, what it does, and what goes wrong when it is not set |
| A cold reader could deploy from the doc alone | PASS — see below |

### What the adversarial read found

Read back as somebody who has this repository and nothing else:

- **Nine commands were not copy-pasteable.** They said `docker compose ... exec
  -T postgres …`, with a literal ellipsis where two `-f` flags and an
  `--env-file` belonged. Replaced by defining a `dc` shell function once at the
  top and using it everywhere, which is shorter to read as well as runnable.
- **"This needs a Rust toolchain on the host" did not say how to get one.** Now
  it carries the rustup line, and says that `rust-toolchain.toml` pins the version
  and the wasm target so nothing else has to be chosen.
- **Fifteen settings were in the example file and mentioned nowhere in the
  procedure.** Now there is a table of every one.
- **Two counts were wrong.** "Four images" is five. The documentation set is 72
  files, which the doc now says, because an operator watching a 72-file import
  should be able to tell it finished.

### The two fix loops

1. **The check declared the kernel's port published when it was not.** It asked
   `docker compose port site 3000` without the production override files, so it
   was asking about the development stack. Now it reads the merged configuration
   and asserts that `proxy` is the only service with any published port at all.
2. **The check called a 502 healthy.** Its readiness loop accepted any response,
   and the proxy answers 502 the instant it is up and the kernel is restarting
   behind it. So it declared the site up a second after a restart and then
   reported every page as broken. It now waits for a 200.

### How production differs from local, and why each difference exists

`docker-compose.production.yml` is an override of the same base file the local
stack uses, so what runs in production is what was tested plus these differences
and no others:

- **Nothing but the proxy publishes a port.** The kernel believes
  `X-Forwarded-For` from an address in `TRUSTED_PROXIES`, and the proxy is in that
  list; a caller able to reach the kernel directly could claim to be any client
  and mint rate-limit buckets without limit. `check-production.sh` asserts it.
- **`restart: always`** rather than `unless-stopped`.
- **A real hostname and a real certificate.** `deploy/Caddyfile.production` adds
  the hostname, HSTS and the www redirect; everything about how the site is served
  is in `deploy/shared.caddy`, which both entry points import. One copy, so the
  two cannot drift.
- **HSTS without `preload`.** A year is a long promise and preloading is
  effectively irreversible. That is a decision to make on purpose.

The local proof runs the production files as written and changes three things a
laptop cannot have: the hostname is `localhost`, the certificate comes from
Caddy's own authority, and the ports are high ones. It runs in its own compose
project so it cannot disturb anything else, and it tears itself down.

### Findings

- **The site plugin has to be built before deploying, and that needs Rust on the
  host.** There is no published artifact for it, because there is no plugin
  registry and no package format — the same gap `/why` describes. The document
  says the build can happen anywhere and the two-file `overlay/` copied across.
- **First boot is not fully scriptable, on purpose.** The installer sets the
  administrator's password, and a password that came from a file in a public
  repository is a password everybody has. It is the one browser step.


## Gate 9 — final review, translations, and the report

**Attempt 1 — 2026-08-25 — PASS (4 fix loops)**

| Check | Result |
|---|---|
| Copy reconciled against what shipped | PASS — `/comment-policy` rewritten: it described a classifier that runs and a provider that receives text, and neither happens today; `/accessibility` was already reconciled in Phase 7 |
| Italian translations of the core pages | PARTIAL — four of five. `/it/why`, `/it/get-started`, `/it/community`, `/it/accessibility` all serve translated titles and bodies. The front page cannot be translated on this kernel; see below |
| Full-site crawl, both languages | PASS — 191 pages from `/`, `/llms.txt` and the Italian entry point: no 404s, no 5xx, no raw dumps, every page themed and navigable |
| Clean-room rebuild | PASS — every container and volume destroyed, then `./scripts/run.sh --fresh --proxy`: 24 seconds to a populated site, and every gate re-run against it |
| The scratch Trovato clone is clean | PASS — `git status --porcelain` empty, still at `v0.101.0`. No file in the Trovato tree was modified at any point |
| Launch checklist committed | PASS — `docs/LAUNCH.md`, 18 ticked with the command that checks each one, 16 unticked and attributed |
| Report written and pushed | PASS — this ledger, and the completion report |

### Translations: what works, and the three things that do not

The Italian pages work. What they cost was finding out that three separate parts
of the translation story are missing on v0.101.0, each verified rather than
inferred:

1. **Nothing can write a translation.** The kernel reads `item_translation` and
   overlays it correctly. Nothing populates it. `config import` stores a
   per-language field map verbatim in the item's own `fields`, where the item
   renderer looks for a `value` key, finds a language code, and renders the field
   as nothing *in every language* — observed: putting the tutorial's own
   `{en: …, it: …}` shape into a page blanked the English page too. The two admin
   routes are registered `GET` only, and both 500 because their templates are not
   in the image. There is no API endpoint. *Trovato-side.*

   What the site does: the translations live in a config variable and the site
   plugin's `tap_cron` copies them into `item_translation`, writing only what
   changed and removing what configuration no longer declares. That is the one
   kernel table this plugin writes and it is declared in `db_tables`.

2. **A plugin cannot read a variable outside its own namespace.**
   `variables_get` prefixes every key with `plugin.{plugin_name}.` before looking
   it up, so `config/variable.site_base_url.yml` was a file nothing ever read. It
   fails silently: the call returns the default that was passed in, and the
   default happened to be the right answer, so the sitemap was correct for four
   phases by luck. Both variables are now named `plugin.trovato_site.…`.
   *Trovato-side, and the thing worth knowing is the silence.*

3. **The front page cannot be translated at all.** `routes/front.rs` renders the
   configured front-page item without calling `apply_translation_overlay`, which
   `routes/item.rs` does call. So a front page is always in the default language
   however the reader asked. The Italian front page was written and then removed
   from configuration rather than left as data nothing reads. *Trovato-side.*

### Two more, found while trying to link the languages together

- **`<html lang>` is always the default language.** `routes/item.rs` inserts the
  resolved `active_language` into the context and then calls
  `inject_site_context`, which overwrites it with the site default. The comment
  there says "route handlers may override active_language"; the handler sets it
  first and the helper clobbers it. Observed: an Italian page whose body is
  Italian declares itself `lang="en"`. `text_direction` is clobbered the same way,
  which would matter more for a right-to-left language than it does here.
  *Trovato-side.* The consequence for a theme is total: no template can render
  anything conditional on the language, which is why the Italian pages link to
  each other in their own copy rather than through a footer block.
- **`hreflang` alternates cannot be built.** The kernel has
  `build_hreflang_links`, which produces exactly them, and nothing calls it: it is
  reachable only from its own tests, so `hreflang_links` is never defined.
  Building them in a template needs the page's reader-facing path, and for an item
  page the context does not carry one — `routes/item.rs` resolves the alias into a
  local `canonical_path`, uses it for the absolute `page_meta.canonical`, and
  inserts only the absolute form. Emitting alternates pointing at `/item/{uuid}`
  would tell a crawler that a page's canonical URL and its alternates are
  different addresses, which is worse than emitting none. *Trovato-side; the site
  emits none and says why in the template.*

### The four fix loops

1. **A per-language field map blanked the page in both languages.** The shape the
   kernel's own tutorial uses in config is not the shape the item renderer reads.
2. **The translation sync read an empty variable and did nothing, silently.** The
   namespacing above. It took adding a log line saying how many bytes it had read
   to see it: "read 2 bytes", which is `{}`, the default.
3. **`tap_cron` with the wrong signature runs and does nothing.** The first
   version was `fn tap_cron() -> String`. It compiled, exported, was dispatched,
   and the kernel logged `tap_cron completed` — with the body never running. Every
   cron tap in the kernel tree is `fn tap_cron(input: CronInput) -> Value`.
4. **Two template edits served a raw field dump.** Both were unbalanced Tera after
   a scripted replacement, and both were caught by size: a page that should be
   12 KB came back as 5 KB. This is the failure mode the crawler was built for in
   Phase 3 and it is still the easiest one to cause.

### The clean-room rebuild

```
docker compose -p trovato-site -f docker-compose.yml -f docker-compose.proxy.yml \
    down -v --remove-orphans
rm -rf overlay
./scripts/run.sh --fresh --proxy
```

24 seconds. Then, against that build: 8 test suites green, 191 pages crawled
clean, axe with no violations on 18 renders, 36 screenshots with no overflow and
nothing off-origin, moderation failing closed, 126 config entities round-tripping
unchanged, and the documentation mirror matching the pinned tag.

`run.sh` now runs cron once before it finishes, so the translations are in place
when the command returns rather than up to a minute later.
