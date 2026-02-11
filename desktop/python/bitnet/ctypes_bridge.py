"""ctypes bridge for Shard Engine hard shim.

This binds to the guaranteed C-ABI exported by shard_engine:
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


class ShardHandle(ctypes.Structure):
    pass


@dataclass
class ShardEngineConfig:
    lib_path: str
    model_path: str
    top_k_size: int = 8


class ShardEngineRuntime:
    def __init__(self, cfg: ShardEngineConfig) -> None:
        lib = ctypes.CDLL(str(Path(cfg.lib_path).expanduser().resolve()))
        self._lib = lib
        self._top_k_size = cfg.top_k_size

        self._lib.shard_init.argtypes = [ctypes.c_char_p]
        self._lib.shard_init.restype = ctypes.POINTER(ShardHandle)

        self._lib.shard_free.argtypes = [ctypes.POINTER(ShardHandle)]
        self._lib.shard_free.restype = None

        self._lib.shard_eval.argtypes = [
            ctypes.POINTER(ShardHandle),
            ctypes.POINTER(ctypes.c_int),
            ctypes.c_int,
        ]
        self._lib.shard_eval.restype = ctypes.c_int

        self._lib.shard_get_logits.argtypes = [
            ctypes.POINTER(ShardHandle),
            ctypes.POINTER(ctypes.c_float),
            ctypes.c_int,
        ]
        self._lib.shard_get_logits.restype = ctypes.c_int

        self._lib.shard_rollback.argtypes = [ctypes.POINTER(ShardHandle), ctypes.c_int]
        self._lib.shard_rollback.restype = ctypes.c_int

        self._lib.shard_get_vram_usage.argtypes = [ctypes.POINTER(ShardHandle)]
        self._lib.shard_get_vram_usage.restype = ctypes.c_int

        self._handle = self._lib.shard_init(cfg.model_path.encode("utf-8"))
        if not self._handle:
            raise RuntimeError("shard_init returned null")

    def eval_tokens(self, token_ids: list[int]) -> int:
        if not token_ids:
            return 0
        in_buf = (ctypes.c_int * len(token_ids))(*token_ids)
        return int(self._lib.shard_eval(self._handle, in_buf, len(token_ids)))

    def get_logits(self, top_k_size: int | None = None) -> list[float]:
        n = top_k_size or self._top_k_size
        out = (ctypes.c_float * n)(*([0.0] * n))
        got = int(self._lib.shard_get_logits(self._handle, out, n))
        if got < 0:
            return []
        return [float(out[i]) for i in range(got)]

    def get_top_k_tokens(self, k: int = 3) -> list[int]:
        logits = self.get_logits(max(k, self._top_k_size))
        if not logits:
            return []
        ranked = sorted(range(len(logits)), key=lambda idx: logits[idx], reverse=True)
        return ranked[:k]

    def rollback(self, steps: int) -> int:
        return int(self._lib.shard_rollback(self._handle, steps))

    def get_vram_usage(self) -> int:
        return int(self._lib.shard_get_vram_usage(self._handle))

    def sample_token(self) -> int | None:
        top = self.get_top_k_tokens(k=1)
        return top[0] if top else None

    def close(self) -> None:
        if self._handle:
            self._lib.shard_free(self._handle)
            self._handle = None

    def __del__(self) -> None:
        self.close()
