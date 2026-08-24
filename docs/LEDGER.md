# Build ledger

**Status:** Phase 0 complete, Phase 1 in progress
**Last updated:** 2026-08-24
**Where things stand:** Environment verified, repository bootstrapped, kernel pinned at v0.101.0.

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
