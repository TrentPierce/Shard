"""Thin ctypes bridge for shard/bitnet C ABIs.

This wrapper keeps model state loaded in-process so token verification and
next-token generation can run without subprocess overhead.
"""

from __future__ import annotations

import ctypes
import hashlib
from dataclasses import dataclass
from pathlib import Path


class BitNetHandle(ctypes.Structure):
    pass


@dataclass
class BitNetConfig:
    lib_path: str
    model_path: str
    n_ctx: int = 4096
    n_threads: int = 8
    top_k: int = 32


class BitNetRuntime:
    """Runtime wrapper that supports both legacy and shard bridge symbols."""

    def __init__(self, cfg: BitNetConfig) -> None:
        self._cfg = cfg
        self._lib = ctypes.CDLL(str(Path(cfg.lib_path).expanduser().resolve()))
        self._token_map: dict[str, int] = {}
        self._reverse_token_map: dict[int, str] = {}
        self._last_request_id = None
        self._n_past = 0

        if hasattr(self._lib, "bitnet_model_load"):
            self._abi = "bitnet"
            self._bind_bitnet_abi()
            self._handle = self._lib.bitnet_model_load(
                cfg.model_path.encode("utf-8"),
                cfg.n_ctx,
                cfg.n_threads,
            )
            if not self._handle:
                raise RuntimeError("bitnet_model_load returned null")
        elif hasattr(self._lib, "shard_init"):
            self._abi = "shard"
            self._bind_shard_abi()
            self._handle = self._lib.shard_init(cfg.model_path.encode("utf-8"))
            if not self._handle:
                raise RuntimeError("shard_init returned null")
        else:
            raise RuntimeError("Unsupported BitNet library ABI: missing bitnet_* and shard_* symbols")

    def _bind_bitnet_abi(self) -> None:
        self._lib.bitnet_model_load.argtypes = [ctypes.c_char_p, ctypes.c_int, ctypes.c_int]
        self._lib.bitnet_model_load.restype = ctypes.POINTER(BitNetHandle)
        self._lib.bitnet_model_free.argtypes = [ctypes.POINTER(BitNetHandle)]
        self._lib.bitnet_model_free.restype = None
        self._lib.bitnet_verify_tokens.argtypes = [
            ctypes.POINTER(BitNetHandle),
            ctypes.POINTER(ctypes.c_int),
            ctypes.c_size_t,
            ctypes.POINTER(ctypes.c_int),
        ]
        self._lib.bitnet_verify_tokens.restype = ctypes.c_size_t

    def _bind_shard_abi(self) -> None:
        self._lib.shard_init.argtypes = [ctypes.c_char_p]
        self._lib.shard_init.restype = ctypes.c_void_p
        self._lib.shard_free.argtypes = [ctypes.c_void_p]
        self._lib.shard_free.restype = None
        self._lib.shard_eval.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_int), ctypes.c_int]
        self._lib.shard_eval.restype = ctypes.c_int
        self._lib.shard_get_logits.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_float), ctypes.c_int]
        self._lib.shard_get_logits.restype = ctypes.c_int
        self._lib.shard_tokenize.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.POINTER(ctypes.c_int), ctypes.c_int]
        self._lib.shard_tokenize.restype = ctypes.c_int
        self._lib.shard_token_to_piece.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_int]
        self._lib.shard_token_to_piece.restype = ctypes.c_int
        self._lib.shard_rollback.argtypes = [ctypes.c_void_p, ctypes.c_int]
        self._lib.shard_rollback.restype = ctypes.c_int

    @staticmethod
    def token_id_for_text(text: str) -> int:
        """Legacy mock token ID. Deprecated in favor of real tokenizer."""
        digest = hashlib.sha256(text.encode("utf-8")).digest()
        # Keep token ids positive and bounded for C int transport.
        return int.from_bytes(digest[:4], "little") & 0x7FFFFFFF

    def encode_text(self, text: str) -> list[int]:
        if self._abi == "shard":
            # Use model-specific real tokenizer
            max_toks = len(text) * 4 + 16
            buf = (ctypes.c_int * max_toks)()
            n = self._lib.shard_tokenize(self._handle, text.encode("utf-8"), buf, max_toks)
            tokens = list(buf)[:n] if n > 0 else []
            # shard_tokenize implicitly adds BOS (128000). We must strip it to avoid:
            # 1. Double BOS at start of prompt (since we manually add <|begin_of_text|>)
            # 2. BOS inserted before EVERY generated token in the eval loop
            if tokens and tokens[0] == 128000:
                tokens = tokens[1:]
            return tokens
        
        # Legacy fallback
        ids: list[int] = []
        for token in text.split():
            tok_id = self._token_map.get(token)
            if tok_id is None:
                tok_id = self.token_id_for_text(token)
                self._token_map[token] = tok_id
                self._reverse_token_map[tok_id] = token
            ids.append(tok_id)
        return ids

    def decode_token(self, token_id: int) -> str:
        if self._abi == "shard":
            buf = ctypes.create_string_buffer(128)
            n = self._lib.shard_token_to_piece(self._handle, token_id, buf, 128)
            if n > 0:
                # Use ctypes.string_at to get exactly n bytes (safely handles null-termination)
                raw = ctypes.string_at(buf, n)
                # Decode with replacement for invalid sequences (preserves valid chars)
                dec = raw.decode("utf-8", errors="replace")
                # Llama-3 BPE sometimes (though rarely) adds space at start of pieces
                # But mostly it encodes spaces as separate tokens or as part of the word
                # Let's trust the raw decode for now, but maybe strip if needed?
                # Actually, for Llama 3, best to just return raw decode.
                return dec
            return f"tok_{token_id}"
        return self._reverse_token_map.get(token_id, f"tok_{token_id}")

    
    def verify_prefix(self, generated_text: list[str], draft_text: list[str]) -> tuple[list[str], str | None]:
        if not draft_text:
            return [], None

        if self._abi == "bitnet":
            draft_ids = [self.token_id_for_text(draft_text[0])] # dummy, just keeping signature
            # ... legacy logic ...
            return [], None

        # shard ABI path
        accepted: list[str] = []
        for draft_tok in draft_text:
            expected = self.generate_next_token(generated_text + accepted)
            if expected is None:
                break
            if draft_tok == expected:
                accepted.append(draft_tok)
                self.eval_text(draft_tok)
                continue
            return accepted, expected
        return accepted, None
    def generate_next_token(self, generated_text: list[str]) -> str | None:
        """Generate one deterministic token from model state."""
        if self._abi == "bitnet":
            # Legacy ABI does not expose logits; synthesize deterministic token id from context.
            seed = " ".join(generated_text[-32:])
            token_id = self.token_id_for_text(seed or "<bos>")
            token = f"tok_{token_id % 10000}"
            self._token_map.setdefault(token, token_id)
            self._reverse_token_map.setdefault(token_id, token)
            return token

        # shard ABI path
        # Note: We rely on the caller to have sent relevant context via eval_text or similar.
        # Fetch model's full vocabulary logits to perform argmax
        vocab_size = 128000 
        logits_buf = (ctypes.c_float * vocab_size)()
        read = int(self._lib.shard_get_logits(self._handle, logits_buf, vocab_size))
        if read <= 0:
            return None

        best_idx = max(range(read), key=lambda i: float(logits_buf[i]))
        print(f"DEBUG: best_idx={best_idx} pieces={self.decode_token(best_idx)!r}")
        return self.decode_token(best_idx)

    def eval_text(self, text: str) -> int:
        """Evaluate text into the model's KV cache. Returns number of tokens added."""
        if self._abi != "shard":
            return 0
        ids = self.encode_text(text)
        print(f"DEBUG: eval_text {text!r} -> {ids}")
        if not ids:
            return 0
        in_buf = (ctypes.c_int * len(ids))(*ids)
        rc = self._lib.shard_eval(self._handle, in_buf, len(ids))
        if rc != 0:
            raise RuntimeError(f"shard_eval failed with code {rc}")
        return len(ids)

    def rollback(self, steps: int) -> int:
        """Rollback KV cache state."""
        if self._abi != "shard":
            return 0
        return self._lib.shard_rollback(self._handle, steps)

    def close(self) -> None:
        if not getattr(self, "_handle", None):
            return
        if self._abi == "bitnet":
            self._lib.bitnet_model_free(self._handle)
        else:
            self._lib.shard_free(self._handle)
        self._handle = None

    def __del__(self) -> None:
        self.close()


