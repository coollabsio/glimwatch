# Haruvo

Haruvo is a self-hosted status page, incident desk, and uptime monitor from [coolLabs](https://github.com/coollabsio), the team behind [Coolify](https://github.com/coollabsio/coolify). One Rust binary serves the API, runs the checks, and hosts the React app. SQLite is the default database. Postgres is the same schema and the same queries, selected with a URL.

The project site is [haruvo.io](https://haruvo.io). The source repository is [coollabsio/haruvo](https://github.com/coollabsio/haruvo).

![Dashboard](docs/screenshots/dashboard.png)

![Public status page](docs/screenshots/status-page.png)

## What it does

- HTTP and TCP monitors with an interval, a timeout, an expected status, a body assertion, and a latency line that marks a check degraded.
- A scheduler in the API process. Repeated failures move a monitor to down and open an incident. A later success resolves that automatic incident.
- Public status pages on a slug (`/status/coollabs`) with components, 90-day uptime bars, and a banner.
- Incidents with the OpenStatus timeline: investigating, identified, monitoring, resolved.
- Maintenance windows. Checks inside a window are stored and are not counted as downtime.
- Notifications. Email (SMTP) and signed webhooks work now. Slack and Discord are incoming-webhook channels on the same interface.
- Agents for private networks. An agent runs checks on targets that only a VPC or Docker network can reach, such as an internal database.
- Cookieless web analytics. A site gets a public id and a `/data.js` snippet. Page views, custom events, and Core Web Vitals are grouped without cookies. The client address is hashed into a visitor id and is not stored.
- A link shortener. Links live on a hostname you add, and also at `/go/{slug}` on this server, with click counts, referrers, and an optional public page.
- Error tracking. Create a project, copy its Sentry DSN, and point an official Sentry SDK at it. Issues group on a fingerprint, with a short id such as `CHECKOUT-1`, stack traces, breadcrumbs, tags, and the raw event.

## Run it

You need Rust 1.99 (see `rust-toolchain.toml`) and [Bun](https://bun.sh) 1.2 or newer.

```bash
cd apps/web && bun install && bun run build
cargo run -p haruvo-server -- seed
cargo run -p haruvo-server -- serve --listen 127.0.0.1:43123
```

Open http://127.0.0.1:43123.

`seed` creates a local demo and does nothing if an account already exists. It also marks setup complete, so a seeded database does not print a setup URL:

- email `test@example.com`
- password `password`
- a coolLabs status page, a demo analytics site (`coolify.io`, public id `pub_coollabs_demo`), short links at `/go/coolify` and `/go/docs`, and demo error projects (`Checkout`, `API`) with sample events

`just dev` runs `seed` before it starts the server. Production does not. A fresh database uses the setup URL below, not this password.

The database file is `data/haruvo.sqlite`. If that file is missing, Haruvo uses an older `data/status.sqlite` when it is already there. The Docker default `/var/lib/haruvo/haruvo.sqlite` does the same for `status.sqlite` in that directory. A custom `HARUVO_DATABASE_PATH` (or a `STATUS_` alias of it) is used as written. Migrations run on startup.

Without Bun on the PATH, `cargo test` still works. `apps/server/build.rs` writes a placeholder page when `apps/web/dist` is missing, so the binary compiles before the UI is built.

### Postgres

Leave the SQLite path in place (it is unused for data) and set a URL. `STATUS_DATABASE_URL` and `DATABASE_URL` are accepted when `HARUVO_DATABASE_URL` is unset. `HARUVO_*` wins over `STATUS_*`.

```bash
export HARUVO_DATABASE_URL=postgres://haruvo:haruvo@127.0.0.1:5432/haruvo
cargo run -p haruvo-server -- serve --listen 127.0.0.1:43123
```

The URL must start with `postgres://` or `postgresql://`. Integer columns are `BIGINT` so one SQL file decodes as `i64` on both engines. A migration is applied once, inside a transaction, and its SHA-256 is stored in `schema_migrations`. A later binary with different SQL for that version refuses to start.

Copy `.env.example` to `.env.local` if you want the variables written down. The process reads the environment directly. `just dev` sources `.env.local` when that file exists.

Email delivery needs SMTP. Without it, an email channel is saved and each send is recorded as skipped:

```bash
export HARUVO_SMTP_HOST=smtp.example.com
export HARUVO_SMTP_PORT=587
export HARUVO_SMTP_FROM=status@example.com
```

Webhook channels can set a secret. The body is posted as JSON with `X-Haruvo-Signature: sha256=<hmac-sha256>`. `X-Status-Signature` is sent with the same value.

### First-run setup

A new database has no open sign-up. The login page does not offer "Create one". On first start the server prints one line to stderr (standard error, not the JSON logs):

```text
Initial setup URL: http://127.0.0.1:43123/setup?token=...
```

Open that URL and create the owner account. That account is the root user. Do not keep a copy of the token. It expires after 30 minutes, and a later start does not print it again while it is still valid. After the owner exists, setup cannot run again and public registration stays closed.

If the token is lost or expired before use, replace it. On SQLite, stop Haruvo first (the server owns the database file). On Postgres the command can run while Haruvo is up:

```bash
haruvo setup-token rotate
# optional: haruvo setup-token rotate --origin https://status.example.com
```

`setup-token init` prints a URL only when setup is unfinished and no unused token is stored. Rotation invalidates the previous unused token.

The root user opens registration in Settings. It stays closed until a mail server is saved there, or `HARUVO_SMTP_HOST` and `HARUVO_SMTP_FROM` are set (`STATUS_SMTP_*` still works), and the root user turns Open registration on. Clearing the saved mail server closes registration again. `HARUVO_OPEN_SIGNUP` and `STATUS_OPEN_SIGNUP` are ignored. Haruvo does not email a sign-up link; a person who can open `/register` chooses their own password. The root user can also mark another account as an installation admin. The root account itself cannot be changed.

Every `HARUVO_*` variable still accepts the older `STATUS_*` name when the `HARUVO_*` variable is unset. See [Upgrading from older names](#upgrading-from-older-names).

When a check cannot connect (DNS, connect, or timeout error), Haruvo first tries a TCP connection to `1.1.1.1:443`, `8.8.8.8:443` and `9.9.9.9:443`. If none of them answer, the problem is the Haruvo host's own network. Haruvo then does not record the check, does not change the monitor state, and sends no alert. The result is cached for 10 seconds. If the host cannot reach the internet by design, set hosts that it can always reach, or turn the check off:

```bash
export HARUVO_CONNECTIVITY_TARGETS=10.0.0.1:53,10.0.0.2:53   # or "off"
```

## Development

```bash
just setup
just dev      # API on 127.0.0.1:43124, Vite on 127.0.0.1:43123
just test
just build    # release binary with the UI embedded
```

`just` is optional. The same commands are `bun install`, `cargo test --workspace`, and `cargo build --release -p haruvo-server` after `bun run build`.

Frontend dev proxies `/api` to the Rust process. Production does not proxy: the binary serves both.

## Tests

```bash
cargo test --workspace
```

Domain tests cover assertions, the failure threshold, uptime math (maintenance is excluded), slugs, and incident resolution. Server tests sign up, run an HTTP check against a local server, open an incident after a failure, publish a status page, skip a check during maintenance, and deliver a signed webhook.

Postgres runs the same migration and API test when these are set:

```bash
export HARUVO_TEST_POSTGRES_URL=postgres://status:status@127.0.0.1:5432/status_platform
export HARUVO_APP_POSTGRES_URL=postgres://status:status@127.0.0.1:5432/status_app
```

Use two empty databases. The platform test resets its own schema. `STATUS_TEST_POSTGRES_URL` and `STATUS_APP_POSTGRES_URL` still work.

## Docker

```bash
docker build -t haruvo:local .
docker run --rm -p 8080:8080 -v haruvo-data:/var/lib/haruvo haruvo:local
```

Pushes to `main` publish `ghcr.io/coollabsio/haruvo` (`linux/amd64` and `linux/arm64`), tagged `latest`, `main`, and the seven-character commit SHA. The same workflow can be run by hand from the Actions tab. If the package is not linked to this repository automatically, link it from the package settings.

```bash
docker pull ghcr.io/coollabsio/haruvo:latest
```

The image listens on `0.0.0.0:8080`, stores SQLite in `/var/lib/haruvo`, and uses `haruvo healthcheck` for the container check. `deploy/compose.yaml` is the same shape as Orbit's compose file, with Postgres left as a commented service. Set `HARUVO_IMAGE=ghcr.io/coollabsio/haruvo:latest` to point that file at the published image.

On a fresh volume the container prints `Initial setup URL` to stderr. Read it from the container logs (`docker logs` or `docker compose logs haruvo`), open it in a browser, and create the owner. Set `HARUVO_PUBLIC_ORIGIN` to the URL you will open before the first start so the printed link uses that host. The token is not stored in the image.

If you miss the URL, stop the container (required for the default SQLite volume; leave Haruvo running when `HARUVO_DATABASE_URL` is Postgres) and issue another:

```bash
docker stop <container>
docker run --rm -v haruvo-data:/var/lib/haruvo \
  -e HARUVO_PUBLIC_ORIGIN=https://status.example.com \
  ghcr.io/coollabsio/haruvo:latest setup-token rotate
```

With Compose, from the repository root:

```bash
docker compose -f deploy/compose.yaml stop haruvo
docker compose -f deploy/compose.yaml run --rm haruvo setup-token rotate
docker compose -f deploy/compose.yaml start haruvo
```

## Agents

An agent checks targets that the Haruvo server cannot reach, for example a database on a private Docker network or a host in a VPC. The agent is the same binary. You start it on the target server or in the same network. It makes only outbound HTTPS requests to the server, so you do not need to open a port into the private network.

The checks are in a config file on the agent host. The server cannot add or change agent checks, so a compromised dashboard cannot make an agent connect to other hosts. Passwords stay on the agent host. The agent judges each check and sends only the status, the status code, the latency, and the error. It never sends a response body.

1. In the web app, open **Agents** and create an agent. Copy the token. The token is shown once.
2. Write the config file, for example `/etc/haruvo/agent.toml`:

   ```toml
   [[check]]
   name = "Main DB"
   type = "postgres"            # http, tcp, postgres, mysql, redis
   target = "localhost:5432"    # a URL for http
   user = "status"
   password_env = "PG_PASSWORD" # the name of an environment variable, not the password
   database = "app"
   interval = 30                # seconds, 15 or more, default 60
   ```

   | Key | Types | Default |
   |---|---|---|
   | `name`, `type`, `target` | all | required |
   | `interval`, `timeout` (seconds), `failure_threshold` | all | 60, 10, 3 |
   | `degraded_after_ms` | all | none |
   | `user`, `password_env`, `database` | postgres, mysql (`user` required); redis (`user` and `password_env` optional) | none |
   | `expected_status`, `body_contains` | http | 2xx, none |

   A `postgres` or `mysql` check logs in and runs `SELECT 1`. A `redis` check sends `AUTH` when a password is set, then `PING`. Redis over TLS is not supported.
3. Start the agent:

   ```bash
   PG_PASSWORD=... haruvo agent --server https://status.example.com --token <token> --config /etc/haruvo/agent.toml
   ```

   The command also reads `HARUVO_AGENT_SERVER`, `HARUVO_AGENT_TOKEN`, and `HARUVO_AGENT_CONFIG`. `STATUS_AGENT_*` still works when the `HARUVO_` name is unset. With Docker, run the image with the `agent` command and `--no-healthcheck`, mount the config file, and attach the container to the network of the targets.

When the agent starts, it sends its list of checks to the server, without secrets. The server creates a monitor for each new check name, updates the monitors that already exist, and pauses the monitors of checks that are no longer in the file. History stays. To change a check, edit the file and restart the agent. The agent stops at once when the server rejects the token or the config, and it shows the reason.

The server records each result with the agent name as the region, and opens or resolves incidents as it does for its own checks. In the dashboard you can pause, resume, and delete agent monitors, and add them to status pages, but you cannot edit them or run them. The agent sends a heartbeat every 15 seconds. It shows as offline when the server has not heard from it for 30 seconds. While an agent is offline, its monitors keep their last state. The agent uses `HARUVO_CONNECTIVITY_TARGETS` in the same way as the server: when its own host has no network, it does not send failed checks.

You cannot delete an agent while it has monitors.

## Errors

**Issues** is the feed (unresolved, resolved, ignored, search, environment, sparkline). Open an issue for the histogram, tags, event navigation, stack trace, breadcrumbs, contexts, and raw JSON. A new event reopens a resolved issue. **Projects** shows the DSN:

```text
http://<public_key>@127.0.0.1:43123/<project_id>
```

SDKs post to `/api/<project_id>/envelope/` and `/api/<project_id>/store/` (with or without the trailing slash). Auth is `X-Sentry-Auth`, `Authorization: Sentry sentry_key=...`, or `?sentry_key=`. Gzip and deflate bodies are accepted. Those routes answer CORS preflight and do not use the dashboard cookie. `/api/<project_id>/security/` is accepted and ignored. Performance items, sessions, and client reports in an envelope are ignored so the SDK still gets HTTP 200.

`examples/` has PHP, Node, browser, and Rust SDK programs. Set `SENTRY_DSN` to the project DSN.

Short-link hostnames only take over a request when the `Host` is a configured link domain and not Haruvo's own origin, and only after `/api` and `/go` have already been routed. Sentry ingest stays on `/api/...`.

## Analytics

Add a site in **Sites**, then paste the snippet from **Tracker**:

```html
<script defer src="https://haruvo.example.com/data.js" data-site="YOUR_PUBLIC_ID"></script>
```

The script posts JSON to `POST /api/receive` on the same origin as `/data.js`. It sends no cookies (`credentials: 'omit'`), allows any origin via CORS, and ignores bots. `window.haruvoEvent(name, data)` records a custom event. `window.beaconEvent` is the same function, so a snippet written for [Beacon](https://github.com/ShadowArcanist/beacons) still works.

Visitor ids rotate monthly. A visit ends after 30 minutes of idle time. Set `HARUVO_PROXY_HOPS` when Haruvo sits behind a reverse proxy (`1` for one proxy, `2` when Cloudflare is in front, which also enables `cf-ipcountry` / `cf-region` / `cf-ipcity`). Point `HARUVO_GEOIP_PATH` at a MaxMind or DB-IP city `.mmdb` if you want country, region, and city without Cloudflare. DB-IP Lite data is published by [DB-IP](https://db-ip.com).

Short links redirect with `307` or `308` and `Cache-Control: no-store`. On Haruvo's own hostname use `/go/{slug}`. On any other hostname that you added under **Links**, the path is the slug. `HEAD` requests and known bots follow the redirect and are not counted.

When public sharing is on, these routes need no session. Counts below the site's threshold are folded into **Other**:

```text
GET /api/public/sites/{slug}/overview?from=<unix>&to=<unix>
GET /api/public/sites/{slug}/{path|referrer|country|browser|os|device}
GET /api/public/links/{slug}/overview
GET /api/public/links/{slug}/{referrer|country|browser|os|device}
```

The matching pages are `/share/sites/{slug}` and `/share/links/{slug}`.

## How this maps to Orbit

Haruvo follows [Orbit](https://github.com/coollabsio/orbit)'s layout and leaves out its realtime stack (websockets, live sync, collaborative editing).

| Orbit | Haruvo |
| --- | --- |
| `apps/server` binary embeds the web app | `apps/server`, binary name `haruvo` |
| `crates/platform` (config, db, auth, migrations) | `crates/platform` |
| `crates/orbit` domain rules | `crates/status` |
| `apps/web` Vite, React, Tailwind | `apps/web` |
| SQLite file, or Postgres via `ORBIT_DATABASE_URL` | SQLite file, or Postgres via `HARUVO_DATABASE_URL` / `STATUS_DATABASE_URL` / `DATABASE_URL` |
| `MigrationRunner`, checksums in `schema_migrations` | same |
| Argon2id passwords, random session tokens | same, sessions stored in the database |
| Initial setup URL, `setup-token rotate`, closed registration | same: one-time `/setup?token=`, then the root user opens registration in Settings after SMTP |
| `platform::jobs` worker | `scheduler`: a lease row plus the check loop |
| `justfile`, multi-stage Dockerfile, `deploy/compose.yaml` | same roles |

The domain follows [OpenStatus](https://github.com/openstatusHQ/openstatus): monitors, check history, status pages and components, incident timelines, maintenance, and a notification provider list. Regions, private locations, and the hosted billing pieces are not in this MVP. Checks run from the Haruvo process itself (`region = local`).

The interface takes Coolify's dark shell and pink primary action, which Orbit already uses, and the public page takes the OpenStatus layout: one banner, a component list, and uptime bars.

## Layout

```
apps/server     API, scheduler, checker, repositories, migrations
apps/web        React dashboard and public status page
crates/platform Config, SQLite/Postgres pool, migrations, passwords, ids
crates/status    Assertions, state machine, uptime, slugs, notification signing
crates/insights  Cookieless visitor ids, URL parsing, user-agent classification
deploy           Docker Compose
```

## Attribution

Cookieless analytics and the link shortener reimplement [Beacon](https://github.com/ShadowArcanist/beacons) by ShadowArcanist, which is MIT licensed. Copyright (c) 2026 ShadowArcanist. The tracker in `apps/server/src/insights/tracker.js` is that script with the Haruvo event name added. See [NOTICE](NOTICE).

## Upgrading from older names

Older `STATUS_*` settings keep working until you rename them to the `HARUVO_` prefix. Haruvo no longer reads any Statixo names.

| Current | Still accepted |
| --- | --- |
| `HARUVO_*` | `STATUS_*` when `HARUVO_*` is unset |
| `haruvo_session` cookie | `status_session`, until the next login replaces it |
| `window.haruvoEvent` | `window.beaconEvent` |
| `X-Haruvo-Signature` | `X-Status-Signature` is sent with the same value |
| `data/haruvo.sqlite`, `/var/lib/haruvo/haruvo.sqlite` | `status.sqlite` in the same directory, when the Haruvo file is absent and the path is a stock default |
| Browser appearance | `haruvo-theme` is still read |

`DATABASE_URL` is still an alias of the database URL, after `HARUVO_DATABASE_URL` and `STATUS_DATABASE_URL`.

The container image sets `HARUVO_ENV`, `HARUVO_BIND`, `HARUVO_PORT`, `HARUVO_DATABASE_PATH`, and `HARUVO_PUBLIC_ORIGIN`. Those win over a `STATUS_*` name of the same setting.

A Statixo database is not found automatically. Point Haruvo at the file:

```bash
export HARUVO_DATABASE_PATH=/var/lib/statixo/statixo.sqlite
```

Or copy it onto the Haruvo path:

```bash
docker run --rm -v statixo-data:/from -v haruvo-data:/to alpine \
  cp -a /from/statixo.sqlite /to/haruvo.sqlite
```

Point Coolify at `ghcr.io/coollabsio/haruvo`. `HARUVO_IMAGE` is the compose override. The source repository is `https://github.com/coollabsio/haruvo`. API problem URLs use `https://haruvo.io/problems/...`.

## License

Apache-2.0. See [LICENSE](LICENSE). Third-party MIT terms for the tracker are in [NOTICE](NOTICE).
