# Haruvo

Haruvo is a self-hosted status page, incident desk, and uptime monitor from [coolLabs](https://github.com/coollabsio), the team behind [Coolify](https://github.com/coollabsio/coolify). One Rust binary serves the API, runs the checks, and hosts the React app. SQLite is the default database. Postgres is the same schema and the same queries, selected with a URL.

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

## Run it

You need Rust 1.99 (see `rust-toolchain.toml`) and [Bun](https://bun.sh) 1.2 or newer.

```bash
cd apps/web && bun install && bun run build
cargo run -p haruvo-server -- seed
cargo run -p haruvo-server -- serve --listen 127.0.0.1:43123
```

Open http://127.0.0.1:43123.

`seed` creates a local demo and does nothing if an account already exists:

- email `test@example.com`
- password `password`
- a coolLabs status page, a demo analytics site (`coolify.io`, public id `pub_coollabs_demo`), and short links at `/go/coolify` and `/go/docs`

`just dev` runs `seed` before it starts the server.

The database file is `data/haruvo.sqlite`. If that file is missing and `data/status.sqlite` is already there, Haruvo keeps using the older file. Migrations run on startup.

Without Bun on the PATH, `cargo test` still works. `apps/server/build.rs` writes a placeholder page when `apps/web/dist` is missing, so the binary compiles before the UI is built.

### Postgres

Leave the SQLite path in place (it is unused for data) and set a URL. `DATABASE_URL` and `STATUS_DATABASE_URL` are accepted as aliases of `HARUVO_DATABASE_URL`.

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

Webhook channels can set a secret. The body is posted as JSON with `X-Haruvo-Signature: sha256=<hmac-sha256>`. The previous `X-Status-Signature` header is sent as well.

In production (`HARUVO_ENV=production`) sign-up closes after the first account. Set `HARUVO_OPEN_SIGNUP=true` to leave it open.

Every `HARUVO_*` variable still accepts the older `STATUS_*` name when the new one is unset.

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

The image listens on `0.0.0.0:8080`, stores SQLite in `/var/lib/haruvo`, and uses `haruvo healthcheck` for the container check. `deploy/compose.yaml` is the same shape as Orbit's compose file, with Postgres left as a commented service.

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

   The command also reads `HARUVO_AGENT_SERVER`, `HARUVO_AGENT_TOKEN`, and `HARUVO_AGENT_CONFIG` (`STATUS_AGENT_*` still works). With Docker, run the image with the `agent` command and `--no-healthcheck`, mount the config file, and attach the container to the network of the targets.

When the agent starts, it sends its list of checks to the server, without secrets. The server creates a monitor for each new check name, updates the monitors that already exist, and pauses the monitors of checks that are no longer in the file. History stays. To change a check, edit the file and restart the agent. The agent stops at once when the server rejects the token or the config, and it shows the reason.

The server records each result with the agent name as the region, and opens or resolves incidents as it does for its own checks. In the dashboard you can pause, resume, and delete agent monitors, and add them to status pages, but you cannot edit them or run them. The agent sends a heartbeat every 15 seconds. It shows as offline when the server has not heard from it for 30 seconds. While an agent is offline, its monitors keep their last state. The agent uses `HARUVO_CONNECTIVITY_TARGETS` in the same way as the server: when its own host has no network, it does not send failed checks.

You cannot delete an agent while it has monitors.

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
| SQLite file, or Postgres via `ORBIT_DATABASE_URL` | SQLite file, or Postgres via `HARUVO_DATABASE_URL` / `DATABASE_URL` |
| `MigrationRunner`, checksums in `schema_migrations` | same |
| Argon2id passwords, random session tokens | same, sessions stored in the database |
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

## License

Apache-2.0. See [LICENSE](LICENSE). Third-party MIT terms for the tracker are in [NOTICE](NOTICE).
