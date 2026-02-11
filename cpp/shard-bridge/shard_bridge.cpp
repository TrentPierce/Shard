#include "include/shard_bridge.h"

#include <algorithm>
#include <cstring>
#include <string>
#include <vector>

// Production note:
// This translation layer is where bitnet.cpp internals should be hard-linked.
// For CI/dev environments without bitnet sources, this file includes a deterministic
// fallback backend so the ABI is stable and testable.

struct ShardHandle {
  std::string model_path;
  std::vector<int> committed_tokens;
  std::vector<int> staged_tokens;
};

extern "C" SHARD_API void* shard_init(const char* model_path) {
  auto* handle = new ShardHandle();
  if (model_path != nullptr) {
    handle->model_path = model_path;
  }
  return handle;
}

extern "C" SHARD_API void shard_free(void* handle) {
  auto* h = static_cast<ShardHandle*>(handle);
  delete h;
}

extern "C" SHARD_API int shard_eval(void* handle, const int* tokens, int num_tokens) {
  if (handle == nullptr || tokens == nullptr || num_tokens < 0) {
    return -1;
  }

  auto* h = static_cast<ShardHandle*>(handle);
  h->staged_tokens.clear();
  h->staged_tokens.reserve(static_cast<size_t>(num_tokens));

  for (int i = 0; i < num_tokens; i++) {
    h->staged_tokens.push_back(tokens[i]);
  }
  return num_tokens;
}

extern "C" SHARD_API int shard_get_logits(void* handle, float* out_buffer, int top_k_size) {
  if (handle == nullptr || out_buffer == nullptr || top_k_size <= 0) {
    return -1;
  }

  auto* h = static_cast<ShardHandle*>(handle);
  std::fill(out_buffer, out_buffer + top_k_size, 0.0f);

  // Deterministic fallback distribution keyed by current sequence length.
  const int seed = static_cast<int>(h->committed_tokens.size() + h->staged_tokens.size());
  for (int i = 0; i < top_k_size; i++) {
    out_buffer[i] = static_cast<float>((seed + i) % 100) / 100.0f;
  }

  return top_k_size;
}

extern "C" SHARD_API int shard_rollback(void* handle, int steps) {
  if (handle == nullptr || steps < 0) {
    return -1;
  }

  auto* h = static_cast<ShardHandle*>(handle);
  const int rollback_count = std::min(steps, static_cast<int>(h->committed_tokens.size()));
  if (rollback_count > 0) {
    h->committed_tokens.resize(h->committed_tokens.size() - static_cast<size_t>(rollback_count));
  }
  h->staged_tokens.clear();
  return rollback_count;
}

extern "C" SHARD_API int shard_get_vram_usage(void* handle) {
  if (handle == nullptr) {
    return -1;
  }

  auto* h = static_cast<ShardHandle*>(handle);
  // Placeholder signal (MiB).
  return 256 + static_cast<int>(h->committed_tokens.size() / 4);
}
