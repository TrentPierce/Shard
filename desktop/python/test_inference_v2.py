"""Test shard_init + tokenize + eval + decode end-to-end with better diagnostics."""
import ctypes, os, sys

MODEL = r"E:\ggml-model-i2_s.gguf"
DLL   = os.path.join(os.path.dirname(__file__), "shard_engine.dll")

# Redirect C-level stderr to a file
log_path = os.path.join(os.path.dirname(__file__), "init_full_log.txt")
old_stderr_fd = os.dup(2)
log_fd = os.open(log_path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC)
os.dup2(log_fd, 2)

lib = ctypes.CDLL(DLL)

# shard_init
lib.shard_init.argtypes = [ctypes.c_char_p]
lib.shard_init.restype  = ctypes.c_void_p

# shard_tokenize
lib.shard_tokenize.argtypes = [ctypes.c_void_p, ctypes.c_char_p,
                                ctypes.POINTER(ctypes.c_int), ctypes.c_int]
lib.shard_tokenize.restype  = ctypes.c_int

# shard_eval  (takes token array)
lib.shard_eval.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_int), ctypes.c_int]
lib.shard_eval.restype  = ctypes.c_int

# shard_get_logits
lib.shard_get_logits.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_float), ctypes.c_int]
lib.shard_get_logits.restype  = ctypes.c_int

# shard_token_to_piece
lib.shard_token_to_piece.argtypes = [ctypes.c_void_p, ctypes.c_int,
                                      ctypes.c_char_p, ctypes.c_int]
lib.shard_token_to_piece.restype  = ctypes.c_int

# shard_free
lib.shard_free.argtypes = [ctypes.c_void_p]
lib.shard_free.restype  = None

print("Loading model...")
ctx = lib.shard_init(MODEL.encode())

# Restore stderr for print output
os.dup2(old_stderr_fd, 2)
os.close(log_fd)
os.close(old_stderr_fd)

if not ctx:
    print("FAILED: shard_init returned NULL")
    with open(log_path, "r", errors="replace") as f:
        for line in f.readlines()[-30:]:
            print(line, end="")
    sys.exit(1)

print(f"Model loaded! ctx = {ctx:#x}")

# Tokenize prompt
prompt = "Hello, how are you today?"
max_tok = 512
tokens_buf = (ctypes.c_int * max_tok)()
n_tokens = lib.shard_tokenize(ctx, prompt.encode(), tokens_buf, max_tok)
print(f"\nPrompt: {prompt}")
print(f"Tokens ({n_tokens}): {[tokens_buf[i] for i in range(n_tokens)]}")

# Eval prompt tokens
print("\nEvaluating prompt tokens...")
old_stderr_fd = os.dup(2)
log_fd = os.open(log_path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC)
os.dup2(log_fd, 2)

rc = lib.shard_eval(ctx, tokens_buf, n_tokens)

os.dup2(old_stderr_fd, 2)
os.close(log_fd)
os.close(old_stderr_fd)

if rc != 0:
    print(f"shard_eval returned {rc}")
    with open(log_path, "r", errors="replace") as f:
        for line in f.readlines()[-20:]:
            print(line, end="")
    lib.shard_free(ctx)
    sys.exit(1)

print("Prompt evaluated successfully!")

# Generate tokens with top-5 diagnostics
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
    logits_list = [(logits_buf[j], j) for j in range(n)]
    logits_list.sort(reverse=True)
    top5 = logits_list[:5]

    # Show logit stats
    all_vals = [logits_buf[j] for j in range(n)]
    min_val = min(all_vals)
    max_val = max(all_vals)
    avg_val = sum(all_vals) / len(all_vals)

    best_val, best_idx = top5[0]
    generated.append(best_idx)

    # Decode token to text
    piece_buf = ctypes.create_string_buffer(256)
    piece_len = lib.shard_token_to_piece(ctx, best_idx, piece_buf, 256)
    piece = piece_buf.value.decode("utf-8", errors="replace") if piece_len > 0 else f"[{best_idx}]"
    full_text += piece

    if i < 5:
        print(f"\n  Token {i}: id={best_idx}, piece='{piece}'  logit={best_val:.4f}")
        print(f"    Logit range: [{min_val:.4f}, {max_val:.4f}], avg={avg_val:.4f}")
        print(f"    Top-5:")
        for val, idx in top5:
            p_buf = ctypes.create_string_buffer(256)
            p_len = lib.shard_token_to_piece(ctx, idx, p_buf, 256)
            p = p_buf.value.decode("utf-8", errors="replace") if p_len > 0 else f"[{idx}]"
            print(f"      {idx:6d} '{p:20s}' logit={val:.4f}")
    else:
        print(f"  Token {i}: id={best_idx}, piece='{piece}'  logit={best_val:.4f}", end="")
        if i % 4 == 0:
            print()
        else:
            print("  ", end="")

    # Check for EOS (128001)
    if best_idx == 128001:
        print("  (EOS reached)")
        break

    # Feed this token back through eval
    next_tok = (ctypes.c_int * 1)(best_idx)
    old_stderr_fd = os.dup(2)
    log_fd = os.open(log_path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC)
    os.dup2(log_fd, 2)

    rc = lib.shard_eval(ctx, next_tok, 1)

    os.dup2(old_stderr_fd, 2)
    os.close(log_fd)
    os.close(old_stderr_fd)

    if rc != 0:
        print(f"shard_eval returned {rc} on token {i}")
        break

print(f"\n\nGenerated text: \"{full_text}\"")

# Cleanup
lib.shard_free(ctx)
print("\nDone. Model freed.")
