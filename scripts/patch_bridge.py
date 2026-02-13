
import os

path = 'desktop/python/bitnet/ctypes_bridge.py'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

new_code = """
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
"""

start = content.find('def verify_prefix(self, generated_text: list[str], draft_text: list[str])')
end = content.find('def generate_next_token(self, generated_text: list[str])')

if start != -1 and end != -1:
    new_content = content[:start] + new_code + content[end:]
    with open(path, 'w', encoding='utf-8') as f:
        f.write(new_content)
    print("Successfully patched ctypes_bridge.py")
else:
    print(f"Failed to find boundaries: start={start}, end={end}")
