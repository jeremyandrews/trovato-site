# Deploying trovato.rs

Everything needed to put this site on a server, in the order it has to happen.
Every command is meant to be copied and run; where one needs a value from you,
the value is named.

Nothing here has been run against a real host. It has been run end to end on a
laptop against the same compose files, the same proxy configuration and the same
unpublished kernel, with a certificate from Caddy's own authority standing in for
a public one: `./scripts/check-production.sh`. What is untested is DNS, a public
certificate authority, and real mail.

## What you need

**A host.** One machine, with Docker and the Compose plugin. The kernel image is
published for `linux/amd64` and `linux/arm64`, so an ARM instance is fine and
usually cheaper.

Sizing, from what the stack contains rather than from a benchmark: five
containers, of which PostgreSQL and the kernel are the ones that use memory.
2 GB of RAM and 2 vCPU is a reasonable starting point, and 20 GB of disk leaves
room for the database, the images pulled and a few backups. Neither number has
been measured under load, and no performance figure appears anywhere on the site
for the same reason.

**Ports 80 and 443 reachable from the internet.** 80 is not optional: the
certificate authority uses it to prove you control the name.

**Nothing else listening on them.** The proxy binds both.

**DNS you can edit.** See below.

**Outbound HTTPS.** The host pulls images from `ghcr.io` and reaches the
certificate authority. If it is behind a firewall that blocks outbound 443,
neither works.

## DNS

Two records, both pointing at the host's public address. Replace `203.0.113.10`
with it.

```
trovato.rs.       A     203.0.113.10
www.trovato.rs.   A     203.0.113.10
```

On IPv6, add the same pair as `AAAA` records. If your host has IPv6 and you skip
this, some readers reach the site and some do not, intermittently, which is the
worst version of the problem.

`www` is there so it can redirect to the apex. Without the record, somebody who
types `www.` gets a DNS failure rather than a redirect.

**Wait for the records to resolve before starting the stack.** Caddy asks for a
certificate on its first start; if the name does not resolve yet, that attempt
fails and it retries with a backoff. It is not broken, but you will spend ten
minutes wondering.

```
dig +short trovato.rs
dig +short www.trovato.rs
```

Both must print the host's address before you continue.

## One command, shortened

Every command below runs `docker compose` with the same two files and the same
environment file. Rather than repeat that, define it once per shell session:

```
dc() {
  docker compose -f docker-compose.yml -f docker-compose.production.yml \
    --env-file .env.production "$@"
}
```

Put it in the host's `~/.bashrc` if you would rather not retype it after every
log-out. Everything from here on says `dc` and means exactly that.

## First boot

### 1. Get the repository onto the host

```
git clone https://github.com/jeremyandrews/trovato-site
cd trovato-site
```

### 2. Write the configuration

```
cp .env.production.example .env.production
```

Open it and replace every `CHANGE ME`. There are ten. Two need generating:

```
openssl rand -base64 36   # POSTGRES_PASSWORD
openssl rand -hex 32      # CRON_KEY
```

`CRON_KEY` is not decoration. The kernel runs no internal scheduler: scheduled
publishing, search index rebuilds and the comment moderation queue all fire from
a POST to `/cron/${CRON_KEY}`, which the cron container sends every minute. A
guessable key is a way for anybody to make the site do work on demand.

Leave the AI block empty unless you have already decided on a provider. The site
runs without one and says so on the pages where it matters.

If you leave the SMTP block empty, registration cannot be completed by anybody:
the account is created and the verification link only ever exists in the email.
That is worth knowing before you announce the site rather than after.

### 3. Build the site plugin

The plugin is built from source into the overlay the kernel mounts. This needs a
Rust toolchain, and only for this step.

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
./scripts/build-overlay.sh
```

`rust-toolchain.toml` in this repository pins the version and the
`wasm32-wasip1` target, so rustup fetches the right ones without being told. The
build takes a few minutes the first time and produces two files in `overlay/`.

If you would rather not install Rust on the server, run this on any machine with
the toolchain and copy the resulting `overlay/` directory across. It contains two
files.

### 4. Start it

```
dc up -d
```

The first start pulls five images, runs the kernel's schema migrations, and asks
for a certificate. Watch it:

```
dc logs -f proxy site
```

You are waiting for `certificate obtained successfully` from the proxy and
`Server listening` from the site. Then:

```
curl -sI https://trovato.rs/health
```

A `200` means the transport works. The site itself is not set up yet.

### 5. Run the installer

Open `https://trovato.rs/install` in a browser. It asks for an administrator
username, an email address and a password, then a site name and a site email.

This is the one step that is not a command, deliberately: it sets the
administrator's password, and a password that came from a file in a repository
is a password everybody has.

Use the site name **Trovato** and the slogan **A content management system
written in Rust.** The site name appears in the header, in `og:site_name` and in
the footer.

### 6. Enable the plugins

