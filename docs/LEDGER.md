# Build ledger

**Status:** Phase 5 complete, Phase 6 next
**Last updated:** 2026-08-24
**Where things stand:** Every page carries real copy. No placeholders, no unmeasured numbers,
no marketing vocabulary. A crawl of 183 pages is clean. Gates 0 through 5 passed. Community,
comments and moderation are next.

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
