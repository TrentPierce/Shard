# Model Pairs for Speculative Decoding

This document lists validated model pair combinations for Shard's speculative decoding. Using compatible model pairs is critical — mismatched models can result in near-zero acceptance rates, making inference *slower* than direct generation.

## Why Model Pair Compatibility Matters

Speculative decoding works by having a small "draft" model generate candidate tokens that a larger "verifier" model validates. This only works well when:

1. **Same tokenizers** — Draft and verifier must use the same tokenizer vocabulary
2. **Similar token distributions** — The draft model's predictions must be close to what the verifier would generate
3. **Compatible architectures** — Both models should have similar prediction patterns

**Warning:** Mixing model families (e.g., Llama + Mistral) typically results in <50% acceptance rates, negating any speed benefit.

---

## Validated Pairs

### Tier 1: Recommended (≥80% acceptance)

| Draft Model | Verifier Model | Acceptance Rate | VRAM (Draft) | VRAM (Verifier) | Use Case |
|------------|----------------|-----------------|---------------|-----------------|----------|
| `Llama-3.2-1B-Instruct` | `Llama-3.1-8B-Instruct` | ~85% | 800MB | 16GB | Desktop browsers + servers |
| `Llama-3.2-1B-Instruct-q4f16_0` | `Llama-3.1-8B-Instruct-q4f16_0` | ~85% | 500MB | 5GB | Low-VRAM servers |
| `Phi-3.5-mini-instruct` | `Phi-3-medium-4k-instruct` | ~82% | 700MB | 14GB | Microsoft ecosystem |

### Tier 2: Supported (60-80% acceptance)

| Draft Model | Verifier Model | Acceptance Rate | VRAM (Draft) | VRAM (Verifier) | Use Case |
|------------|----------------|-----------------|---------------|-----------------|----------|
| `TinyLlama-1.1B-Chat` | `Llama-3.1-8B-Instruct` | ~70% | 700MB | 16GB | Resource-constrained |
| `Qwen2-0.5B-Instruct` | `Qwen2-1.5B-Instruct` | ~75% | 600MB | 3GB | Chinese language |

### Tier 3: Experimental (<60% acceptance - not recommended)

| Draft Model | Verifier Model | Acceptance Rate | Notes |
|------------|----------------|-----------------|-------|
| `Gemma-2B-it` | `Llama-3.1-8B` | ~35% | ⚠️ Different tokenizer |
| `Mistral-7B-Instruct` | `Llama-3.1-8B` | ~40% | ⚠️ Different architecture |

---

## Browser (WebLLM) Compatible Draft Models

These models work with WebLLM in browsers:

| Model ID | Size | Quantization | Browser VRAM |
|----------|------|--------------|--------------|
| `Llama-3.2-1B-Instruct-q4f32_1-MLC` | 1B | Q4 | ~500MB |
| `Llama-3.2-1B-Instruct-q4f16_0-MLC` | 1B | Q4 | ~300MB |
| `Phi-3.5-mini-instruct-q4f16_0-MLC` | 3.8B | Q4 | ~400MB |
| `TinyLlama-1.1B-Chat-v1.0-q4f16_0-MLC` | 1B | Q4 | ~250MB |

---

## Server (GGUF) Compatible Verifier Models

These models work as verifiers on servers with llama.cpp:

| Model ID | Size | Quantization | VRAM |
|----------|------|--------------|------|
| `Llama-3.1-8B-Instruct-Q4_K_M.gguf` | 8B | Q4 | ~5GB |
| `Llama-3.1-70B-Instruct-Q4_K_M.gguf` | 70B | Q4 | ~40GB |
| `Phi-3-medium-4k-instruct-q4f16_0.gguf` | 14B | Q4 | ~8GB |
| `Qwen2-1.5B-Instruct-q4f16_0.gguf` | 1.5B | Q4 | ~1GB |

---

## Configuration

### Setting Model Pairs in Rust Daemon

```rust
// In your config or main.rs
const APPROVED_MODEL_PAIRS: &[(&str, &str)] = &[
    ("Llama-3.2-1B-Instruct", "Llama-3.1-8B-Instruct"),
    ("Phi-3.5-mini-instruct", "Phi-3-medium-4k-instruct"),
    ("TinyLlama-1.1B-Chat", "Llama-3.1-8B-Instruct"),
];
```

### Rejecting Unknown Pairs

When a Scout connects with an unknown model pair, log a warning:

```rust
fn validate_model_pair(draft: &str, verifier: &str) -> bool {
    APPROVED_MODEL_PAIRS.iter().any(|(d, v)| {
        draft.starts_with(d) && verifier.starts_with(v)
    })
}
```

---

## Testing New Pairs

Run the compatibility test to validate any new combination:

```bash
python benchmarks/model-pair-test.py \
    --draft Llama-3.2-1B-Instruct \
    --verifier Llama-3.1-8B-Instruct \
    --num-tests 50
```

The test will output:
- Acceptance rate (should be ≥60%)
- Speedup factor vs baseline
- Recommendation (recommended / not recommended)

---

## Adding New Pairs

To add a new validated pair:

1. Run `model-pair-test.py` with both models
2. Achieve ≥60% acceptance rate in tests
3. Document in this file with:
   - Draft and verifier model IDs
   - Measured acceptance rate
   - VRAM requirements
   - Use case description

---

## Troubleshooting

### Low Acceptance Rate

If you're seeing <50% acceptance:

1. **Check model families** — Are draft and verifier from the same family?
2. **Verify tokenizers** — Run a simple test: does `tokenizer.encode("hello")` produce the same IDs on both models?
3. **Check quantization** — Q4 and Q8 may have different prediction patterns than FP16

### No Speedup

If speculative decoding is slower than direct generation:

- Acceptance rate is likely too low (<60%)
- Try a different model pair
- Consider running verifier without speculative decoding for short prompts
