
import gguf
import sys

model_path = "E:\\ggml-model-i2_s.gguf"
reader = gguf.GGUFReader(model_path)

print(f"Model: {model_path}")
print(f"Version: {reader.header.version}")
print(f"Tensor count: {len(reader.tensors)}")

for i, tensor in enumerate(reader.tensors):
    print(f"Tensor {i}: {tensor.name}, shape={tensor.shape}, type={tensor.tensor_type}")
    if i > 10:
        print("...")
        break
