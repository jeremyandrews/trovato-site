# Completion report

The official Trovato website, built end to end on a released kernel. Nine phases,
nine gates, all passed. Nothing in the Trovato tree was modified at any point:
the scratch clone the run read from is still `git status` clean at `v0.101.0`.

**Repository:** https://github.com/jeremyandrews/trovato-site
**Kernel:** `ghcr.io/jeremyandrews/trovato:0.101.0`, pinned, never `latest`
**Target:** https://trovato.rs

## The two commands

```
./scripts/run.sh --fresh --proxy
```

From nothing to a populated, themed, working site on `http://127.0.0.1:3080`, and
through the front proxy on `:8081`. Measured from destroyed volumes: **24
seconds**. It builds the plugin, brings up PostgreSQL, Redis, the kernel and the
cron poker, runs the installer, enables the plugins, imports the configuration
and the mirrored documentation, restarts so both are live, and runs cron once so
the translations are in place before it returns.

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

Full detail, including what each loop turned out to be, is in `docs/LEDGER.md`.

## The final crawl

191 pages, from `/`, from `/llms.txt`, and from the Italian entry point. No 404s,
no 5xx, no template-failure dumps, every page carrying its main landmark, its
stylesheet, its skip link and its main navigation.

**The site** — `/`, `/why`, `/get-started`, `/rust`, `/community`,
`/accessibility`, `/comment-policy`, `/authors/jeremy-andrews`, `/search`,
`/contact`.

**Listings and posts** — `/news` with three posts, `/blog` with one,
`/blog/archive`, and each post at its own alias.

**Documentation** — `/learn` and 36 mirrored pages: the readme, the installation
guide, building a first site, developing with Docker, nine tutorial parts, five
plugin references, twelve design documents, the coding standards, the roadmap,
contributing, the code of conduct, and the known issues.

**Italian** — `/it/why`, `/it/get-started`, `/it/community`, `/it/accessibility`.

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

Twenty-two things. None of them was fixed in Trovato; the site works with or
around each one, and each is recorded with the evidence in `docs/LEDGER.md`.

### Trovato-side, and load-bearing for anybody deploying

1. **Static assets are rate-limited as API calls at 100/minute per IP, and the
   limits are not configurable.** Measured: the 101st request for a stylesheet
   returns 429. A page here pulls about ten subresources, so a reader gets ten
   pages in before their fonts stop loading. *The site's answer: the proxy serves
   `/static`. 150 consecutive requests through it, all 200.*
2. **Nothing can write a content translation.** Config import stores a
   per-language field map in the item's own fields, where the renderer finds a
   language code instead of a value and renders nothing *in every language*. The
   two admin routes are `GET`-only and their templates are not in the image. There
   is no API endpoint. *The site's answer: the plugin's `tap_cron` writes
   `item_translation` from configuration.*
3. **A plugin reading a variable outside its own namespace gets the default it
   passed in.** `variables_get` prefixes every key with `plugin.{name}.`. No
   error, no warning. The site's sitemap read a key that never existed for four
   phases and was correct only because the default was right.
4. **`<html lang>` is always the site default.** `routes/item.rs` inserts the
   resolved language and then `inject_site_context` overwrites it. A theme cannot
   render anything conditional on the language. `text_direction` is clobbered the
   same way, which would matter far more for a right-to-left language.
5. **The front page is never translated.** `routes/front.rs` renders the
   configured item without applying the translation overlay that `routes/item.rs`
   applies.
6. **`sitemap.xml` emits relative `<loc>` values** and lists items only. The
   protocol requires absolute URLs. *The site serves its own; the proxy maps the
   standard path onto it, because a plugin claiming `/sitemap.xml` makes axum
   panic at startup.*
7. **The login page's "Forgot password?" is a dead link.** It points at
   `/user/password-reset`, registered `POST`-only; the human page is
   `/user/recover`.
8. **The login page's inline script is blocked by the kernel's own CSP.** Verified
   in a browser: passkey sign-in does not work on a default install.
9. **`mime_from_path` has no case for `.md`, `.txt` or `.xml`.** All three are
   served as `application/octet-stream`, so a site cannot serve markdown, plain
   text or a feed from `static/`.
10. **Config import cannot set `promote` or `sticky`,** so a config-defined site
    can never reach the kernel's own promoted-items front page. It also does not
    update an existing item's `created` or `changed`.
11. **A role file without `created` fails validation** and, because import
    validates the whole set before writing, takes every other file down with it.
    Every other entity defaults that field.

### Trovato-side, and cosmetic or internal

12. `trovato_blog` declares `tap_item_view` in its manifest and does not export
    it, so every item view logs an ERROR.
13. Seventeen plugins log a route warning at startup about a callback with the
    wrong `handler_type`.
14. `elements/comments.html` is resolved for every item page and is not in the
    image, so comments render as nothing on a stock install. *The site supplies
    it.*
15. `admin/content-translate-list.html` and `-edit.html` are not in the image
    either, so both translation admin routes return 500.
16. A themed plugin response's title is put in the context and rendered by no
    theme, so every themed plugin page ships without an `<h1>`. *The site supplies
    `page--contact.html`.*
17. `track_request_timing` sets a `Server-Timing` header and logs slow requests,
    and nothing applies it to the router. No render-time footer is possible.
18. `build_hreflang_links` produces exactly the alternates a translated site
    needs and is reachable only from its own tests.
19. `item-api` declares `get-item` and `save-item`, the SDK ships no bindings for
    either, and no plugin in the tree uses them.
20. The kernel's `markdown` Tera filter sanitizes with ammonia's defaults, which
    strip `class` from every `<code>` and `<span>`, so it cannot produce
    highlighted code or even preserve a language hint.
21. Field naming is inconsistent: the kernel's `page` type uses `body`,
    `trovato_blog` uses `field_body`, and the kernel's own blog listing template
    and promoted-items renderer both read `fields.body` — so a blog teaser renders
    with a title, a date and no text.
22. Gather cannot link to a friendly URL: a relationship joins on column equality
    and the alias join is an expression, `includes` matches on plain fields, and
    no Tera filter resolves an alias. Every gather listing links to
    `/item/{uuid}`, the kernel's own included.

### Site-fixable later

- **The contact form does not associate its errors with its fields.** The markup
  belongs to `trovato_contact`; the site does not patch the kernel. Stated on
  `/accessibility`.
- **Code samples in hand-written pages are not syntax-highlighted,** because
  `filtered_html` strips the class off a token span. The mirrored documentation is
  highlighted because the site's own template renders it.
- **`hreflang` alternates are absent.** See 4, 18 and 22: the theme has neither
  the language nor the page's reader-facing path.

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

The site is built, checked, and reproducible from an empty machine in 24 seconds.
`docs/DEPLOY.md` is a procedure a stranger could follow. Two of the gates on that
list leave a feature switched off rather than the site broken, and the site says
which and why on the page where it matters.

The one that is not a decision is the screen-reader pass. `/accessibility` says it
has not happened. It should happen before the site is announced, and until it
does that page will keep saying so.
