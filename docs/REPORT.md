# Completion report

The official Trovato website, built end to end on a released kernel. Nine phases,
nine gates, all passed, and then moved one release forward. Nothing in the
Trovato tree was modified at any point: the clone this run read from is still
`git status` clean, and every fact about the kernel in this document comes from a
tag rather than from a working tree.

**Repository:** https://github.com/jeremyandrews/trovato-site
**Kernel:** `ghcr.io/jeremyandrews/trovato:0.102.0`, pinned, never `latest`
**Target:** https://trovato.rs

**The pin, which moves as one triple:**

| What | Value |
|---|---|
| SDK revision in `Cargo.toml` | `20baa121810b5c656b3f80028335770069fab5e0` |
| The tag that commit is | `v0.102.0` |
| Kernel image tag (`TROVATO_VERSION`) | `0.102.0` |
| `api_version` in the plugin manifest | `0.102` |

The plugin's own source did not change to build against it. The SDK moved from
0.101.0 to 0.102.0, the manifest's `api_version` moved with it, and
`cargo build --release --target wasm32-wasip1` compiled in 2.04 seconds with
nothing to fix, which is what a frozen plugin contract is supposed to feel like.

The mirrored documentation is deliberately not part of that triple. It is pinned
separately, by `TAG` in `tools/src/docs_import.rs`, and it is still `v0.101.0`:
re-mirroring regenerates 72 files and is its own reviewable change, which is the
order `docs/DEPLOY.md` sets out. CI checks the mirror against its own tag, so the
two are consistent; what is out of step is the documentation a reader sees on
`/learn` against the kernel this site runs.

## The two commands

```
./scripts/run.sh --fresh --proxy
```

From nothing to a populated, themed, working site on `http://127.0.0.1:3080`, and
through the front proxy on `:8081`. Measured from destroyed volumes and a removed
`overlay/`: **22.5 seconds on 0.102.0**, against **24.7 seconds on 0.101.0**
measured the same way on the same machine minutes earlier. Two seconds is noise
rather than a result, and the number worth having is that the bump cost nothing.
It builds the plugin, brings up PostgreSQL, Redis, the kernel and the cron poker,
runs the installer, enables the plugins, imports the configuration and the
mirrored documentation, restarts so both are live, and runs cron once so the
translations are in place before it returns.

```
./scripts/check-production.sh
```

The production compose files as written, with a certificate from Caddy's own
authority standing in for a public one. It asserts TLS, HSTS, the caching policy,
the sitemap, the www redirect, the HTTP-to-HTTPS redirect, and that no service
except the proxy publishes a port. For a real host, `docs/DEPLOY.md`.

## The gates

| Gate | Result | Fix loops | What it cost |
|---|---|---|---|
| 0 — baseline and preflight | PASS | 0 | — |
| 1 — skeleton | PASS | 2 | A uuid/text comparison that Postgres refuses outright; imported config not being live until the server reads it again |
| 2 — theme and brand | PASS | 3 | A nested Tera block that drops its siblings; `full_html` downgraded to escaped text; two buttons left at browser and kernel defaults |
| 3 — content architecture | PASS | 2 | A Tera object literal that does not exist, served as a raw dump; two broken links on the login page |
| 4 — documentation import | PASS | 2 | Two queries with a literal backslash where a continuation belonged; one paragraph pushing the page sideways at 360px |
| 5 — copy | PASS | 1 | A page's copy written into a new item instead of the one the alias points at |
| 6 — community and moderation | PASS | 2 | A role file without `created` taking the whole import down; a permission list that had only been reordered |
| 7 — accessibility and performance | PASS | 3 | A check that could not run after the other checks; a proxy sharing one rate-limit bucket with everybody; a wrong conclusion about a security defect, corrected |
| 8 — deployment | PASS | 2 | A check asking the wrong compose files; a check calling a 502 healthy |
| 9 — final review and translations | PASS | 4 | A field map that blanked the page in both languages; a variable read silently returning its default; a `tap_cron` with the wrong signature running and doing nothing; two unbalanced templates served as raw dumps |
| 11 — the kernel bump to 0.102.0 | PASS | 1 | An `hreflang` alternate and a canonical naming `/item/{uuid}` on every language-prefixed page, because the site had never declared its aliases in the second language |

Gate 10, the visual rework, is on its own branch and is not merged here, which is
why the numbering steps over it. Full detail, including what each loop turned out
to be, is in `docs/LEDGER.md`.

