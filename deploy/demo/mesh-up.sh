#!/usr/bin/env bash
set -euo pipefail

NODES="${1:-2}"
COMPOSE_FILE="${COMPOSE_FILE:-deploy/demo/docker-compose.mesh.yml}"
MAX_WAIT_SECONDS="${MAX_WAIT_SECONDS:-120}"

echo "Starting bootstrap node..."
docker compose -f "${COMPOSE_FILE}" up -d --build bootstrap >/dev/null

deadline=$((SECONDS + MAX_WAIT_SECONDS))
peer_id=""
while (( SECONDS < deadline )); do
  if health_json="$(curl -fsS http://localhost:19091/health 2>/dev/null)"; then
    peer_id="$(printf '%s' "${health_json}" | sed -n 's/.*"peer_id":"\([^"]*\)".*/\1/p')"
    if [[ -n "${peer_id}" ]]; then
      break
    fi
  fi
  sleep 2
done

if [[ -z "${peer_id}" ]]; then
  echo "Bootstrap node did not report peer_id within ${MAX_WAIT_SECONDS}s" >&2
  exit 1
fi

export SHARD_BOOTSTRAP_NODE="/dns4/bootstrap/tcp/4001/p2p/${peer_id}"
echo "Bootstrap peer: ${SHARD_BOOTSTRAP_NODE}"
echo "Starting shard-node scale=${NODES} ..."
docker compose -f "${COMPOSE_FILE}" up -d --scale shard-node="${NODES}" shard-node >/dev/null

echo
echo "Mesh started."
echo "Gateway health:  http://localhost:19091/health"
echo "Gateway metrics: http://localhost:19091/metrics/summary"
echo "To stop: docker compose -f ${COMPOSE_FILE} down -v"
