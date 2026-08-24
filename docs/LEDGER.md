# Build ledger

**Status:** Phase 2 complete, Phase 3 next
**Last updated:** 2026-08-24
**Where things stand:** The site is themed in both color schemes, on self-hosted faces, with
the contrast of every token pairing enforced by `cargo test`. Gates 0, 1 and 2 passed.
Content architecture is next.

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