## The final crawl

188 pages, from `/`, from `/llms.txt`, and from `/it/`. No 404s, no 5xx, no
template-failure dumps, every page carrying its main landmark, its stylesheet, its
skip link and its main navigation.

It was 191 on 0.101.0, and the three are an improvement rather than a loss. Four
of them were `/user/login?destination=/item/{uuid}` addresses, one per translated
page, produced because the comment form on a language-prefixed page was handed
the uuid path as the place to come back to; they collapse onto the four English
aliases that were already in the set. Against that, `/it/` is now a page, where on
0.101.0 a bare language prefix was a 404. Fewer addresses, more site.

**The site** — `/`, `/why`, `/get-started`, `/rust`, `/community`,
`/accessibility`, `/comment-policy`, `/authors/jeremy-andrews`, `/search`,
`/contact`.

**Listings and posts** — `/news` with three posts, `/blog` with one,
`/blog/archive`, and each post at its own alias.

**Documentation** — `/learn` and 36 mirrored pages: the readme, the installation
guide, building a first site, developing with Docker, nine tutorial parts, five
plugin references, twelve design documents, the coding standards, the roadmap,
contributing, the code of conduct, and the known issues.

**Italian** — `/it/`, `/it/why`, `/it/get-started`, `/it/community`,
`/it/accessibility`. Five pages, each declaring `<html lang="it">`, each naming
its English counterpart in an `hreflang` alternate and in a switcher a reader can
click, and each reached through a menu whose translated entries carry the `/it`
prefix. On 0.101.0 the same five were four, `lang="en"`, with no alternates and a
paragraph of hand-written links at the bottom of every body.

**Documents** — `/rss/blog.xml`, `/rss/news.xml`, `/llms.txt`, `/robots.txt`,
`/sitemap/pages.xml` (served as `/sitemap.xml` through the proxy), and a
markdown source URL for each of the 36 documentation pages.

**Accounts** — `/user/login`, `/user/register`, `/user/recover`.

## The plugins

One built here. Eight enabled from the kernel image, because the site enables
what exists rather than rebuilding it:

| Plugin | Why |
|---|---|
| `trovato_site` | **This repository.** The `news`, `docs` and `front_page` content types, the front page's live listing, the sitemap, the raw-markdown routes, `llms.txt`, and the translation sync |
| `trovato_blog` | The blog content type and its listing |
| `trovato_seo` | Head metadata and JSON-LD |
| `trovato_search` | The three-tier search |
| `trovato_contact` | The contact form |
| `trovato_scheduled_publishing` | Publishing on a date |
| `trovato_redirects` | Redirect management |
| `trovato_comments` | Threaded comments, the admin queue, the permissions |
| `trovato_spam` | AI comment moderation, enabled and inert without a provider |

Seven more are enabled by the kernel's own defaults and the site does not use
them directly: `trovato_audit_log`, `trovato_categories`,
`trovato_config_translation`, `trovato_content_locking`,
`trovato_content_translation`, `trovato_image_styles`, `trovato_locale`,
`trovato_media`, `trovato_oauth2`, `trovato_webhooks`.

## What was found

Twenty-two things, found against v0.101.0. Each one now carries its status
against the release this repository actually runs, so the list stops describing a
release that has moved:

| Status | Count | Which |
|---|---|---|
| **Fixed at 0.102.0** | 3 | 4, 5, 18 |
| **Open** | 18 | 1, 2, 3, 6, 7, 8, 9, 10, 11, 12, 13, 15, 16, 17, 19, 20, 21, 22 |
| **Could not reproduce** | 1 | 14 |

Every status was re-derived from the kernel source at tag `v0.102.0` and, for the
three that moved, from the pages in a browser. Three of the original findings
were themselves partly wrong, and the correction is recorded with the finding
rather than quietly applied. What the bump revealed that was not on this list at
all is under "What the bump revealed".

### Trovato-side, and load-bearing for anybody deploying

1. **Open.** **Static assets are rate-limited as API calls at 100/minute per IP,
   and the limits are not configurable.** Measured: the 101st request for a
   stylesheet returns 429. A page here pulls about ten subresources, so a reader
   gets ten pages in before their fonts stop loading. *The site's answer: the
   proxy serves `/static`. 150 consecutive requests through it, all 200.*
   Unchanged at 0.102.0: `categorize_path` has no `/static` arm and the only
   `RateLimitConfig` is `::default()`. Both browser gates hit 429 while crawling
   this release, which is the finding reproducing itself.