```
dc exec site sh -c '
for p in trovato_blog trovato_seo trovato_search trovato_contact \
         trovato_scheduled_publishing trovato_redirects trovato_comments \
         trovato_spam trovato_site; do
  ./trovato plugin install "$p" || ./trovato plugin enable "$p"
done'
```

Every one of those except `trovato_site` ships in the kernel image. The site
enables what exists rather than carrying its own copy.

Then restart, because a plugin's content types are registered when the kernel
loads it:

```
dc restart site
```

### 7. Import the content

Two imports. `config import` reads one directory and does not recurse, and the
mirrored documentation is a separate set of 72 files.

```
dc exec site ./trovato config import /site/config
dc exec site ./trovato config import /site/config/docs
```

Then restart again. Gather queries and menu links are read into their registries
at startup, so an imported query is in the database and not yet routable: without
this, `/news` answers `{"error":"query not found"}` and the main menu renders
empty.

```
dc restart site
```

### 8. Check it

```
curl -s -o /dev/null -w '%{http_code}\n' https://trovato.rs/
curl -s https://trovato.rs/sitemap.xml | head -5
curl -sI https://trovato.rs/why | grep -i 'cache-control\|strict-transport'
curl -s -o /dev/null -w '%{http_code} %{redirect_url}\n' https://www.trovato.rs/
```

Expect: `200`; a sitemap whose first `<loc>` is `https://trovato.rs/`;
`Cache-Control: public, max-age=60, stale-while-revalidate=600` and an HSTS
header; and a `301` to `https://trovato.rs/`.

## Every setting

`.env.production` is the whole of what this deployment is configured by. Each
line below is one setting in it.

| Setting | What it does | If it is wrong |
|---|---|---|
| `SITE_HOST` | The hostname the proxy serves and gets a certificate for. `www.` of it redirects here. | No certificate, and nothing answers on the name |
| `SITE_URL` | The base of every absolute URL the site emits: canonicals, Open Graph, sitemap entries, feed self-links. | Search engines are told the site lives somewhere it does not |
| `ACME_EMAIL` | Where the certificate authority writes about expiry and problems. | You find out about an expiring certificate from a reader |
| `TROVATO_VERSION` | The kernel release this runs. Never `latest`. | An upgrade happens when somebody else publishes, not when you decide |
| `RUST_LOG` | Log verbosity. `info,tower_http=info,sqlx=warn` is the useful default. | Either no signal or every SQL statement |
| `POSTGRES_USER`, `POSTGRES_DB` | The database role and name. The defaults are fine. | The kernel cannot connect |
| `POSTGRES_PASSWORD` | That role's password. Generate it. | The database is only as private as the password |
| `CRON_KEY` | The secret in the URL the cron container POSTs to every minute. Scheduled publishing, search index rebuilds and comment moderation all fire from it. | Guessable: anybody can make the site do work. Wrong: none of those three ever run |
| `CRON_INTERVAL` | Seconds between pokes. 60. | Longer means scheduled things happen later |
| `COMPOSE_SUBNET`, `PROXY_IP` | The compose network and the proxy's fixed address on it. `TRUSTED_PROXIES` is derived from `PROXY_IP`. | Every visitor shares one bucket of a hundred requests a minute |
| `SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_ENCRYPTION` | The mail relay. | Empty: nobody can complete a registration and the contact form delivers nothing, silently |
| `SITE_MAIL` | The address system mail comes from. Must be one the relay will send as. | Mail is rejected or filed as spam |
| `AI_API_KEY_ENV`, `AI_API_KEY` | The name of the variable holding the AI provider's key, and the key. The `ai_providers` entry in site configuration names the same variable. | Empty: comment classification never answers, so comments wait for a person, and search is keyword-only. Both are stated on the site |

`.env.example` carries a different set: `SITE_PORT`, `PROXY_PORT`, `SITE_ADDRESS`
and `AUTO_HTTPS` are for running the stack locally and have no meaning here.
`TRUSTED_PROXIES` is set by `docker-compose.production.yml` from `PROXY_IP` and
should not be set by hand.

## Backups

### What has to be backed up

Only PostgreSQL. Everything else is either in the repository or can be fetched
again:

| Volume | What is in it | Back up? |
|---|---|---|
| `postgres_data` | Every page, post, comment and account | **Yes** |
| `redis_data` | Sessions and caches | No. Losing it logs everybody out. |
| `uploads` | Uploaded files | Yes, once anything has been uploaded |
| `caddy_data` | Certificates | Optional. Losing it means asking for a new certificate, which is fine unless it happens often enough to be rate-limited. |

### Taking one

```
dc exec -T postgres pg_dump -U trovato -Fc trovato > trovato-$(date +%F).dump
```

`-Fc` is the custom format: compressed, and restorable selectively. Copy the file
off the host. A backup on the same disk as the database is not a backup.

### The restore drill, which you should run once before you need it

