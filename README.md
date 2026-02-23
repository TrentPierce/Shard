<div align="center">
  <img src="assets/logo.png" alt="Shard" width="200" />
  <h1>Shard</h1>
  <p><strong>Browser-Powered Distributed Inference for Private Enterprise AI</strong></p>

  <br/>

  [![CI](https://github.com/TrentPierce/Shard/actions/workflows/ci.yml/badge.svg)](https://github.com/TrentPierce/Shard/actions/workflows/ci.yml)
  [![License: BUSL-1.1](https://img.shields.io/badge/license-BUSL--1.1-blue.svg)](LICENSE)
  [![Version](https://img.shields.io/badge/version-0.4.9-00d4ff.svg)](#)
  [![Demo Video](https://img.shields.io/badge/Demo-Video-red?style=for-the-badge&logo=youtube)](#)

</div>

---

## Why Shard?

Shard eliminates the exorbitant cost of centralized AI APIs by utilizing the idle compute already present in your organization's web browsers.

| Feature | Traditional Cloud AI | Shard Network |
| :--- | :--- | :--- |
| **Cost** | $0.002–$0.06 per 1K tokens | **$0 (Compute-for-Access)** |
| **Privacy** | Data processed on 3rd-party servers | **Localhost-first routing** |
| **Scalability** | Subject to strict API rate limits | **Infinite horizontal scaling** |

---

## How It Works

Shard uses a technique called **speculative decoding** to deliver high-quality AI responses without requiring massive server infrastructure:

1. **User Sends a Prompt**: Your application sends a request to the Shard API.
2. **Scouts Generate Drafts**: Active browser tabs (Scouts) generate lightweight "candidate" tokens using WebGPU.
3. **Shards Verify Results**: A single server (Shard) verifies the candidate tokens in one parallel pass using a full-scale model.
4. **Instant Delivery**: You get the same quality as a giant 70B model but only pay for the fraction of compute needed to verify the work.
5. **Private Mesh**: Sensitive data stays within your network; browser compute is contributed only by authorized users.

---

## Get Started in 5 Minutes

### 1. Join as a Scout (Contribute Compute)
Open the [Live Demo](https://shardnetwork.live) in any Chrome or Edge browser. Your browser will automatically begin loading a lightweight WebGPU model and contributing compute to the public mesh.

### 2. Run a Shard Node (Host Your Own)
Run the verifier daemon on any machine with a GPU or decent CPU:
```bash
docker compose up --build shard-daemon
```
*Your node is now ready to verify drafts from the mesh.*

### 3. Enterprise Integration (API Drop-in)
Replace your OpenAI base URL with your local Shard endpoint. It works instantly with existing SDKs:
```python
from openai import OpenAI
client = OpenAI(base_url="http://localhost:9091/v1", api_key="shard")
```

---

## Architecture & Technical Deep-Dive

Shard is built on a hybrid P2P mesh using **libp2p**. It separates the "heavy lifting" of LLM inference into two roles:
- **Scouts (TypeScript/WebGPU)**: Run 1B-3B parameter models (like Phi-3 or TinyLlama) via WebLLM to produce rapid speculative drafts.
- **Shards (Rust/BitNet)**: Run authoritative 1.58-bit ternary models to verify drafts at scale with minimal VRAM requirements.

[**Read the Whitepaper**](docs/Shard-White-Paper-Feb-2026.md) | [**API Documentation**](docs/API.md) | [**Contributing Guide**](CONTRIBUTING.md)

---

<div align="center">
  <sub>Built with 🧊 by the <a href="https://github.com/TrentPierce/Shard">Shard</a> community</sub>
</div>
