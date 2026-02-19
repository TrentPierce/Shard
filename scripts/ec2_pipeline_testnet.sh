#!/usr/bin/env bash
set -euo pipefail

# Phase 2 helper:
# - SSH to EC2
# - clone/update Shard via gh
# - start 3-5 daemon nodes with overlapping layer ranges
# - randomly kill containers to test race-to-first resilience
#
# Usage:
#   scripts/ec2_pipeline_testnet.sh <ec2_host> <ssh_key_path> [five]
#
# Example:
#   scripts/ec2_pipeline_testnet.sh 54.224.107.75 ~/.ssh/ClawdBot.pem five

EC2_HOST="${1:-}"
SSH_KEY="${2:-}"
MODE="${3:-three}"
REMOTE_DIR="/home/ubuntu/Shard"
COMPOSE_FILE="deploy/testnet/docker-compose.pipeline.yml"

if [[ -z "${EC2_HOST}" || -z "${SSH_KEY}" ]]; then
  echo "Usage: $0 <ec2_host> <ssh_key_path> [three|five]"
  exit 1
fi

PROFILE_ARGS=""
if [[ "${MODE}" == "five" ]]; then
  PROFILE_ARGS="--profile five-node"
fi

echo "[1/6] Ensure gh and docker are available on remote host"
ssh -i "${SSH_KEY}" "ubuntu@${EC2_HOST}" "gh --version >/dev/null && docker --version >/dev/null && docker compose version >/dev/null"

echo "[2/6] Clone or update repository via gh"
ssh -i "${SSH_KEY}" "ubuntu@${EC2_HOST}" "
  set -euo pipefail
  if [[ ! -d '${REMOTE_DIR}' ]]; then
    gh repo clone TrentPierce/Shard '${REMOTE_DIR}'
  else
    cd '${REMOTE_DIR}'
    git fetch origin
    git checkout main
    git pull --ff-only origin main
  fi
"

echo "[3/6] Start pipeline testnet"
ssh -i "${SSH_KEY}" "ubuntu@${EC2_HOST}" "
  set -euo pipefail
  cd '${REMOTE_DIR}'
  MODEL_ID='test-gguf' docker compose -f '${COMPOSE_FILE}' ${PROFILE_ARGS} up -d --build
"

echo "[4/6] Show running containers"
ssh -i "${SSH_KEY}" "ubuntu@${EC2_HOST}" "
  set -euo pipefail
  cd '${REMOTE_DIR}'
  docker compose -f '${COMPOSE_FILE}' ${PROFILE_ARGS} ps
"

echo "[5/6] Run random kill stress loop"
ssh -i "${SSH_KEY}" "ubuntu@${EC2_HOST}" "
  set -euo pipefail
  cd '${REMOTE_DIR}'
  for i in \$(seq 1 10); do
    CANDIDATES=\$(docker compose -f '${COMPOSE_FILE}' ${PROFILE_ARGS} ps --services | tr '\n' ' ')
    TARGET=\$(echo \$CANDIDATES | tr ' ' '\n' | grep -E 'node-[abcde]' | shuf -n 1)
    if [[ -n \"\$TARGET\" ]]; then
      echo \"kill iteration \$i: \$TARGET\"
      docker kill \"\$(docker compose -f '${COMPOSE_FILE}' ${PROFILE_ARGS} ps -q \$TARGET)\" || true
      sleep 2
      docker compose -f '${COMPOSE_FILE}' ${PROFILE_ARGS} up -d \$TARGET
    fi
    sleep 4
  done
"

echo "[6/6] Final status"
ssh -i "${SSH_KEY}" "ubuntu@${EC2_HOST}" "
  set -euo pipefail
  cd '${REMOTE_DIR}'
  docker compose -f '${COMPOSE_FILE}' ${PROFILE_ARGS} ps
"

echo "Done."
