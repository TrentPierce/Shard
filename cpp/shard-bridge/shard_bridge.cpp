#include "shard_bridge.h"
#include "llama.h"

#include <algorithm>
#include <cstdint>
#include <cstring>
#include <limits>
#include <string>
#include <vector>
#include <iostream>

struct ShardEngineState {
    llama_model* model = nullptr;
    llama_context* ctx = nullptr;
    int32_t n_ctx = 4096;
    int32_t n_past = 0;
    int32_t n_layer_total = 0;
    int32_t layer_start = 0;
    int32_t layer_end = 0;
    std::string model_id;
    int32_t pipeline_mode = 0;
};

namespace {

constexpr uint32_t kShardSnapshotMagic = 0x50414E53; // "SNAP" little-endian
constexpr uint32_t kShardSnapshotVersion = 1;

struct ShardSnapshotHeader {
    uint32_t magic;
    uint32_t version;
    uint32_t n_past;
    uint32_t payload_size;
};

} // namespace

extern "C" {

SHARD_API void* shard_init(const char* model_path) {
    shard_init_config cfg{};
    cfg.model_path = model_path;
    cfg.layer_start = 0;
    cfg.layer_end = -1;
    cfg.model_id = "";
    cfg.pipeline_mode = 0;
    return shard_init_ex(&cfg);
}

SHARD_API void* shard_init_ex(const shard_init_config* config) {
    if (config == nullptr) {
        return nullptr;
    }
    const char* model_path = config->model_path;
    if (model_path == nullptr || std::strlen(model_path) == 0) {
        return nullptr;
    }

    llama_backend_init();

    llama_model_params mparams = llama_model_default_params();
    mparams.use_mmap = false;
    mparams.use_mlock = false;
    mparams.n_gpu_layers = 0; 

    llama_model* model = llama_model_load_from_file(model_path, mparams);
    if (!model) {
        return nullptr;
    }

    llama_context_params cparams = llama_context_default_params();
    cparams.n_ctx = 4096;
    cparams.n_threads = 8;
    cparams.n_threads_batch = 8;

    llama_context* ctx = llama_init_from_model(model, cparams);
    if (!ctx) {
        llama_model_free(model);
        return nullptr;
    }

    auto* state = new ShardEngineState();
    state->model = model;
    state->ctx = ctx;
    state->n_ctx = cparams.n_ctx;
    state->n_past = 0;
    state->n_layer_total = llama_model_n_layer(model);
    state->layer_start = std::max(0, config->layer_start);
    state->layer_end = config->layer_end < 0 ? (state->n_layer_total - 1) : config->layer_end;
    if (state->layer_end >= state->n_layer_total) {
        state->layer_end = state->n_layer_total - 1;
    }
    if (state->layer_start > state->layer_end) {
        state->layer_start = 0;
        state->layer_end = state->n_layer_total - 1;
    }
    state->model_id = config->model_id ? config->model_id : "";
    state->pipeline_mode = config->pipeline_mode;

    return state;
}

SHARD_API void shard_free(void* handle) {
    if (handle == nullptr) return;
    auto* state = static_cast<ShardEngineState*>(handle);
    if (state->ctx) llama_free(state->ctx);
    if (state->model) llama_model_free(state->model);
    delete state;
    llama_backend_free();
}

SHARD_API int shard_eval(void* handle, const int* tokens, int num_tokens) {
    if (handle == nullptr || tokens == nullptr || num_tokens <= 0) {
        return -1;
    }
    auto* state = static_cast<ShardEngineState*>(handle);

    // Use the helper for simple single-sequence decoding
    // Note: llama_batch_get_one takes a non-const pointer, so we cast (it does not modify the data)
    llama_batch batch = llama_batch_get_one(const_cast<llama_token*>(tokens), num_tokens);

    if (llama_decode(state->ctx, batch) != 0) {
        return -2;
    }

    state->n_past += num_tokens;
    return 0;
}

SHARD_API int shard_get_logits(void* handle, float* out_buffer, int top_k_size) {
    if (handle == nullptr || out_buffer == nullptr || top_k_size <= 0) {
        return -1;
    }
    auto* state = static_cast<ShardEngineState*>(handle);

    const float* logits = llama_get_logits(state->ctx);
    int n_vocab = llama_vocab_n_tokens(llama_model_get_vocab(state->model));
    
    // Copy top_k logits. Note: Shard assumes out_buffer is large enough for top_k_size.
    // In a real implementation we'd sort these, but Shard's cooperative loop does its own sorting.
    // We just provide the raw logits for the first 'top_k_size' tokens for now
    // OR we provide the actual top scoring ones.
    
    // For now, copy first N logits to satisfy the interface.
    // The Python side handles the actual argmax/top-k logic.
    int to_copy = std::min(top_k_size, n_vocab);
    std::memcpy(out_buffer, logits, to_copy * sizeof(float));

    return to_copy;
}

SHARD_API int shard_rollback(void* handle, int steps) {
    if (handle == nullptr || steps < 0) return -1;
    auto* state = static_cast<ShardEngineState*>(handle);
    state->n_past = std::max(0, state->n_past - steps);
    
    // Clear KV cache for those positions
    // Using seq_id=-1 acts as wildcard to clear across all sequences
    // p1=-1 means "to end of sequence"
    llama_memory_seq_rm(llama_get_memory(state->ctx), -1, state->n_past, -1);
    
    return 0;
}

SHARD_API int shard_get_layer_range(void* handle, int* out_layer_start, int* out_layer_end) {
    if (handle == nullptr || out_layer_start == nullptr || out_layer_end == nullptr) {
        return -1;
    }
    auto* state = static_cast<ShardEngineState*>(handle);
    *out_layer_start = state->layer_start;
    *out_layer_end = state->layer_end;
    return 0;
}

SHARD_API int shard_eval_hidden(void* handle, const shard_tensor_view* in_tensor, shard_tensor_view* out_tensor) {
    if (handle == nullptr || in_tensor == nullptr || out_tensor == nullptr) {
        return -1;
    }
    auto* state = static_cast<ShardEngineState*>(handle);
    if (in_tensor->dtype != 1 || out_tensor->dtype != 1) {
        return -2;
    }
    if (in_tensor->n_dims < 2 || in_tensor->n_dims > 4 || in_tensor->data == nullptr || out_tensor->data == nullptr) {
        return -3;
    }

    const int32_t n_embd = llama_model_n_embd(state->model);
    const uint64_t tokens = in_tensor->dims[0];
    const uint64_t hidden = in_tensor->dims[1];
    if (tokens == 0 || hidden != static_cast<uint64_t>(n_embd)) {
        return -4;
    }

    const uint64_t required_bytes = tokens * hidden * sizeof(float);
    if (in_tensor->byte_size < required_bytes || out_tensor->byte_size < required_bytes) {
        return -5;
    }

    // Note: this executes the model from embedding input. The layer-range slicing
    // metadata is tracked in state for pipeline routing and future load-pruning.
    llama_batch batch{};
    batch.n_tokens = static_cast<int32_t>(tokens);
    batch.token = nullptr;
    batch.embd = static_cast<float*>(in_tensor->data);
    batch.pos = nullptr;
    batch.n_seq_id = nullptr;
    batch.seq_id = nullptr;
    batch.logits = nullptr;

    if (llama_decode(state->ctx, batch) != 0) {
        return -6;
    }

    float* emb = llama_get_embeddings(state->ctx);
    if (emb == nullptr) {
        return -7;
    }

    std::memcpy(out_tensor->data, emb, static_cast<size_t>(required_bytes));
    out_tensor->n_dims = in_tensor->n_dims;
    std::memcpy(out_tensor->dims, in_tensor->dims, sizeof(out_tensor->dims));
    out_tensor->byte_size = required_bytes;
    return 0;
}

SHARD_API int shard_tokenize(void* handle, const char* text, int* out_tokens, int max_tokens) {
    if (handle == nullptr || text == nullptr || out_tokens == nullptr) return -1;
    auto* state = static_cast<ShardEngineState*>(handle);
    
    // Use the model's vocab for real tokenization
    const struct llama_vocab * vocab = llama_model_get_vocab(state->model);
    int n_tokens = llama_tokenize(vocab, text, strlen(text), (llama_token*)out_tokens, max_tokens, true, true);
    return n_tokens;
}

SHARD_API int shard_token_to_piece(void* handle, int token_id, char* out_buffer, int buffer_size) {
    if (handle == nullptr || out_buffer == nullptr) return -1;
    auto* state = static_cast<ShardEngineState*>(handle);

    const struct llama_vocab * vocab = llama_model_get_vocab(state->model);
    int n = llama_token_to_piece(vocab, token_id, out_buffer, buffer_size, 0, true);
    if (n > 0 && n < buffer_size) {
        out_buffer[n] = '\0';
    }
    return n;
}

SHARD_API int shard_get_vram_usage(void* handle) {
    if (handle == nullptr) return -1;
    // placeholder: llama.cpp doesn't expose a simple "current vram used" easily without more plumbing
    return 0;
}

SHARD_API int shard_kv_snapshot_size(void* handle) {
    if (handle == nullptr) {
        return -1;
    }

    auto* state = static_cast<ShardEngineState*>(handle);
    const size_t payload_size = llama_state_get_size(state->ctx);
    const size_t total_size = sizeof(ShardSnapshotHeader) + payload_size;
    if (total_size > static_cast<size_t>(std::numeric_limits<int>::max())) {
        return -2;
    }
    return static_cast<int>(total_size);
}

SHARD_API int shard_kv_snapshot_export(
    void* handle,
    unsigned char* out_buffer,
    int out_buffer_size,
    int max_snapshot_bytes
) {
    if (handle == nullptr || out_buffer == nullptr || out_buffer_size <= 0 || max_snapshot_bytes <= 0) {
        return -1;
    }

    auto* state = static_cast<ShardEngineState*>(handle);
    const size_t payload_size = llama_state_get_size(state->ctx);
    const size_t total_size = sizeof(ShardSnapshotHeader) + payload_size;

    if (total_size > static_cast<size_t>(max_snapshot_bytes)) {
        return -2;
    }
    if (total_size > static_cast<size_t>(out_buffer_size)) {
        return -3;
    }

    ShardSnapshotHeader header {
        kShardSnapshotMagic,
        kShardSnapshotVersion,
        static_cast<uint32_t>(state->n_past),
        static_cast<uint32_t>(payload_size),
    };
    std::memcpy(out_buffer, &header, sizeof(header));

    const size_t written = llama_state_get_data(
        state->ctx,
        out_buffer + sizeof(header),
        payload_size
    );
    if (written != payload_size) {
        return -4;
    }

    return static_cast<int>(sizeof(header) + written);
}

SHARD_API int shard_kv_snapshot_import(
    void* handle,
    const unsigned char* snapshot_data,
    int snapshot_size,
    int max_snapshot_bytes
) {
    if (handle == nullptr || snapshot_data == nullptr || snapshot_size <= static_cast<int>(sizeof(ShardSnapshotHeader)) || max_snapshot_bytes <= 0) {
        return -1;
    }
    if (snapshot_size > max_snapshot_bytes) {
        return -2;
    }

    auto* state = static_cast<ShardEngineState*>(handle);

    ShardSnapshotHeader header {};
    std::memcpy(&header, snapshot_data, sizeof(header));
    if (header.magic != kShardSnapshotMagic || header.version != kShardSnapshotVersion) {
        return -3;
    }

    const size_t payload_size = static_cast<size_t>(header.payload_size);
    const size_t expected_size = sizeof(ShardSnapshotHeader) + payload_size;
    if (expected_size != static_cast<size_t>(snapshot_size)) {
        return -4;
    }

    const size_t loaded = llama_state_set_data(
        state->ctx,
        snapshot_data + sizeof(ShardSnapshotHeader),
        payload_size
    );
    if (loaded != payload_size) {
        return -5;
    }

    state->n_past = static_cast<int32_t>(header.n_past);
    return 0;
}

} // extern "C"
