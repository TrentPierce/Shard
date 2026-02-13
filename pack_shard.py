import os

# --- CONFIGURATION ---
OUTPUT_FILE = "shard_whitepaper_context.txt"
MAX_FILE_SIZE_KB = 20  # STRICT LIMIT: Skip any file larger than 50KB

# 1. ALLOWED EXTENSIONS (Source code only)
ALLOWED_EXTENSIONS = {
    '.rs', '.py', '.ts', '.tsx', '.js', '.jsx', 
    '.cpp', '.c', '.h', '.hpp', 
    '.md', '.toml', '.yaml', '.yml', '.json' # Added JSON back, but size limit will catch the big ones
}

# 2. IGNORED DIRECTORIES (The usual suspects)
IGNORE_DIRS = {
    '.next', 'node_modules', 'target', 'build', 'dist', 
    'venv', 'env', '.git', '.vscode', '.idea', '__pycache__', 
    'bin', 'obj', 'pkg', 'assets', 'public', 'images'
}

# 3. IGNORED FILES (Specific annoyances)
IGNORE_FILES = {
    'package-lock.json', 'yarn.lock', 'pnpm-lock.yaml', 'cargo.lock', 
    OUTPUT_FILE
}

def is_text_file(filepath):
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            f.read(1024)
            return True
    except:
        return False

def pack_repo():
    if os.path.exists(OUTPUT_FILE):
        try:
            os.remove(OUTPUT_FILE)
        except:
            print(f"⚠️ Could not delete old {OUTPUT_FILE}. Please close it if it's open.")
            return

    total_files = 0
    skipped_count = 0
    
    with open(OUTPUT_FILE, 'w', encoding='utf-8') as outfile:
        for root, dirs, files in os.walk("."):
            # Clean up directories to skip
            dirs[:] = [d for d in dirs if d not in IGNORE_DIRS and not d.startswith('.')]

            for file in files:
                ext = os.path.splitext(file)[1].lower()
                path = os.path.join(root, file)

                # 1. Basic Filters
                if file in IGNORE_FILES: continue
                if ext not in ALLOWED_EXTENSIONS: continue
                if "shard_context" in file: continue 

                # 2. SIZE CHECK (The most important part)
                try:
                    size_kb = os.path.getsize(path) / 1024
                    if size_kb > MAX_FILE_SIZE_KB:
                        print(f"🚫 SKIPPING HUGE FILE ({size_kb:.1f} KB): {path}")
                        skipped_count += 1
                        continue
                except:
                    continue

                # 3. Binary Check
                if not is_text_file(path): continue

                # 4. Write
                try:
                    with open(path, 'r', encoding='utf-8') as infile:
                        content = infile.read()
                        outfile.write(f"\n\n{'='*20}\nFILE: {path}\n{'='*20}\n")
                        outfile.write(content)
                    total_files += 1
                except Exception as e:
                    print(f"Error reading {path}: {e}")

    # FINAL REPORT
    if os.path.exists(OUTPUT_FILE):
        final_size = os.path.getsize(OUTPUT_FILE) / (1024 * 1024)
        print(f"\n--- SUCCESS ---")
        print(f"Packed {total_files} files.")
        print(f"Skipped {skipped_count} massive files.")
        print(f"Final Size: {final_size:.2f} MB")
        
        if final_size < 5:
            print("✅ READY. Upload this file to the AI.")
        else:
            print("⚠️ STILL LARGE. Try lowering MAX_FILE_SIZE_KB to 20.")
    else:
        print("Error: Output file was not created.")

if __name__ == "__main__":
    pack_repo()