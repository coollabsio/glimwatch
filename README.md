# CoolStatus

CoolStatus is a self-hosted status page, incident desk, and uptime monitor from [coolLabs](https://github.com/coollabsio), the team behind [Coolify](https://github.com/coollabsio/coolify). One Rust binary serves the API, runs the checks, and hosts the React app. SQLite is the default database. Postgres is the same schema and the same queries, selected with a URL.

![Dashboard](docs/screenshots/dashboard.png)

![Public status page](docs/screenshots/status-page.png)

## What it does

- HTTP and TCP monitors with an interval, a timeout, an expected status, a body assertion, and a latency line that marks a check degraded.
- A scheduler in the API process. Repeated failures move a monitor to down and open an incident. A later success resolves that automatic incident.
- Public status pages on a slug (`/status/coollabs`) with components, 90-day uptime bars, and a banner.
- Incidents with the OpenStatus timeline: investigating, identified, monitoring, resolved.
- Maintenance windows. Checks inside a window are stored and are not counted as downtime.
- Notifications. Email (SMTP) and signed webhooks work now. Slack and Discord are incoming-webhook channels on the same interface.

## Run it

You need Rust 1.99 (see `rust-toolchain.toml`) and [Bun](https://bun.sh) 1.2 or newer.

```bash
cd apps/web && bun install && bun run build
cargo run -p coolstatus-server -- seed
cargo run -p coolstatus-server -- serve --listen 127.0.0.1:43123
```

Open http://127.0.0.1:43123.

`seed` creates a local demo and does nothing if an account already exists:

- email `andras@coollabs.io`
- password `coolstatus-demo`

The database file is `data/coolstatus.sqlite`. Migrations run on startup.

Without Bun on the PATH, `cargo test` still works. `apps/server/build.rs` writes a placeholder page when `apps/web/dist` is missing, so the binary compiles before the UI is built.

### Postgres

Leave the SQLite path in place (it is unused for data) and set a URL. `DATABASE_URL` is accepted as an alias of `COOLSTATUS_DATABASE_URL`.

```bash
export COOLSTATUS_DATABASE_URL=postgres://coolstatus:coolstatus@127.0.0.1:5432/coolstatus
cargo run -p coolstatus-server -- serve --listen 127.0.0.1:43123
```

The URL must start with `postgres://` or `postgresql://`. Integer columns are `BIGINT` so one SQL file decodes as `i64` on both engines. A migration is applied once, inside a transaction, and its SHA-256 is stored in `schema_migrations`. A later binary with different SQL for that version refuses to start.

Copy `.env.example` to `.env.local` if you want the variables written down. The process reads the environment directly. `just dev` sources `.env.local` when that file exists.

Email delivery needs SMTP. Without it, an email channel is saved and each send is recorded as skipped:

```bash
export COOLSTATUS_SMTP_HOST=smtp.example.com
export COOLSTATUS_SMTP_PORT=587
export COOLSTATUS_SMTP_FROM=status@example.com
```

Webhook channels can set a secret. The body is posted as JSON with `X-CoolStatus-Signature: sha256=<hmac-sha256>`.

In production (`COOLSTATUS_ENV=production`) sign-up closes after the first account. Set `COOLSTATUS_OPEN_SIGNUP=true` to leave it open.

## Development

```bash
just setup
just dev      # API on 127.0.0.1:43124, Vite on 127.0.0.1:43123
just test
just build    # release binary with the UI embedded
```

`just` is optional. The same commands are `bun install`, `cargo test --workspace`, and `cargo build --release -p coolstatus-server` after `bun run build`.

Frontend dev proxies `/api` to the Rust process. Production does not proxy: the binary serves both.

## Tests

```bash
cargo test --workspace
```

Domain tests cover assertions, the failure threshold, uptime math (maintenance is excluded), slugs, and incident resolution. Server tests sign up, run an HTTP check against a local server, open an incident after a failure, publish a status page, skip a check during maintenance, and deliver a signed webhook.

Postgres runs the same migration and API test when these are set:

```bash
export COOLSTATUS_TEST_POSTGRES_URL=postgres://coolstatus:coolstatus@127.0.0.1:5432/coolstatus_platform
export COOLSTATUS_APP_POSTGRES_URL=postgres://coolstatus:coolstatus@127.0.0.1:5432/coolstatus_app
```

Use two empty databases. The platform test resets its own schema.

## Docker

```bash
docker build -t coolstatus:local .
docker run --rm -p 8080:8080 -v coolstatus-data:/var/lib/coolstatus coolstatus:local
```

The image listens on `0.0.0.0:8080`, stores SQLite in `/var/lib/coolstatus`, and uses `coolstatus healthcheck` for the container check. `deploy/compose.yaml` is the same shape as Orbit's compose file, with Postgres left as a commented service.

## How this maps to Orbit

CoolStatus follows [Orbit](https://github.com/coollabsio/orbit)'s layout and leaves out its realtime stack (websockets, live sync, collaborative editing).

| Orbit | CoolStatus |
| --- | --- |
| `apps/server` binary embeds the web app | `apps/server`, binary name `coolstatus` |
| `crates/platform` (config, db, auth, migrations) | `crates/platform` |
| `crates/orbit` domain rules | `crates/coolstatus` |
| `apps/web` Vite, React, Tailwind | `apps/web` |
| SQLite file, or Postgres via `ORBIT_DATABASE_URL` | SQLite file, or Postgres via `COOLSTATUS_DATABASE_URL` / `DATABASE_URL` |
| `MigrationRunner`, checksums in `schema_migrations` | same |
| Argon2id passwords, random session tokens | same, sessions stored in the database |
| `platform::jobs` worker | `scheduler`: a lease row plus the check loop |
| `justfile`, multi-stage Dockerfile, `deploy/compose.yaml` | same roles |

The domain follows [OpenStatus](https://github.com/openstatusHQ/openstatus): monitors, check history, status pages and components, incident timelines, maintenance, and a notification provider list. Regions, private locations, and the hosted billing pieces are not in this MVP. Checks run from the CoolStatus process itself (`region = local`).

The interface takes Coolify's dark shell and pink primary action, which Orbit already uses, and the public page takes the OpenStatus layout: one banner, a component list, and uptime bars.

## Layout

```
apps/server     API, scheduler, checker, repositories, migrations
apps/web        React dashboard and public status page
crates/platform Config, SQLite/Postgres pool, migrations, passwords, ids
crates/coolstatus  Assertions, state machine, uptime, slugs, notification signing
deploy          Docker Compose
```

## License

Apache-2.0. See [LICENSE](LICENSE).
