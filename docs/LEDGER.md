# Build ledger

**Status:** Phase 1 complete, Phase 2 next
**Last updated:** 2026-08-24
**Where things stand:** The skeleton is up. `./scripts/run.sh` reaches a populated site on
http://127.0.0.1:3080 from nothing, in one command. Gate 1 passed. Theme work is next.

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
