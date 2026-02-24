# Model Compatibility Matrix

Version: `0.6.1`

This matrix defines which draft model and verifier model pairs are allowed for speculative scheduling.
The scheduler consumes these compatibility rules through `shard_verifier::inference::is_model_pair_compatible`.

| Draft Model | Verifier Model | Speculative Supported |
| --- | --- | --- |
| `shard-hybrid` | `default-model` | Yes |
| `shard-hybrid` | `bitnet-1.58b` | Yes |
| `llama-3.2-1b-draft` | `bitnet-1.58b` | Yes |
| `llama-3.2-1b-draft` | `verifier-v2` | Yes |
| `legacy-draft-v0` | `verifier-v2` | No |

Notes:
- If a pair is not listed, speculative scheduling is treated as incompatible except identical model IDs.
- Incompatible pairs return no candidates from `/layers/next` and fall back to non-speculative generation in chat runtime.
