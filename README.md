# trovato-site

The official website for [Trovato](https://github.com/jeremyandrews/trovato), built
on a released Trovato kernel with no modification to the kernel itself.

The site is an external application. It adds nothing to the Trovato tree and patches
nothing there. Everything it contributes — a site plugin, templates, config, static
assets, seed content — is layered on through `PLUGINS_DIR`, `TEMPLATES_DIR` and
`STATIC_DIR`, which the kernel reads as colon-separated search paths where a later
entry wins a name collision.

Target domain: **https://trovato.rs**

## Repository home

This repository lives at `jeremyandrews/trovato-site`, matching the kernel's home at
`jeremyandrews/trovato`. Moving it under an organization is a decision for later; the
only thing that would change is the remote.

## Layout

```
plugins/trovato_site/   the site plugin: the `news` content type and site taps
templates/              template overrides layered over the kernel's
config/                 the importable site definition (content types, menus, queries, aliases)
content/                seed content, importable so a fresh database is not empty
static/                 brand assets, fonts, the design-token stylesheet
scripts/                overlay assembler, demo runner, crawler
docs/                   LEDGER.md (build log), DEPLOY.md (production)
```

## Running it

```
./scripts/run.sh            # http://127.0.0.1:3080
./scripts/run.sh --proxy    # and through the front proxy on :8081
```

From nothing: it builds the site plugin, brings up PostgreSQL, Redis, the pinned
kernel and the cron poker, runs the installer, imports the config and the
mirrored documentation, and restarts so both are live. `--fresh` destroys this
project's volumes first and starts over.

## Checking it

Each of these is a gate, and all of them run in CI on every push.

```
cargo test --all                  # the plugin, the contrast of every token pairing, the content set
npm run crawl                     # every page reachable from the front page
npm run a11y                      # axe-core, both colour schemes, nothing allowlisted
npm run shoot                     # both widths, both schemes, no overflow, no off-origin requests
./scripts/check-moderation.sh     # comment moderation fails closed
./scripts/check-roundtrip.sh      # every declared config entity survives an export
./scripts/check-production.sh     # the production profile, over TLS, on this machine
cargo run -p tools --bin docs-import -- --check   # the documentation mirror matches the pinned tag
```

## Production

```
./scripts/check-production.sh     # the production profile, locally, end to end
```

[docs/DEPLOY.md](docs/DEPLOY.md) is the procedure for a real host: DNS, first
boot, backups and the restore drill, and the upgrade. `docs/LEDGER.md` is the
build log, gate by gate, including everything that did not work and why.

## Brand assets

The SVGs under `static/brand/` are copied from `assets/brand/` in the Trovato kernel
repository at tag v0.101.0. `assets/brand/BRAND.md` there is the source of truth for
the palette, the lockups and the rules; this repository holds copies, not originals.

## License

Dual licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option,
matching the kernel.
