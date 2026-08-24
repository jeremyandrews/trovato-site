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

See `docs/LEDGER.md` for build status and `docs/DEPLOY.md` for production.

## Brand assets

The SVGs under `static/brand/` are copied from `assets/brand/` in the Trovato kernel
repository at tag v0.101.0. `assets/brand/BRAND.md` there is the source of truth for
the palette, the lockups and the rules; this repository holds copies, not originals.

## License

Dual licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option,
matching the kernel.
