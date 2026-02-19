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
typedef struct shard_init_config {
    const char* model_path;
    int layer_start;
    int layer_end;
    const char* model_id;
    int pipeline_mode;
} shard_init_config;

typedef struct shard_tensor_view {
    void* data;
    unsigned long long byte_size;
    int n_dims;
    unsigned long long dims[4];
    int dtype; // 1 = f32 (current supported type)
} shard_tensor_view;

// Lifecycle
SHARD_API void* shard_init(const char* model_path);
SHARD_API void* shard_init_ex(const shard_init_config* config);
SHARD_API void shard_free(void* handle);

// Peeking API (mandatory)
SHARD_API int shard_eval(void* handle, const int* tokens, int num_tokens);
SHARD_API int shard_get_logits(void* handle, float* out_buffer, int top_k_size);
SHARD_API int shard_rollback(void* handle, int steps);
SHARD_API int shard_get_layer_range(void* handle, int* out_layer_start, int* out_layer_end);
SHARD_API int shard_eval_hidden(void* handle, const shard_tensor_view* in_tensor, shard_tensor_view* out_tensor);

// Tokenization
SHARD_API int shard_tokenize(void* handle, const char* text, int* out_tokens, int max_tokens);
SHARD_API int shard_token_to_piece(void* handle, int token_id, char* out_buffer, int buffer_size);

// System health
SHARD_API int shard_get_vram_usage(void* handle);

// Fault-tolerant KV cache snapshots
// Returns the exact serialized snapshot size in bytes (including metadata header), or < 0 on error.
SHARD_API int shard_kv_snapshot_size(void* handle);
// Serializes the current decoding state into caller-owned buffer.
// Returns bytes written, -2 when max_snapshot_bytes safety limit is exceeded,
// -3 when out_buffer is too small, and <0 for generic errors.
SHARD_API int shard_kv_snapshot_export(
    void* handle,
    unsigned char* out_buffer,
    int out_buffer_size,
    int max_snapshot_bytes
);
// Restores a previously exported snapshot. Returns 0 on success or <0 on error.
SHARD_API int shard_kv_snapshot_import(
    void* handle,
    const unsigned char* snapshot_data,
    int snapshot_size,
    int max_snapshot_bytes
);
}
