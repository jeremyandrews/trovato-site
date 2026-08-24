# Launch checklist

A box is ticked here only when something checks it and can go red. Where the
check is automated, the command is named: run it and it either passes or tells
you what is wrong. Everything unticked needs a person, and each one says who and
what for.

Ticked items were verified against a clean-room rebuild: every container and
volume destroyed, then `./scripts/run.sh --fresh --proxy`, which took 24 seconds
from nothing to a populated site.

## The site itself

- [x] **Every page serves.** 14 destinations plus 4 Italian pages, all 200.
      `npm run crawl` walks 191 pages from the front page and the Italian entry
      point and fails on any 404, any 5xx, and any page served as a
      template-failure dump.
- [x] **Every page is themed and navigable.** The same crawl fails a page missing
      its main landmark, its stylesheet, its skip link or its main navigation.
- [x] **The site works with JavaScript switched off.** Search submits to the
      server, the narrow-screen menu is a `<details>` disclosure, the comment form
      and the contact form are plain form posts. `npm run crawl` fetches documents
      only and every page renders.
- [x] **No page reaches another origin.** `npm run shoot` records every request on
      every page in both colour schemes and fails on anything off-site. Fonts,
      styles, scripts and images are all served from here.
- [x] **Both colour schemes work at both widths.** 36 screenshots, no horizontal
      scroll at 360px, nothing carrying a background from the wrong scheme.
- [x] **Accessibility, as far as a machine can check it.** `npm run a11y` runs
      axe-core over one page per template family in both schemes, against WCAG
      2.0, 2.1 and 2.2 A and AA plus best-practice rules, with nothing
      allowlisted. No violations.
- [x] **Colour contrast.** Every pairing the stylesheet declares is verified by
      `cargo test -p checks`, in both schemes, including the seven
      syntax-highlighting colours.
- [x] **The feeds and the sitemap are valid.** `/rss/blog.xml`, `/rss/news.xml`
      and the sitemap are well-formed XML with absolute URLs; both feeds are
      discoverable from every page's head.
- [x] **The documentation mirror matches the release it claims.**
      `cargo run -p tools --bin docs-import -- --check` re-runs the import against
      the pinned tag and fails if anything would change. 36 documents,
      syntax-highlighted, with a markdown source URL each.
- [x] **`llms.txt` lists what exists.** Built from the same rows the site renders,
      so it cannot name a page that is not there. Every link in it is followed by
      the crawl.
- [x] **The whole site is reproducible from this repository.**
      `./scripts/check-roundtrip.sh` exports the running site's configuration and
      verifies that all 54 hand-written and 72 generated entities come back with
      every declared key unchanged.

## Community

- [x] **Registration works and is rate-limited.** Measured: the fourth attempt
      from one address returns 429 with `retry-after: 3600`.
- [x] **Comments work and are rate-limited.** Measured: four succeed, the fifth
      returns 429.
- [x] **Comments are held, and approval publishes them.** Verified end to end:
      posted, held, invisible to an anonymous visitor, visible in the admin queue,
      approved, then visible.
- [x] **Moderation fails closed.** `./scripts/check-moderation.sh` posts a
      comment, drains the classification queue, and fails if anything published.
      It was itself verified to go red, by flipping `comment_default_status` to
      `published`.
- [ ] **Somebody can actually receive mail from the site.** Blocked on SMTP
      credentials. Until then registration creates an account and cannot send its
      verification link, and the contact form delivers nothing. **Jeremy.**

## Production

- [x] **The production profile serves the site over TLS with its headers on.**
      `./scripts/check-production.sh` runs the production compose files as
      written, with a certificate from Caddy's own authority standing in for a
      public one, and checks TLS, HSTS, the caching policy, the sitemap, the www
      redirect and the HTTP-to-HTTPS redirect.
- [x] **Only the proxy is exposed.** The same check asserts that no other service
      declares a published port. A caller able to reach the kernel directly could
      claim any client address it liked.
- [x] **The deployment procedure is complete.** `docs/DEPLOY.md`: DNS, first boot,
      every setting, backups with a restore drill, and the upgrade. Read back
      adversarially; every command is copy-pasteable.
- [ ] **DNS points at a host.** Two A records, and AAAA if the host has IPv6.
      **Jeremy.**
- [ ] **There is a host, somebody on call, and a monthly budget.** **Jeremy.**
- [ ] **A backup runs on a schedule, and the restore drill has been run once.**
      The procedure is in `docs/DEPLOY.md` and needs a host to run on. **Jeremy.**

## What only a person can check

- [ ] **A blind visitor should be able to register, search, read the tutorial, and
      post a comment without help.**

      This is the one that matters and no automated check can tick it. axe-core
      finds missing labels and bad contrast; it does not tell you whether a page
      is comprehensible read aloud, whether the reading order makes sense, or
      whether the comment form's confirmation is announced. Somebody has to sit
      down with VoiceOver and with NVDA and try to do those four things.

      `/accessibility` says this has not been done. It stays unticked and stays
      said until it has. **Jeremy, or somebody he asks.**

- [ ] **A deliberate keyboard pass over the whole site.** The disclosure menu is
      opened by keyboard in the automated pass and code blocks are focusable. The
      rest has not been tabbed through on purpose. **Jeremy.**

- [ ] **The copy reads right to somebody who is not the person who wrote it.**
      Nine pages in the site's own voice. The claims are checked and the numbers
      are traced; whether it sounds like Jeremy is Jeremy's call. **Jeremy.**

## Decisions the site is waiting on

None of these stops a launch. Each one leaves a feature off, and the site says so
on the page where it matters rather than pretending.

- [ ] **An AI provider, a model, and a monthly token budget.** Without one,
      comment classification never returns a verdict, so every comment from an
      account that has not earned its way past the queue waits for a person; and
      the two AI stages of search do not run, so search is keyword-only. Both are
      the safe failure. `/search` and `/comment-policy` both say so. **Jeremy.**
- [ ] **Whether search is called Scolta.** The widget's class names carry a
      product name belonging to Tag1. None of it reaches a reader, and the copy
      calls the feature search. Using the name is a deliberate yes, not a default.
      **Jeremy.**
- [ ] **A mailbox at the domain** for the contact form to deliver to. **Jeremy.**
- [ ] **Where the repository lives.** It is at `jeremyandrews/trovato-site`,
      matching the kernel. Moving it under an organisation changes one remote.
      **Jeremy.**
- [ ] **Publishing `trovato-sdk` to crates.io.** The site pins it as a git
      dependency by revision, which works and means every plugin author does the
      same. **Jeremy.**
- [ ] **Published performance numbers.** No performance figure appears anywhere on
      the site, because none has been measured. The EPYC benchmark harness and the
      Goose run are what would change that. **Jeremy.**

## After launch

Not blocking, and worth writing down before they are forgotten.

- [ ] A public demo instance. It needs a second operated deployment.
- [ ] `scolta-trovato`, the flagship search integration.
- [ ] Anonymous commenting, if it is ever wanted. Today an account is required.
