set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# Install frontend dependencies. Rust dependencies are fetched by Cargo.
setup:
    cd apps/web && bun install --frozen-lockfile

# API on 43124, Vite on 43123 with a proxy. Seeds the demo account on an empty database.
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -f .env.local ]; then
        set -a
        source .env.local
        set +a
    fi
    cargo run -p owlee-server -- seed
    cargo run -p owlee-server -- serve --listen 127.0.0.1:43124 &
    server_pid=$!
    trap 'kill "$server_pid" 2>/dev/null || true' EXIT
    (cd apps/web && VITE_API_PROXY=http://127.0.0.1:43124 bun run dev)
    wait

# Backend tests, including Postgres when OWLEE_TEST_POSTGRES_URL is set.
test:
    cargo test --workspace

# Production assets embedded in the binary, then the release build.
build:
    cd apps/web && bun run build
    cargo build --release -p owlee-server

# Demo account, monitors, status page, analytics site, short links, and error events.
seed:
    cargo run -p owlee-server -- seed

# Make a migration file with a UTC timestamp version, e.g. `just new-migration add_tags`.
new-migration name:
    #!/usr/bin/env bash
    set -euo pipefail
    [[ "{{name}}" =~ ^[a-z0-9_]+$ ]] || { echo "use lowercase letters, digits and _ in the name"; exit 1; }
    file="apps/server/migrations/$(date -u +%Y%m%d%H%M%S)_{{name}}.sql"
    echo "-- One SQL file for SQLite and Postgres." > "$file"
    echo "$file"
