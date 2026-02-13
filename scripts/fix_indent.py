
import os

path = 'desktop/python/bitnet/ctypes_bridge.py'
with open(path, 'r', encoding='utf-8') as f:
    lines = f.readlines()

new_lines = []
in_class = False

for line in lines:
    stripped = line.lstrip()
    if stripped.startswith('class BitNetRuntime:'):
        in_class = True
        new_lines.append(line)
        continue
    
    # If we are in the class and see a def at col 0, indent it
    if in_class and line.startswith('def '):
        new_lines.append('    ' + line)
    # Also indentation of the content of the function if it was messed up
    elif in_class and line.startswith('        '): # already indented too much?
        # Let's just do a simpler approach:
        # If line starts with 'def ' at col 0, fix it and following lines until next def or end
        new_lines.append(line)
    else:
        new_lines.append(line)

# Let's try a more specific fix for the observed issue
fixed_lines = []
for i, line in enumerate(lines):
    # Fix the specific lines starting from line 142 (idx 141)
    if i >= 141 and line.startswith('def generate_next_token'):
        fixed_lines.append('    ' + line)
    # The body of generate_next_token seems to already have 8 spaces of indentation in the view?
    # No, it says 143:         """...""" which is 8 spaces.
    # If the class is at 0, def should be at 4, body at 8.
    # line 142 was at 0. body at 8. 
    # So def needs +4.
    else:
        fixed_lines.append(line)

with open(path, 'w', encoding='utf-8') as f:
    f.writelines(fixed_lines)
print("Manually fixed line 142 indentation")
