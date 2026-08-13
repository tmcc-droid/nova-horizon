#!/usr/bin/env bash
# Start local development dependencies for Nova Horizon (Unix).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ ! -f .env ]]; then
  echo "Creating .env from .env.example"
  cp .env.example .env
fi

echo "Starting Postgres..."
docker compose up -d postgres

echo "Waiting for Postgres health..."
deadline=$((SECONDS + 60))
until [[ "$(docker inspect --format='{{.State.Health.Status}}' nova-horizon-postgres 2>/dev/null || true)" == "healthy" ]]; do
  if (( SECONDS >= deadline )); then
    echo "Warning: Postgres may still be starting. Check: docker compose ps" >&2
    break
  fi
  sleep 1
done

echo
echo "Next steps:"
echo "  1. cargo run -p game-server -- migrate   # after PR-03"
echo "  2. cargo run -p game-server"
echo "  3. Open client/ in Godot 4 and run the main scene"
echo
echo "Optional Redis: docker compose --profile redis up -d"