2. **Open.** **Nothing can write a content translation.** Config import stores a
   per-language field map in the item's own fields, where the renderer finds a
   language code instead of a value and renders nothing *in every language*. The
   two admin routes are `GET`-only and their templates are not in the image. There
   is no API endpoint. *The site's answer: the plugin's `tap_cron` writes
   `item_translation` from configuration.* Re-verified at 0.102.0: both routes are
   still `get(...)` and neither template is in the image.
3. **Open.** **A plugin reading a variable outside its own namespace gets the
   default it passed in.** `variables_get` prefixes every key with
   `plugin.{name}.`. No error, no warning. The site's sitemap read a key that
   never existed for four phases and was correct only because the default was
   right. The silence is unchanged: a miss returns the caller's default and
   `warn!` is reserved for a database error.
4. **Fixed at 0.102.0.** **`<html lang>` was always the site default.**
   `routes/item.rs` inserted the resolved language and then
   `inject_site_context` overwrote it. The helper now fills the language in only
   when the context does not already carry it, and its doc comment states the
   contract: the route's value is authoritative, the site default is a fallback.
   Verified on the page: `/it/why` serves `<html lang="it">` where it served
   `lang="en"` on 0.101.0, and the theme branches on the language in two places
   that could not exist before.
5. **Fixed at 0.102.0.** **The front page was never translated.**
   `routes/front.rs` rendered the configured item without applying the
   translation overlay that `routes/item.rs` applies. It applies the overlay now,
   with the same field-access re-filter, and gives the page
   `available_translations` and `hreflang_links` of its own. The Italian front
   page was written and removed during Gate 9 because it would have been
   configuration nothing read; it is back, and `/it/` serves it.
6. **Open.** **`sitemap.xml` emits relative `<loc>` values** and lists items only.
   The protocol requires absolute URLs. *The site serves its own; the proxy maps
   the standard path onto it, because a plugin claiming `/sitemap.xml` makes axum
   panic at startup.* Unchanged at 0.102.0, both halves: no base URL is prepended,
   the query still selects from `item` only, and there is still no reserved-path
   check before the plugin router is merged.
7. **Open.** **The login page's "Forgot password?" is a dead link.** It points at
   `/user/password-reset`, registered `POST`-only; the human page is
   `/user/recover`. The template and both routes are untouched at 0.102.0, so the
   click still yields 405.
8. **Open.** **The login page's inline script is blocked by the kernel's own
   CSP.** Verified in a browser: passkey sign-in does not work on a default
   install. Still inline at 0.102.0, and `DEFAULT_CSP` is a `from_static` header
   with no nonce and no hash, so a nonce is not reachable without a code change.
9. **Open.** **`mime_from_path` has no case for `.md`, `.txt` or `.xml`.** All
   three are served as `application/octet-stream`, so a site cannot serve
   markdown, plain text or a feed from `static/`. The match arms are unchanged.
10. **Open, and one third of it was wrong.** **Config import cannot set `promote`
    or `sticky`,** so a config-defined site can never reach the kernel's own
    promoted-items front page. `ConfigItem` carries neither field and the insert
    hardcodes both to `0`. It also does not update an existing item's `created`.
    **Correction:** the original finding also claimed `changed` is not updated,
    and that is not true, at 0.102.0 or at 0.101.0: the upsert sets
    `changed = EXCLUDED.changed` from the file.
11. **Open, and the second sentence was imprecise.** **A role file without
    `created` fails validation** and, because import validates the whole set
    before writing, takes every other file down with it. **Correction:** the
    original finding said every other entity defaults that field. They do not:
    the others carry a bare `pub created: i64`, which deserializes without an
    attribute because of its type. `Role` is the outlier because its `created` is
    a `DateTime<Utc>`, which has no such free pass.

### Trovato-side, and cosmetic or internal

12. **Open.** `trovato_blog` declares `tap_item_view` in its manifest and does not
    export it, so every item view logs an ERROR. The manifest still lists the tap
    and the crate still exports four others and not that one.
13. **Open, and the count was wrong.** Plugins log a route warning at startup
    about a callback with the wrong `handler_type`. **Correction:** it is 16
    plugins and 22 menu entries, not seventeen plugins. The cause is structural
    rather than a mistake repeated sixteen times: the frozen SDK's
    `MenuDefinition` has no `handler_type` field at all, so every entry built
    through it takes the registry's default of `page`. A plugin that wants a
    dispatchable callback has to use `MenuRoute` instead, which is what the two
    plugins that do not warn use.
