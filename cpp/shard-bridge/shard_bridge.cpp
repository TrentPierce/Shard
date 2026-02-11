#include "shard_bridge.h"

#include <algorithm>
#include <cstring>
#include <string>
#include <vector>

// NOTE:
// This is the hard shim layer for production packaging.
// Replace FakeBitNetState internals with direct bitnet.cpp calls during integration.
// The exported C ABI is stable and must remain backward-compatible.
struct FakeBitNetState {
  std::string model_path;
  std::vector<int> committed_tokens;
  int fake_vram_mb = 1024;
};

extern "C" {

SHARD_API void* shard_init(const char* model_path) {
  if (model_path == nullptr || std::strlen(model_path) == 0) {
    return nullptr;
  }
  auto* state = new FakeBitNetState();
  state->model_path = model_path;
  return state;
}

SHARD_API void shard_free(void* handle) {
  if (handle == nullptr) {
    return;
  }
  auto* state = static_cast<FakeBitNetState*>(handle);
  delete state;
}

SHARD_API int shard_eval(void* handle, const int* tokens, int num_tokens) {
  if (handle == nullptr || (tokens == nullptr && num_tokens > 0) || num_tokens < 0) {
    return -1;
  }
  auto* state = static_cast<FakeBitNetState*>(handle);
  for (int i = 0; i < num_tokens; ++i) {
    state->committed_tokens.push_back(tokens[i]);
  }
  return 0;
}

SHARD_API int shard_get_logits(void* handle, float* out_buffer, int top_k_size) {
  if (handle == nullptr || out_buffer == nullptr || top_k_size <= 0) {
    return -1;
  }

  // Stable deterministic placeholder logits distribution.
  for (int i = 0; i < top_k_size; ++i) {
    out_buffer[i] = 1.0f / static_cast<float>(i + 1);
  }
  return top_k_size;
}

SHARD_API int shard_rollback(void* handle, int steps) {
  if (handle == nullptr || steps < 0) {
    return -1;
  }
  auto* state = static_cast<FakeBitNetState*>(handle);
  if (steps == 0) {
    return 0;
  }
  if (static_cast<size_t>(steps) > state->committed_tokens.size()) {
    state->committed_tokens.clear();
    return 0;
  }
  state->committed_tokens.resize(state->committed_tokens.size() - static_cast<size_t>(steps));
  return 0;
}

SHARD_API int shard_get_vram_usage(void* handle) {
  if (handle == nullptr) {
    return -1;
  }
  auto* state = static_cast<FakeBitNetState*>(handle);
  return state->fake_vram_mb;
}

} // extern "C"
