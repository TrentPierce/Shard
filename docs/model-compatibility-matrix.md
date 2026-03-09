# Model Compatibility Matrix

Version: `0.6.5`

This matrix defines which draft model and verifier model pairs are allowed for speculative scheduling.
The scheduler consumes these compatibility rules through `shard_verifier::inference::is_model_pair_compatible`.

| Draft Model | Verifier Model | Speculative Supported |
| --- | --- | --- |
| `meta-llama/Llama-3.2-1B` | `meta-llama/Llama-3.2-1B` | Yes |
| `meta-llama/Llama-3.2-1B` | `bitnet-1.58b` | Yes |
| `meta-llama/Llama-3.2-1B` | `verifier-v2` | Yes |
| `legacy-draft-v0` | `verifier-v2` | No |

Notes:
- Runtime normalizes legacy aliases (`shard-hybrid`, `default-model`, `llama-3.2-1b-draft`) to `meta-llama/Llama-3.2-1B` for backward compatibility.
- If a pair is not listed, speculative scheduling is treated as incompatible except identical model IDs.
- Incompatible pairs return no candidates from `/layers/next` and fall back to non-speculative generation in chat runtime.

