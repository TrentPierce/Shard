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

    @staticmethod
    def token_id_for_text(text: str) -> int:
        digest = hashlib.sha256(text.encode("utf-8")).digest()
        # Keep token ids positive and bounded for C int transport.
        return int.from_bytes(digest[:4], "little") & 0x7FFFFFFF

    def encode_text(self, text: str) -> list[int]:
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
        return self._reverse_token_map.get(token_id, f"tok_{token_id}")

    def verify_prefix(self, generated_text: list[str], draft_text: list[str]) -> tuple[list[str], str | None]:
        """Return accepted textual prefix + optional correction token.

        Deterministic verification rule for shard ABI:
        - Encode generated + draft tokens to stable numeric ids.
        - Eval generated context, fetch logits, derive expected token from argmax.
        - Accept draft tokens while they match expected token sequence.
        - On first mismatch, return expected correction token.

        For legacy bitnet ABI, defer to bitnet_verify_tokens.
        """
        if not draft_text:
            return [], None

        if self._abi == "bitnet":
            draft_ids = [self.token_id_for_text(t) for t in draft_text]
            in_buf = (ctypes.c_int * len(draft_ids))(*draft_ids)
            mismatch = ctypes.c_int(-1)
            accepted_len = int(
                self._lib.bitnet_verify_tokens(self._handle, in_buf, len(draft_ids), ctypes.byref(mismatch))
            )
            accepted = draft_text[:accepted_len]
            correction = self.decode_token(int(mismatch.value)) if accepted_len < len(draft_text) and mismatch.value >= 0 else None
            return accepted, correction

        # shard ABI deterministic verification
        accepted: list[str] = []
        context = list(generated_text)
        for draft_tok in draft_text:
            expected = self.generate_next_token(context)
            if expected is None:
                break
            if draft_tok == expected:
                accepted.append(draft_tok)
                context.append(draft_tok)
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
        # Feed only the latest token to avoid re-evaluating full context each step.
        if generated_text:
            token_id = self.token_id_for_text(generated_text[-1])
            in_buf = (ctypes.c_int * 1)(token_id)
            rc = self._lib.shard_eval(self._handle, in_buf, 1)
            if rc != 0:
                raise RuntimeError(f"shard_eval failed with code {rc}")

        logits_buf = (ctypes.c_float * self._cfg.top_k)()
        read = int(self._lib.shard_get_logits(self._handle, logits_buf, self._cfg.top_k))
        if read <= 0:
            return None

        best_idx = max(range(read), key=lambda i: float(logits_buf[i]))
        token = f"tok_{best_idx}"
        token_id = self.token_id_for_text(token)
        self._token_map.setdefault(token, token_id)
        self._reverse_token_map.setdefault(token_id, token)
        return token

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