14. **Could not reproduce.** The finding said `elements/comments.html` is resolved
    for every item page and is not in the image, so comments render as nothing on
    a stock install. It is in the image, at `/app/templates/elements/comments.html`,
    and it is in the 0.101.0 image too, so the original finding was wrong rather
    than fixed. *The site still supplies its own, which wins the name collision
    and is the markup the theme is built around.*
15. **Open.** `admin/content-translate-list.html` and `-edit.html` are not in the
    image either, so both translation admin routes return 500. Neither template is
    in the 0.102.0 tree.
16. **Open at 0.102.0, fixed on kernel main after the tag.** A themed plugin
    response's title is put in the context and rendered by no theme, so every
    themed plugin page ships without an `<h1>`. *The site supplies
    `page--contact.html`.* The kernel commit that fixes it, "Render a themed
    plugin page's title as its heading", is 21 commits past `v0.102.0` and will
    arrive in the next release.
17. **Open.** `track_request_timing` sets a `Server-Timing` header and logs slow
    requests, and nothing applies it to the router. No render-time footer is
    possible. The function and its re-export are still the only two references to
    it in the tree.
18. **Fixed at 0.102.0.** `build_hreflang_links` produced exactly the alternates a
    translated site needs and was reachable only from its own tests. Both the item
    route and the front route call it now, over the languages a page actually
    exists in, and `base.html` emits the tags again. Verified on the page: `/why`
    and `/it/why` each carry `en`, `it` and `x-default` alternates naming
    reader-facing addresses.
19. **Open, and one clause was wrong.** `item-api` declares `get-item` and
    `save-item` and the SDK ships no bindings for either. **Correction:** the
    original finding added that no plugin in the tree uses them, and that was
    already untrue at 0.101.0: `plugins/argus` hand-rolls its own binding against
    that interface. The SDK gap is the finding; the hand-rolled binding is the
    evidence that somebody needed it.
20. **Open.** The kernel's `markdown` Tera filter sanitizes with ammonia's
    defaults, which strip `class` from every `<code>` and `<span>`, so it cannot
    produce highlighted code or even preserve a language hint. The filter still
    calls bare `ammonia::clean`, while the kernel's own content filter builds a
    configured sanitizer a few files away.
21. **Open.** Field naming is inconsistent: the kernel's `page` type uses `body`,
    `trovato_blog` uses `field_body`, and the kernel's own blog listing template
    and promoted-items renderer both read `fields.body` — so a blog teaser renders
    with a title, a date and no text. All four still read the way they did.
22. **Open, with one adjacent half fixed.** Gather cannot link to a friendly URL:
    a relationship joins on column equality and the alias join is an expression,
    `includes` matches on plain fields, and no Tera filter resolves an alias. Every
    gather listing still links to `/item/{uuid}`, the kernel's own included, and
    the 36 rows of `/learn` are the proof. What did move is the other consequence
    of the same fact: `requested_path` now carries the address as asked for, so a
    theme can light the active navigation trail on a page reached through an
    alias, which is every page here except the listings. The site's own navigation
    now reads it.

### Site-fixable later

- **The contact form does not associate its errors with its fields.** The markup
  belongs to `trovato_contact`; the site does not patch the kernel. Stated on
  `/accessibility`.
- **Code samples in hand-written pages are not syntax-highlighted,** because
  `filtered_html` strips the class off a token span. The mirrored documentation is
  highlighted because the site's own template renders it.
- ~~**`hreflang` alternates are absent.**~~ Done at 0.102.0. See 4, 18 and the
  next section: the theme has both the language and the page's reader-facing
  address now, and the site declares its aliases in both languages so the
  addresses the kernel names are the ones a reader uses.

## What the bump revealed

Seven things, none of them on the list above, all found by moving one release and
none of them fixable by editing the kernel from here. Each is phrased for the
kernel's backlog, and each is a place where 0.102.0's own new facts stop one step
short of the theme that consumes them.

