"""Test inference with clean output - no stderr redirection."""
import ctypes, os, sys

MODEL = r"E:\ggml-model-i2_s.gguf"
DLL   = os.path.join(os.path.dirname(__file__), "shard_engine.dll")

# Suppress C-level stderr entirely by sending to NUL
devnull_fd = os.open("NUL", os.O_WRONLY)
old_stderr_fd = os.dup(2)
os.dup2(devnull_fd, 2)

lib = ctypes.CDLL(DLL)

lib.shard_init.argtypes = [ctypes.c_char_p]
lib.shard_init.restype  = ctypes.c_void_p
lib.shard_tokenize.argtypes = [ctypes.c_void_p, ctypes.c_char_p,
                                ctypes.POINTER(ctypes.c_int), ctypes.c_int]
lib.shard_tokenize.restype  = ctypes.c_int
lib.shard_eval.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_int), ctypes.c_int]
lib.shard_eval.restype  = ctypes.c_int
lib.shard_get_logits.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_float), ctypes.c_int]
lib.shard_get_logits.restype  = ctypes.c_int
lib.shard_token_to_piece.argtypes = [ctypes.c_void_p, ctypes.c_int,
                                      ctypes.c_char_p, ctypes.c_int]
lib.shard_token_to_piece.restype  = ctypes.c_int
lib.shard_free.argtypes = [ctypes.c_void_p]
lib.shard_free.restype  = None

print("Loading model...", flush=True)
ctx = lib.shard_init(MODEL.encode())

# Restore stderr
os.dup2(old_stderr_fd, 2)
os.close(devnull_fd)
os.close(old_stderr_fd)

if not ctx:
    print("FAILED: shard_init returned NULL")
    sys.exit(1)

print(f"Model loaded! ctx = {ctx:#x}", flush=True)

# Tokenize prompt
prompt = "Hello, how are you today?"
max_tok = 512
tokens_buf = (ctypes.c_int * max_tok)()
n_tokens = lib.shard_tokenize(ctx, prompt.encode(), tokens_buf, max_tok)
print(f"\nPrompt: {prompt}")
print(f"Tokens ({n_tokens}): {[tokens_buf[i] for i in range(n_tokens)]}")

# Suppress stderr for eval
devnull_fd = os.open("NUL", os.O_WRONLY)
old_stderr_fd = os.dup(2)
os.dup2(devnull_fd, 2)

print("\nEvaluating prompt tokens...", flush=True)
rc = lib.shard_eval(ctx, tokens_buf, n_tokens)

os.dup2(old_stderr_fd, 2)
os.close(devnull_fd)
os.close(old_stderr_fd)

if rc != 0:
    print(f"shard_eval returned {rc}")
    lib.shard_free(ctx)
    sys.exit(1)

print("Prompt evaluated successfully!\n")

def get_piece(token_id):
    piece_buf = ctypes.create_string_buffer(256)
    piece_len = lib.shard_token_to_piece(ctx, token_id, piece_buf, 256)
    return piece_buf.value.decode("utf-8", errors="replace") if piece_len > 0 else f"[{token_id}]"

# Generate tokens
n_vocab = 128256
generated = []
full_text = ""

for i in range(32):
    logits_buf = (ctypes.c_float * n_vocab)()
    n = lib.shard_get_logits(ctx, logits_buf, n_vocab)
    if n <= 0:
        print(f"shard_get_logits returned {n}")
        break

    # Find top-5 tokens
    indexed = [(logits_buf[j], j) for j in range(n)]
    indexed.sort(reverse=True)
    top5 = indexed[:5]

    best_val, best_idx = top5[0]
    generated.append(best_idx)
    piece = get_piece(best_idx)
    full_text += piece

    # Show diagnostics for first 5 tokens
    if i < 5:
        all_vals = [logits_buf[j] for j in range(n)]
        min_v = min(all_vals)
        max_v = max(all_vals)
        avg_v = sum(all_vals) / len(all_vals)
        print(f"--- Token {i} ---")
        print(f"  Winner: id={best_idx}, piece='{piece}', logit={best_val:.4f}")
        print(f"  Logit stats: min={min_v:.4f}, max={max_v:.4f}, avg={avg_v:.4f}")
        print(f"  Top-5:")
        for val, idx in top5:
            p = get_piece(idx)
            print(f"    {idx:6d}  '{p}'  logit={val:.4f}")
    else:
        print(f"  Token {i}: '{piece}' (id={best_idx}, logit={best_val:.4f})")

    if best_idx == 128001:
        print("  (EOS reached)")
        break

    # Feed token back - suppress stderr
    next_tok = (ctypes.c_int * 1)(best_idx)
    devnull_fd = os.open("NUL", os.O_WRONLY)
    old_stderr_fd = os.dup(2)
    os.dup2(devnull_fd, 2)

    rc = lib.shard_eval(ctx, next_tok, 1)

    os.dup2(old_stderr_fd, 2)
    os.close(devnull_fd)
    os.close(old_stderr_fd)

    if rc != 0:
        print(f"shard_eval returned {rc} on token {i}")
        break

print(f"\n========================================")
print(f"Full generated text:")
print(f"'{full_text}'")
print(f"========================================")

lib.shard_free(ctx)
print("\nDone. Model freed.")
