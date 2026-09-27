# Deploying Backline

One instance, on one VPS, at `backline.betafactory.co` (§15). Media is served
from `backline-media.betafactory.co`. Deployment is manual and there is no CI —
`mise run check` is what stands between a laptop and production.

```bash
mise run check       # fmt, clippy, tests, types — before every deployment
mise run vps:check   # is the machine ready? read-only
mise run vps:deploy  # ship, build, start, bootstrap the admin, verify
```

---

## What owns what

Ports 80 and 443, TLS, the certificates and the firewall belong to **the
machine**, not to this application. They live in the `vps-infra` repository,
which runs a shared edge (`caddy-docker-proxy` + `docker-socket-proxy`) for
every project on the box.

Backline's side of that contract is three lines, in `compose.prod.yml`:

1. join the external `edge` network,
2. declare a hostname with `caddy:` labels,
3. **publish no ports at all**.

So there is deliberately no `vps:setup` here: two repositories provisioning the
same `ufw` would fight. `vps:check` asserts the machine is ready and refuses to
deploy otherwise — if it reports no `edge` network, run `mise run proxy:up` in
`vps-infra`.

The application keeps its own Caddy *behind* that edge, listening on plain
`:80`. It holds the `/api` · `/ical` · `/health` split from `infra/Caddyfile`,
which is also what local development exercises. Giving it a hostname instead
would make it attempt ACME on a port it cannot answer.

### Why media has its own hostname

Every browser-facing media URL is presigned (`api/src/services/storage.rs`).
The client is path-style, so a URL is `<endpoint>/<bucket>/<key>`, and the
SigV4 signature covers the **Host** header. A `/media/*` prefix on the main
hostname would either make MinIO read `media` as the bucket, or — if the prefix
were stripped — break the signature. Hence a sibling name.

Two things must therefore never be added to the media labels: a path prefix, and
`header_up Host {upstream_hostport}`. Caddy preserves the inbound Host by
default, which is exactly what the signature needs.

The bucket is **not** anonymous-readable in production. Nothing reads it without
a presigned URL, so public objects would add no capability and would remove the
expiry — a link pasted into a chat would stay live for good.

---

## Secrets

Generated **on the server**, once, into `~/backline/.env`, by the first
`vps:deploy`. They never pass through the laptop and are never written into the
repository. A redeploy leaves that file untouched (the shipped tarball follows
`.gitignore`, which excludes it).

`openssl rand -hex` only, never `-base64`: `MINIO_ROOT_PASSWORD` is interpolated
into a `/bin/sh -c` string in `storage-init`, and a `/` or a shell
metacharacter there produces a bucket that silently never gets created.

Two layers refuse a misconfigured deployment:

- `compose.prod.yml` uses `${VAR:?message}` — it fails at parse time, before an
  image is pulled.
- `api/src/config.rs` refuses to start when `PUBLIC_BASE_URL` is https and
  `SESSION_SECRET` is still the development value published in
  `docker-compose.yml`, is shorter than 32 characters, or `S3_SECRET_KEY` is the
  development one. Every rule is conditioned on https, so a local stack is
  untouched.

To read or change a value:

```bash
mise run vps:shell            # then: cd backline && cat .env
```

After editing `.env`, restart what reads it:

```bash
ssh ubuntu@92.222.171.209 'cd backline && docker compose -f docker-compose.yml \
  -f compose.prod.yml --env-file .env up -d api bot'
```

---

## Accounts

Production never runs the seed (`RUN_SEED=0`): the demo collective has no
business in a real instance. §3 still requires an instance admin who signs in
with an email and a password, so administration never depends on Telegram — and
passwords are argon2-hashed, so no `psql` workaround exists.

`vps:deploy` therefore runs `bootstrap-admin` on **every** deployment. It is
idempotent: with an instance admin already present it changes nothing.

Afterwards, accounts are created from `/instance` in the browser, or from the
server:

```bash
cd backline
C="docker compose -f docker-compose.yml -f compose.prod.yml --env-file .env"
$C run --rm --no-deps api accounts list
$C run --rm --no-deps api accounts create --name "Ada" --email ada@example.org \
    --collective bonsoir-techno --role admin
$C run --rm --no-deps api accounts invite ada@example.org   # a fresh link
```

Both print a single-use invitation link valid 30 days; issuing one kills the
previous.

---

## Telegram

The bot **long-polls** (`api/src/services/bot.rs`): no inbound route, no
webhook to register, nothing to break when a certificate renews. Deployment
works with no token at all — the bot idles and notifications are logged.

To enable it, put the token in the server `.env` and restart the bot. Then:

- **BotFather `/setdomain` → `backline.betafactory.co`**, or the Telegram Login
  Widget refuses silently.
- `getUpdates` is exclusive: exactly one process may poll a token. Use a
  **separate BotFather bot for development**, and check once that
  `https://api.telegram.org/bot<token>/getWebhookInfo` returns `"url":""` — a
  leftover webhook makes `getUpdates` return 409 for ever.

