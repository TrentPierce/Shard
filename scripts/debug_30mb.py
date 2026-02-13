import os

# Set this to your 30MB file path
HUGE_FILE = "shard_whitepaper_context.txt"

def analyze_huge_file():
    if not os.path.exists(HUGE_FILE):
        print("Error: File not found.")
        return

    print(f"Scanning {HUGE_FILE}...")
    
    current_file = "Unknown"
    file_sizes = {} # {filename: size_in_bytes}
    
    with open(HUGE_FILE, 'r', encoding='utf-8') as f:
        for line in f:
            # Detect our separator from the previous script
            if "FILE: " in line and "=====" in line: 
                # Extract filename from "FILE: ./path/to/file"
                parts = line.split("FILE: ")
                if len(parts) > 1:
                    current_file = parts[1].strip()
                    file_sizes[current_file] = 0
            else:
                if current_file in file_sizes:
                    file_sizes[current_file] += len(line.encode('utf-8'))

    # Sort and print
    sorted_files = sorted(file_sizes.items(), key=lambda x: x[1], reverse=True)
    
    print("\n--- THE OFFENDERS (Top 20) ---")
    for name, size in sorted_files[:20]:
        print(f"{size / (1024*1024):.2f} MB  :  {name}")

if __name__ == "__main__":
    analyze_huge_file()