#!/usr/bin/env bash
set -euo pipefail

MODEL_DIR="${MODEL_DIR:-/data/models}"
MODEL_NAME="${MODEL_NAME:-Llama-3.2-1B-Instruct-Q4_K_M.gguf}"
MODEL_URL="${BITNET_MODEL_URL:-https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf}"
MODEL_SHA256="${BITNET_MODEL_SHA256:-6f85a640a97cf2bf5b8e764087b1e83da0fdb51d7c9fab7d0fece9385611df83}"
MODEL_PATH="${BITNET_MODEL:-${MODEL_DIR}/${MODEL_NAME}}"

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
exec shard-daemon "$@"
