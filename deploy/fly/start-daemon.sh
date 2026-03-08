#!/usr/bin/env bash
set -euo pipefail

MODEL_DIR="${MODEL_DIR:-/data/models}"
MODEL_NAME="${MODEL_NAME:-Llama-3.2-1B-Instruct-Q4_K_M.gguf}"
MODEL_URL="${BITNET_MODEL_URL:-https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf}"
MODEL_SHA256="${BITNET_MODEL_SHA256:-6f85a640a97cf2bf5b8e764087b1e83da0fdb51d7c9fab7d0fece9385611df83}"
MODEL_PATH="${BITNET_MODEL:-${MODEL_DIR}/${MODEL_NAME}}"
PUBLIC_HOST_VALUE="${PUBLIC_HOST:-}"
if [[ -z "${PUBLIC_HOST_VALUE}" && -n "${FLY_APP_NAME:-}" ]]; then
  PUBLIC_HOST_VALUE="${FLY_APP_NAME}.fly.dev"
fi

mkdir -p "$(dirname "${MODEL_PATH}")"
export BITNET_MODEL="${MODEL_PATH}"
export BITNET_LIB="${BITNET_LIB:-/opt/shard/lib/libshard_engine.so}"

if [[ ! -s "${MODEL_PATH}" ]]; then
  tmp_path="${MODEL_PATH}.download"
  echo "[fly-start] downloading verifier model to ${MODEL_PATH}"
  rm -f "${tmp_path}"
  wget -O "${tmp_path}" "${MODEL_URL}"
  if [[ -n "${MODEL_SHA256}" ]]; then
    echo "${MODEL_SHA256}  ${tmp_path}" | sha256sum -c -
  fi
  mv "${tmp_path}" "${MODEL_PATH}"
fi

echo "[fly-start] starting shard-daemon with model ${MODEL_PATH}"
args=(
  --public-api
  --control-port "${CONTROL_PORT:-9091}"
  --telemetry-ws-port "${TELEMETRY_WS_PORT:-9093}"
  --tcp-port "${TCP_PORT:-4001}"
  --webrtc-port "${WEBRTC_PORT:-9090}"
  --quic-port "${QUIC_PORT:-9092}"
)

if [[ -n "${PUBLIC_HOST_VALUE}" ]]; then
  args+=(--public-host "${PUBLIC_HOST_VALUE}")
fi

if [[ -n "${SHARD_DEFAULT_BOOTSTRAP:-}" ]]; then
  IFS=',' read -r -a bootstrap_nodes <<< "${SHARD_DEFAULT_BOOTSTRAP}"
  for node in "${bootstrap_nodes[@]}"; do
    trimmed="$(echo "${node}" | xargs)"
    if [[ -n "${trimmed}" ]]; then
      args+=(--bootstrap-node "${trimmed}")
    fi
  done
fi

exec shard-daemon "${args[@]}" "$@"