1. **The canonical lookup has no default-language fallback, and its opposite
   number does.** `find_by_alias_with_context` resolves an address to an item
   with `language IN ($lang, 'en')`, and its doc comment says why: a shared alias
   should resolve for every language. `get_canonical_alias_with_context` resolves
   the other direction with `language = $lang` and no fallback. So an item
   translated into Italian and aliased only in English resolves no canonical on
   `/it/…` and falls back to `/item/{uuid}`, which then becomes the page's
   canonical link, both of its `hreflang` alternates, the address its switcher
   offers, and the address a reader is returned to after logging in to comment.
   Measured on 0.101.0 and on 0.102.0 before the site's own fix: `/it/why`
   declared `<link rel="canonical" href="https://trovato.rs/item/0193b000-…">`.
   *The site's answer, which is the site's own omission rather than a workaround:
   declare each translated page's alias in both languages. The table's unique key
   is `(alias, language, stage_id)`, which is the shape this needs, and a test
   now fails if a translation arrives without its alias.*
2. **A gather listing gets no `available_translations`.** The item route and the
   front route both build it; `routes/gather.rs` does not, and neither does a
   themed plugin page. So `/news`, `/blog`, `/learn`, `/search` and `/contact`
   carry the right `<html lang>` and no per-page switcher, in either language.
   *The site's answer: those pages offer an entry-point link to the other
   language's front page instead of a switcher.*
3. **`tap_item_view` is not told the language the page is being served in.** The
   tap receives the item, and `Item.language` is the item's own language, which is
   the default one even when the request is not; its doc comment invites a plugin
   to "read this to implement language-specific behavior", which is misleading
   for exactly this case. A plugin's rendered output therefore cannot follow the
   page. Visible as the English heading over the Italian front page's listing of
   recent posts.
4. **A translated menu link keeps its default-language label unless the label is
   the target's exact default-language title.** That rule is deliberate, and it is
   the right rule: a label somebody wrote by hand should not be replaced. What is
   missing is the other half, a way for a site to supply the translated label, so
   a hand-shortened label has no path to translation at all. Visible in the
   Italian navigation, where "Come iniziare" and "Comunità" are translated because
   the link titles happen to match, and "Why" is not because the link says "Why"
   and the page is called "Why Trovato".
5. **The comment thread is rendered with the item's default-language canonical and
   no `requested_path`.** `render_thread` builds its own context, so the "log in
   to comment" link on `/it/why` returns a reader to `/why`. `requested_path`
   exists now and is not passed to it, so a theme cannot fix this from the
   template.
6. **`trovato_seo`'s JSON-LD carries the untranslated title.** `/it/why` serves
   `<title>Perché Trovato</title>` and `og:title` in Italian, and
   `"headline":"Why Trovato"` in the same document.
7. **The site name and slogan are not translatable.** Both come from installer
   configuration and appear in the header of every page, so an Italian page
   carries an English slogan under an Italian wordmark.

## What is waiting on Jeremy

| Decision | What it unblocks |
|---|---|
| **DNS for trovato.rs** | The site being reachable. Two A records, plus AAAA if the host has IPv6 |
| **A host, somebody on call, a monthly budget** | Everything |
| **SMTP credentials** | Registration can be completed and the contact form delivers. Both are built and both do nothing without this |
| **An AI provider, model and token budget** | Comment classification returns a verdict instead of waiting for a person, and search gets its two AI stages. Both currently fail safe and the site says so |
| **A mailbox at the domain** | Where the contact form delivers to |
| **Whether search is called Scolta** | The name is Tag1's. The copy calls it search; using the name is a deliberate yes |
| **A screen-reader pass** | The one item on the launch checklist that matters most and that no machine can tick |
| **A deliberate keyboard pass** | Two places are verified; the rest is not |
| **The EPYC benchmark run** | Any performance number appearing anywhere. None does today |
| **Publishing `trovato-sdk` to crates.io** | The site pins it by git revision, which works and is what every plugin author would do |
| **The repository's home** | It is at `jeremyandrews/trovato-site`. Moving it changes one remote |
| **A public demo instance** | Post-launch; it needs a second operated deployment |

## What stands between this repository and https://trovato.rs serving it

DNS, a host, and the decisions above.

The site is built, checked, and reproducible from an empty machine in 22.5
seconds.
`docs/DEPLOY.md` is a procedure a stranger could follow. Two of the gates on that
list leave a feature switched off rather than the site broken, and the site says
which and why on the page where it matters.

The one that is not a decision is the screen-reader pass. `/accessibility` says it
has not happened. It should happen before the site is announced, and until it
does that page will keep saying so.
