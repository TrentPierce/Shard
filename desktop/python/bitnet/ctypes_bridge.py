"""Thin ctypes bridge for bitnet.cpp-style C ABI.

This wrapper keeps the model loaded in-process so verification can run token-by-token
without subprocess overhead.
"""

from __future__ import annotations

import ctypes
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


class BitNetRuntime:
    def __init__(self, cfg: BitNetConfig) -> None:
        lib = ctypes.CDLL(str(Path(cfg.lib_path).expanduser().resolve()))
        self._lib = lib

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

        self._handle = self._lib.bitnet_model_load(
            cfg.model_path.encode("utf-8"),
            cfg.n_ctx,
            cfg.n_threads,
        )
        if not self._handle:
            raise RuntimeError("bitnet_model_load returned null")

    def verify_prefix(self, draft_tokens: list[int]) -> tuple[list[int], list[int]]:
        """Return accepted prefix + one-token correction tail.

        ABI expectation:
        - `bitnet_verify_tokens` writes corrected token into `first_mismatch_out`.
        - Return value is accepted prefix length.
        """
        if not draft_tokens:
            return [], []

        in_buf = (ctypes.c_int * len(draft_tokens))(*draft_tokens)
        mismatch = ctypes.c_int(-1)
        accepted_len = int(
            self._lib.bitnet_verify_tokens(self._handle, in_buf, len(draft_tokens), ctypes.byref(mismatch))
        )

        accepted = draft_tokens[:accepted_len]
        if accepted_len < len(draft_tokens) and mismatch.value >= 0:
            return accepted, [int(mismatch.value)]
        return accepted, []

    def close(self) -> None:
        if self._handle:
            self._lib.bitnet_model_free(self._handle)
            self._handle = None

    def __del__(self) -> None:
        self.close()
