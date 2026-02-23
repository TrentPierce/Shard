import os
import requests
from tqdm import tqdm

def download_model():
    model_dir = "models"
    model_name = "tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf"
    model_path = os.path.join(model_dir, model_name)
    url = "https://huggingface.co/TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF/resolve/main/tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf"

    if not os.path.exists(model_dir):
        os.makedirs(model_dir)

    if os.path.exists(model_path):
        print(f"Model already exists at {model_path}")
        return

    print(f"Downloading {model_name}...")
    response = requests.get(url, stream=True)
    total_size = int(response.headers.get('content-length', 0))
    
    with open(model_path, 'wb') as f, tqdm(
        desc=model_name,
        total=total_size,
        unit='iB',
        unit_scale=True,
        unit_divisor=1024,
    ) as bar:
        for data in response.iter_content(chunk_size=1024):
            size = f.write(data)
            bar.update(size)
    print("Download complete.")

if __name__ == "__main__":
    download_model()