---

## Building on the VPS

The box has 2 cores, ~4 GB of RAM and 2 GB of swap. Three precautions, all
already in place:

- `CARGO_BUILD_JOBS=2` in `api/Dockerfile`.
- `vps:deploy` builds `api`, `web` and `stills` **one at a time**; `up --build`
  fans out, and rustc, vite and a Chrome download in flight together are what
  makes the box OOM.
- `.dockerignore` keeps `target/` and `node_modules/` out of the build context —
  not for upload size, but because `web/` and `stills/` copy their sources
  *after* `pnpm install`, so a local `node_modules/` would be laid over the
  freshly installed tree.

The first build takes 15–30 minutes. Later ones reuse the BuildKit cache on the
machine. That cache grows; the disk is shared with every other project, so
occasionally:

```bash
ssh ubuntu@92.222.171.209 'docker builder prune --filter until=168h -f'
```

The `api` build is the one that can push the box over: it is a release build of
the whole Rust tree, on a machine that shares its memory with every other
project. Build it here instead and ship the image:

```bash
VPS_LOCAL_BUILD=api mise run vps:deploy
```

`web` and `stills` stay on the VPS — they are heavy on disk, not on memory, and
shipping the Chrome layer of `stills` costs far more than fetching it there.

---

## Object storage, and a supply-chain scar

MinIO stopped publishing pullable images. The Docker Hub repository
(`minio/minio`, `minio/mc`) is gone, and `quay.io/minio/*` answers **401 to an
anonymous request — pinned release tags included**. A machine that has never
pulled them cannot obtain them, whatever the compose file says.

`vps:deploy` therefore checks every third-party image before starting: what the
VPS cannot pull is shipped from the machine running the deployment, over the
same `docker save | ssh docker load` stream used for a locally built image. The
first deployment worked precisely because this laptop still held the images
pulled before the change.

**That is a stopgap and it will rot.** The day no machine here holds them, the
stack cannot be rebuilt from scratch. Three ways out, in increasing order of
effort:

1. push the two images to a registry under our control and reference that;
2. move object storage to **OVH Object Storage**, which `PRD.md` §15 and
   `api/src/services/storage.rs` already anticipate — `S3_ENDPOINT` and the
   credentials are all that change, and MinIO disappears from the stack;
3. swap MinIO for a freely distributed S3-compatible server (Garage, SeaweedFS)
   — a real migration, since the bucket and its keys are created differently.

Option 2 is the one the specification points at.

---

## Backups

`infra/backup.sh` runs on the server at 03:10 daily (the cron entry is installed
by `vps:deploy`): `pg_dump | gzip` plus an `mc mirror` of the bucket into
`~/backline/backups/`. Retention is a fortnight of dailies, keeping the first of
each month for good.

```bash
mise run vps:backup     # dump now, then pull everything here
mise run backup:verify  # restore the newest dump into a throwaway PostgreSQL
```

`backup:verify` is what discharges §15's *tested* restore: it restores and
counts rows, then tears the container down.

**Known deviations from §15**, deliberate and worth revisiting:

- no offsite copy to OVH Object Storage — backups land on the machine and on
  whichever laptop ran the pull, the same choice `vps-infra` makes;
- no monitoring and no "a service went down" Telegram alert — nothing on this
  machine has any.

---

## After a deployment, check the chain

Each of these proves a different link, in order.

```bash
# 1. the edge routes both names, and DNS agrees   (run in vps-infra)
mise run sites

# 2. TLS, and no Backline container owns 80/443
curl -sI https://backline.betafactory.co/ | head -3
ssh ubuntu@92.222.171.209 'ss -tlnp | grep -E ":(80|443)"'   # only the edge

# 3. edge → inner proxy → api
curl -s https://backline.betafactory.co/health                # ok

# 4. the production env actually reached the container
curl -s https://backline.betafactory.co/api/config            # https base URL

# 5. SPA fallback
curl -so /dev/null -w '%{http_code}\n' https://backline.betafactory.co/evenements/x

# 6. sign in — the cookie must carry Secure
curl -isX POST https://backline.betafactory.co/api/auth/login/password \
  -H 'content-type: application/json' \
  -d '{"email":"…","password":"…"}' | grep -i set-cookie
```

Then, signed in, through the interface:

7. upload an image in the Studio (one over 2 MB — that was axum's old default
   limit) and confirm it displays. The asset URL must point at the media host
   and carry `X-Amz-Signature`.
8. the **same URL with the query string removed must return 403** — that is the
   assertion that anonymous bucket read is off.
9. subscribe to an iCal feed in a real calendar client; the `URL:` lines inside
   must be https.
10. with a token: `/start`, `/agenda`, then `/fiche` — the PDF arriving proves
    Telegram's servers reach the media host from the public internet.
11. `sudo reboot`, then re-run 3, 7 and the bot check: `restart: unless-stopped`
    must bring everything back, and `storage-init` staying `exited (0)` must not
    block `api`.
