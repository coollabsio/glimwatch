set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# Install frontend dependencies. Rust dependencies are fetched by Cargo.
setup:
    cd apps/web && bun install --frozen-lockfile

# API on 43124, Vite on 43123 with a proxy. SQLite is created on first request.
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -f .env.local ]; then
        set -a
        source .env.local
        set +a
    fi
    cargo run -p coolstatus-server -- serve --listen 127.0.0.1:43124 &
    server_pid=$!
    trap 'kill "$server_pid" 2>/dev/null || true' EXIT
    (cd apps/web && VITE_API_PROXY=http://127.0.0.1:43124 bun run dev)
    wait

# Backend tests, including Postgres when COOLSTATUS_TEST_POSTGRES_URL is set.
test:
    cargo test --workspace

# Production assets embedded in the binary, then the release build.
build:
    cd apps/web && bun run build
    cargo build --release -p coolstatus-server

# Demo account, monitors, and the coolLabs status page.
seed:
    cargo run -p coolstatus-server -- seed
