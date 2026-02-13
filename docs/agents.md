# AGENT ARCHITECTURE: The "Hive-Mind" Protocol

## 1. System Overview
The system is a decentralized, peer-to-peer (P2P) inference network that provides free, unlimited LLM access by utilizing a "Hybrid Speculative Decoding" mesh. 

**Core Philosophy:** - **"Compute is Currency":** Users pay for intelligence by contributing idle hardware.
- **"Verify, Don't Trust":** All nodes are assumed untrusted until work is cryptographically verified.
- **"Heavier is Truth":** Heavier nodes (Exe) verify the work of lighter nodes (Web).

## 2. Node Classes (The Agents)

### Class A: "The Shard" (Titan Node)
* **Hardware:** Desktop PC / Server (NVIDIA GPU 8GB+ VRAM).
* **Software:** Installed Native Client (.exe / Python).
* **Role:** The "Target Model" host.
* **Function:** * Holds the full 1.58-bit Quantized Model (e.g., Llama-3-70B-BitNet).
    * Performs "Verification Steps" for Class B nodes.
    * Earns "High Priority" tokens for instant generation.
* **Stack:** `bitnet.cpp`, `libp2p` (Python/Rust bindings).

### Class B: "The Scout" (Browser Node)
* **Hardware:** Standard Laptop / Phone (Consumer GPU/NPU).
* **Software:** Web Browser (Chrome/Edge/Brave).
* **Role:** The "Draft Model" host.
* **Function:** * Runs a tiny, "distilled" model (e.g., Llama-3-1B-Int4) via WebGPU.
    * Performs **Speculative Decoding**: rapid-fires 5-10 token guesses.
    * Sends guesses to a Shard Node for validation.
    * Usage contributes to "Free Tier" access.
* **Stack:** `WebLLM`, `Apache TVM`, `js-libp2p`.

### Class C: "The Leech" (Consumer)
* **Hardware:** Any.
* **Software:** Web Client (No contribution).
* **Role:** Pure consumer.
* **Function:** * Sends prompts, waits for queue.
    * **Traffic Shaping:** Lowest priority. Only served when Swarm Load < 60%.
    * **Upsell:** Prompted to "Turn on Scout Mode" to skip the line.

## 3. The "Hybrid Speculative" Workflow
1.  **Prompt:** User sends prompt "Explain Quantum Physics."
2.  **Scout Swarm:** 3 local browser peers (Scouts) generate the first 10 tokens using a tiny draft model.
3.  **Consensus:** They send these draft tokens to 1 Shard.
4.  **Verification:** The Shard runs the full model *once* in parallel to verify the draft tokens.
    * *Match:* Tokens accepted. Output streamed to user.
    * *Mismatch:* Shard corrects the stream, slashes Scout reputation score.
5.  **Result:** User gets "Server-Grade" quality at "Edge" latency.

## 4. Incentive & Security Protocol
* **Proof of Inference (PoI):** * Shards occasionally inject "Golden Tickets" (pre-solved prompts) to test Scouts.
    * Scouts failing Golden Tickets are banned (Sybil Attack Prevention).
* **The "Double-Dip" Prevention:** * If a native `.exe` client is detected on localhost:8080, the Browser Client **MUST** disable WebGPU and route strictly to the local Shard to prevent hardware crash.
