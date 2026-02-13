
import os

path = 'desktop/python/bitnet/ctypes_bridge.py'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

# Add debug prints to generate_next_token
old_code = '        best_idx = max(range(read), key=lambda i: float(logits_buf[i]))\n        return self.decode_token(best_idx)'
new_code = '        best_idx = max(range(read), key=lambda i: float(logits_buf[i]))\n        print(f"DEBUG: best_idx={best_idx} pieces={self.decode_token(best_idx)!r}")\n        return self.decode_token(best_idx)'

content = content.replace(old_code, new_code)

# Add debug prints to eval_text
old_eval = '        ids = self.encode_text(text)\n        if not ids:'
new_eval = '        ids = self.encode_text(text)\n        print(f"DEBUG: eval_text {text!r} -> {ids}")\n        if not ids:'

content = content.replace(old_eval, new_eval)

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print("Added debug logging to ctypes_bridge.py")
