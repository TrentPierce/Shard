"""Hard-linked ctypes bridge to shard_engine C ABI.

This module intentionally fails fast if required symbols are unavailable.
Expected exported symbols:
- shard_init
- shard_free
- shard_eval
- shard_get_logits
- shard_rollback
- shard_get_vram_usage
"""

from __future__ import annotations

import ctypes
from dataclasses import dataclass
from pathlib import Path


@dataclass
class BitNetConfig:
    lib_path: str
    model_path: str
    top_k_size: int = 3


class BitNetRuntime:
    def __init__(self, cfg: BitNetConfig) -> None:
        self._cfg = cfg
        self._lib = ctypes.CDLL(str(Path(cfg.lib_path).expanduser().resolve()))

        # Mandatory ABI bindings (fail-fast).
        self._lib.shard_init.argtypes = [ctypes.c_char_p]
        self._lib.shard_init.restype = ctypes.c_void_p

        self._lib.shard_free.argtypes = [ctypes.c_void_p]
        self._lib.shard_free.restype = None

        self._lib.shard_eval.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_int), ctypes.c_int]
        self._lib.shard_eval.restype = ctypes.c_int

        self._lib.shard_get_logits.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_float), ctypes.c_int]
        self._lib.shard_get_logits.restype = ctypes.c_int

        self._lib.shard_rollback.argtypes = [ctypes.c_void_p, ctypes.c_int]
        self._lib.shard_rollback.restype = ctypes.c_int

        self._lib.shard_get_vram_usage.argtypes = [ctypes.c_void_p]
        self._lib.shard_get_vram_usage.restype = ctypes.c_int

        self._handle = self._lib.shard_init(cfg.model_path.encode("utf-8"))
        if not self._handle:
            raise RuntimeError("shard_init failed")

    def eval_tokens(self, tokens: list[int]) -> None:
        if not tokens:
            return
        buf = (ctypes.c_int * len(tokens))(*tokens)
        rc = int(self._lib.shard_eval(self._handle, buf, len(tokens)))
        if rc != 0:
            raise RuntimeError(f"shard_eval failed with code={rc}")

    def get_top_k_tokens(self, k: int = 3) -> list[int]:
        out = (ctypes.c_float * k)()
        n = int(self._lib.shard_get_logits(self._handle, out, k))
        if n <= 0:
            raise RuntimeError("shard_get_logits failed")

        # Placeholder ID mapping until tokenizer-prob index mapping is integrated.
        # We return stable ranked pseudo-token ids for loop scaffolding.
        return list(range(n))

    def commit_token(self, token: int) -> None:
        self.eval_tokens([token])

    def rollback(self, n_tokens: int) -> None:
        rc = int(self._lib.shard_rollback(self._handle, n_tokens))
        if rc != 0:
            raise RuntimeError(f"shard_rollback failed with code={rc}")

    def sample_token(self) -> int:
        # Placeholder deterministic sample from ranked list.
        return self.get_top_k_tokens(k=max(1, self._cfg.top_k_size))[0]

    def verify_prefix(self, draft_tokens: list[int]) -> tuple[list[int], list[int]]:
        if not draft_tokens:
            return [], []
        accepted: list[int] = []
        for i, token in enumerate(draft_tokens):
            top_k = self.get_top_k_tokens(k=self._cfg.top_k_size)
            if token in top_k:
                self.commit_token(token)
                accepted.append(token)
                continue
            corrected = self.sample_token()
            self.rollback(len(draft_tokens) - i)
            return accepted, [corrected]
        return accepted, []

    def get_vram_usage_mb(self) -> int:
        mb = int(self._lib.shard_get_vram_usage(self._handle))
        if mb < 0:
            raise RuntimeError("shard_get_vram_usage failed")
        return mb

    def close(self) -> None:
        if self._handle:
            self._lib.shard_free(self._handle)
            self._handle = None

    def __del__(self) -> None:
        self.close()
