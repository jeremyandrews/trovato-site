# Brand asset provenance

Every file in this directory is a copy, taken unchanged from the Trovato kernel
repository at tag **v0.101.0** (commit `5304a68`):

- the SVGs from `assets/brand/`
- the PNGs from `assets/brand/png/`

`assets/brand/BRAND.md` in that repository is the source of truth for the palette,
the lockups, the clearspace rules and what may not be done to the mark. This
directory holds copies so that the site serves its own assets from its own origin;
it is not a second source of truth. `gen_final.py` there regenerates every SVG from
geometry constants — if a mark changes, it changes there first and is re-copied here.

The kernel image also ships a subset of these under `/app/static/brand/`. The site's
`STATIC_DIR` entry comes later in the search path, so these copies win the name
collision and the whole set is served from one place.
