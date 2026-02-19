import os

# Set this to where your project is
ROOT_DIR = "." 

def find_huge_files():
    files_with_size = []
    
    print(f"Scanning {os.path.abspath(ROOT_DIR)}...")
    
    for root, dirs, files in os.walk(ROOT_DIR):
        # Skip the obvious junk folders to speed it up
        if 'node_modules' in root or '.git' in root or 'target' in root:
            continue
            
        for file in files:
            path = os.path.join(root, file)
            try:
                size = os.path.getsize(path)
                # Convert to MB for readability
                size_mb = size / (1024 * 1024)
                files_with_size.append((path, size_mb))
            except:
                pass

    # Sort by size (largest first)
    files_with_size.sort(key=lambda x: x[1], reverse=True)

    print("\n--- TOP 20 LARGEST FILES ---")
    for path, size in files_with_size[:20]:
        print(f"{size:.2f} MB  :  {path}")

if __name__ == "__main__":
    find_huge_files()