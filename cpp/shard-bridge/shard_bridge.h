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

extern "C" {
// Lifecycle
SHARD_API void* shard_init(const char* model_path);
SHARD_API void shard_free(void* handle);

// Peeking API (mandatory)
SHARD_API int shard_eval(void* handle, const int* tokens, int num_tokens);
SHARD_API int shard_get_logits(void* handle, float* out_buffer, int top_k_size);
SHARD_API int shard_rollback(void* handle, int steps);

// Tokenization
SHARD_API int shard_tokenize(void* handle, const char* text, int* out_tokens, int max_tokens);
SHARD_API int shard_token_to_piece(void* handle, int token_id, char* out_buffer, int buffer_size);

// System health
SHARD_API int shard_get_vram_usage(void* handle);
}
