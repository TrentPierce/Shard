#!/usr/bin/env bash
set -euo pipefail

COMPOSE_FILE="${COMPOSE_FILE:-docker-compose.yml}"
SERVICE="${1:-shard-api}"

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required"
  exit 1
fi

echo "[upgrade] pulling latest image/build context for ${SERVICE}"
docker compose -f "${COMPOSE_FILE}" pull "${SERVICE}" || true

echo "[upgrade] recreating ${SERVICE} with zero dependency churn"
docker compose -f "${COMPOSE_FILE}" up -d --no-deps --force-recreate "${SERVICE}"

echo "[upgrade] waiting for healthy state"
for _ in $(seq 1 30); do
  status=$(docker inspect --format='{{.State.Health.Status}}' "${SERVICE}" 2>/dev/null || echo "unknown")
  if [[ "${status}" == "healthy" ]]; then
    echo "[upgrade] ${SERVICE} is healthy"
    exit 0
  fi
  sleep 2
done

echo "[upgrade] ${SERVICE} did not become healthy in time"
exit 1