Restoring is the half nobody tests. Do it now, on a throwaway database, so that
the first time is not the day it matters.

```
# 1. A scratch database inside the same PostgreSQL.
dc exec -T postgres createdb -U trovato restore_drill

# 2. Restore into it.
dc exec -T postgres pg_restore -U trovato -d restore_drill < trovato-$(date +%F).dump

# 3. Ask it something only real data can answer.
dc exec -T postgres psql -U trovato -d restore_drill -c 'SELECT count(*) FROM item;'

# 4. Throw it away.
dc exec -T postgres dropdb -U trovato restore_drill
```

Step 3 should print the number of items the live site has. If it prints `0`, the
backup is empty and you have found that out on a quiet afternoon.

### Restoring for real

```
dc stop site cron
dc exec -T postgres dropdb -U trovato trovato
dc exec -T postgres createdb -U trovato trovato
dc exec -T postgres pg_restore -U trovato -d trovato < BACKUP.dump
dc start site cron
```

Stop the site first. Restoring underneath a running kernel gives it a database
that changes shape while it is reading.

## Upgrading the kernel

Trovato's migrations are forward-only. There is no rollback, and an upgrade that
goes wrong is recovered from a backup, which is why the backup section is above
this one.

```
# 1. Back up, and confirm the dump is not empty.
dc exec -T postgres pg_dump -U trovato -Fc trovato > pre-upgrade.dump
ls -lh pre-upgrade.dump

# 2. Change one line in .env.production.
#    TROVATO_VERSION=0.102.0

# 3. Pull and restart. Migrations run on startup.
dc pull site
dc up -d site

# 4. Watch the migrations.
dc logs -f site

# 5. Check.
curl -s -o /dev/null -w '%{http_code}\n' https://trovato.rs/
```

Two things to do in the repository afterwards, neither of which is urgent and
both of which drift if forgotten:

- **The SDK pin.** `Cargo.toml` pins `trovato-sdk` to a revision of the kernel
  repository. The plugin keeps working across a minor release without rebuilding,
  because the kernel accepts a plugin whose major version matches and whose minor
  version is no higher than its own. Repin at your convenience; repin before a
  major.
- **The documentation mirror.** `TAG` in `tools/src/docs_import.rs` names the
  release the documentation is mirrored from. Change it, run
  `cargo run -p tools --bin docs-import`, commit the result, deploy, and re-import
  `/site/config/docs`. CI fails if the mirror and the tag disagree.

## Changing the site

Content and configuration live in `config/` in this repository. To change a page:
edit the file, commit, pull on the host, and re-import.

```
git pull
dc exec site ./trovato config import /site/config
dc restart site
```

The restart is needed whenever a gather query, a menu link or a variable changed.
It is not needed for an item's body, but doing it always is one rule instead of
two.

`config import` validates the whole set before it writes anything: one bad file
means nothing is written and the run names the file.

## Things this deployment does deliberately

**The kernel publishes no port.** Only the proxy listens. The kernel believes an
`X-Forwarded-For` header from an address in `TRUSTED_PROXIES`, and the proxy is
in that list; anything else able to reach the kernel directly could claim to be
any client it liked and mint rate-limit buckets without limit. If you add a port
mapping to the `site` service for debugging, bind it to `127.0.0.1` and take it
off again.

**The proxy serves the site's own static files.** The kernel rate-limits every
GET at a hundred a minute per IP, static assets included, and it is not
configurable. Measured: the 101st consecutive request for a stylesheet returns
429. A page here pulls about ten subresources, so without this a reader gets ten
pages into the site before their fonts stop loading.

**`TRUSTED_PROXIES` names an exact address, not a range.** That is why the proxy
has a fixed address on a fixed subnet in the compose files. If you change
`COMPOSE_SUBNET`, change `PROXY_IP` to match.

**HSTS is set, and `preload` is not.** The header promises a browser that this
hostname will always be reachable over TLS, for a year. Preloading is
effectively irreversible and is a decision to make deliberately.

**`/sitemap.xml` is served by the proxy from `/sitemap/pages.xml`.** The kernel
owns `/sitemap.xml` and emits paths where the protocol requires absolute URLs,
and lists items only, so the site generates its own. The plugin cannot claim the
standard path because two handlers on one route make axum panic at startup.

## What is not decided

These are not steps. They are things somebody has to choose, and the site runs
without them:

- **Where it is hosted, who is on call, and what it may cost per month.**
- **SMTP credentials.** Without them nobody can complete a registration and the
  contact form delivers nothing. Both features are built and waiting.
- **An AI provider, a model and a token budget.** Without one, comment
  classification never returns a verdict, so every comment from an account that
  has not earned its way past the queue waits for a person; and the two AI stages
  of search do not run, so search is keyword-only. Both are the safe failure and
  both are stated on the site.
- **A mailbox at the domain** for the contact form to deliver to.
