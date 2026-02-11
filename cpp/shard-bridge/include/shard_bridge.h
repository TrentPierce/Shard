#pragma once

#if defined(_WIN32)
  #if defined(SHARD_ENGINE_BUILD)
    #define SHARD_API __declspec(dllexport)
  #else
    #define SHARD_API __declspec(dllimport)
  #endif
#else
  #define SHARD_API __attribute__((visibility("default")))
#endif

#ifdef __cplusplus
extern "C" {
#endif

// Lifecycle
SHARD_API void* shard_init(const char* model_path);
SHARD_API void shard_free(void* handle);

// Peeking API
SHARD_API int shard_eval(void* handle, const int* tokens, int num_tokens);
SHARD_API int shard_get_logits(void* handle, float* out_buffer, int top_k_size);
SHARD_API int shard_rollback(void* handle, int steps);

// System health
SHARD_API int shard_get_vram_usage(void* handle);

#ifdef __cplusplus
}
#endif
