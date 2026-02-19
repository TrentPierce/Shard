
import os

path = 'desktop/python/shard_api.py'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

# Target part of the file to replace
old_code_marker = 'async def _local_generate(generated: list[str], prompt: str) -> str | None:'
if old_code_marker not in content:
    print("Could not find Target Marker!")
    # Look for a variation
    old_code_marker = 'async def _local_generate(generated: list[str], prompt: str)'

new_code = """
_session_eval_pos: dict[str, int] = {}

async def _local_generate(generated: list[str], prompt: str, request_id: str) -> str | None:
    runtime = await get_or_load_bitnet()
    if runtime is None:
        return None

    pos = _session_eval_pos.get(request_id, 0)
    if pos == 0:
        # Reset engine state for new request and eval prompt
        runtime.rollback(999999) 
        pos = runtime.eval_text(prompt)
        _session_eval_pos[request_id] = pos

    # Eval any new tokens in generated
    gen_idx = _session_eval_pos.get(f"{request_id}_idx", 0)
    while gen_idx < len(generated):
        runtime.eval_text(generated[gen_idx])
        gen_idx += 1
    _session_eval_pos[f"{request_id}_idx"] = gen_idx

    try:
        return runtime.generate_next_token(generated)
    except Exception:
        # Skip logger to avoid missing import issues in script
        raise
"""

# Find the start and end of the function to replace it cleanly
start = content.find('async def _local_generate')
end = content.find('async def _verify_draft')

if start != -1 and end != -1:
    new_content = content[:start] + new_code + content[end:]
    with open(path, 'w', encoding='utf-8') as f:
        f.write(new_content)
    print("Successfully patched shard_api.py")
else:
    print(f"Failed to find boundaries: start={start}, end={end}")
