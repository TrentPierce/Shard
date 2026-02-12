from __future__ import annotations

import asyncio
import statistics
import time
from pathlib import Path
import sys

sys.path.append(str(Path(__file__).resolve().parents[1] / "desktop" / "python"))
from inference import cooperative_generate  # noqa: E402


class NullControlPlane:
    async def broadcast_work(self, *_args, **_kwargs):
        return True

    async def try_pop_result(self):
        return None


async def run_once(max_tokens: int = 64) -> float:
    async def local_model_generate(generated: list[str], _prompt: str):
        if len(generated) >= max_tokens:
            return None
        return "tok"

    async def verify_draft(_generated: list[str], draft: list[str]):
        return draft, None

    control = NullControlPlane()
    start = time.perf_counter()
    count = 0
    async for _ in cooperative_generate(
        prompt="benchmark prompt",
        local_model_generate=local_model_generate,
        verify_draft=verify_draft,
        control_plane=control,  # type: ignore[arg-type]
        max_tokens=max_tokens,
    ):
        count += 1

    elapsed = time.perf_counter() - start
    tps = count / elapsed if elapsed > 0 else 0.0
    return tps


async def main() -> None:
    samples = [await run_once() for _ in range(5)]
    print("tokens_per_sec_samples=", [round(s, 2) for s in samples])
    print("tokens_per_sec_avg=", round(statistics.mean(samples), 2))
    print("tokens_per_sec_p50=", round(statistics.median(samples), 2))


if __name__ == "__main__":
    asyncio.run(main())
