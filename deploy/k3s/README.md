# The review site on the home k3s cluster

The trovato.rs site, deployed from a working tree to namespace `trovato-site`
on the ks cluster, on the LAN only: <http://10.20.250.17.nip.io/> (or
<http://10.20.250.17/>). No Ingress and no DNS record.

## Use

```sh
deploy/k3s/apply.sh                # build, push, diff, apply, populate
deploy/k3s/apply.sh --no-populate  # stop after the rollout
deploy/k3s/gate.sh                 # crawl, a11y, shoot, roundtrip, moderation
deploy/k3s/gate.sh crawl roundtrip # any subset
```

`apply.sh` is idempotent and is also how content changes are deployed: edit
`config/`, `templates/` or `static/`, run it again. It builds the plugin, bakes
the overlay into an image on the pinned kernel (`Dockerfile`), pushes it, prints
`kubectl diff`, applies, waits for the rollout, then does what `scripts/run.sh`
does: enables the plugins, runs the installer on a fresh database only, imports
`config/` and `config/docs/` (which overwrites the database copy of each item),
restarts the site so queries and menus load, and runs cron once.

Image: `registry.pozza/jeremyandrews/trovato-site:<short sha>`, with
`-dirty-<content hash>` appended when the tree has uncommitted changes, so every
distinct content gets its own tag. The push goes straight to the registry if
this machine is logged in (`docker login registry.pozza -u jeremyandrews`);
otherwise it is relayed through ks1, whose root podman holds the login.

Environment: `KUBECONFIG` (default `~/.kube/ks1.yaml`), `LB_IP` (default
`10.20.250.17`), `SITE_HOST` (default `<LB_IP>.nip.io`), `SITE_BASE` for the
gates (default `http://<LB_IP>.nip.io`), `PUSH_HOST` (default `ks1.pozza`).

The administrator is `admin`; the password, the database password and the
cron key are in Secret `trovato-site-secrets`, generated on the first apply and
never written to disk:

```sh
kubectl -n trovato-site get secret trovato-site-secrets -o jsonpath='{.data.ADMIN_PASS}' | base64 -d
```

## What runs

- `postgres` and `redis`: StatefulSets on `local-path` volumes (5Gi, 1Gi).
- `trovato-site`: one pod with the kernel (`site`, port 3000), Caddy (`proxy`,
  port 80, `deploy/shared.caddy` with the upstream pointed at loopback; it
  serves `/static` itself from a copy the init container takes out of the
  image) and the cron poker (`cron`). Uploads on a 1Gi `local-path` volume.
- Service `trovato-site`: LoadBalancer, `externalTrafficPolicy: Local`, address
  fixed by the `metallb.io/loadBalancerIPs` annotation.

Every pod requires arm64 and excludes ks2 and ks3, which leaves ks1.

## Why SITE_URL is a nip.io name

The kernel takes its WebAuthn relying party ID from the host in `SITE_URL` and
will not start when that host is an IP address. `10.20.250.17.nip.io` is public
wildcard DNS that answers with `10.20.250.17`, so the canonical, Open Graph,
sitemap and feed URLs reach this Service without a DNS record of our own.

## The gate scripts

`check-roundtrip.sh` and `check-moderation.sh` run the scripts under
`scripts/` unmodified, with `shim/docker` first on `PATH` turning their
`docker compose exec` and `cp` calls into `kubectl exec`.

- Roundtrip compares the export with the config the running image was built
  from, so edits made since the last apply do not count against it.
  `ROUNDTRIP_AGAINST=worktree` compares with the tree instead.
- Moderation registers a user and posts a comment, so it runs against a
  throwaway site (`modcheck.yaml`: one pod, emptyDir, label
  `app=trovato-modcheck`) reached through `kubectl port-forward`, and deletes
  it afterwards.
- `shoot` writes its screenshots under the temp directory `gate.sh` prints.

## Removing it

```sh
kubectl delete namespace trovato-site
```

This deletes the volumes and the generated secrets with it.
