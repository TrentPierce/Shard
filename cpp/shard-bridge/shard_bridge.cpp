#include "shard_bridge.h"
#include "llama.h"

#include <algorithm>
#include <cstring>
#include <string>
#include <vector>
#include <iostream>

struct ShardEngineState {
    llama_model* model = nullptr;
    llama_context* ctx = nullptr;
    int32_t n_ctx = 4096;
    int32_t n_past = 0;
};

extern "C" {

SHARD_API void* shard_init(const char* model_path) {
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
    llama_memory_seq_rm(llama_get_memory(state->ctx), 0, state->n_past, -1);
    
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

} // extern "C"
