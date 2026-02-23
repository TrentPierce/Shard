# Shard Demo Script

This script provides step-by-step instructions for recording a 3-minute demo video showcasing the Shard distributed inference network.

## Pre-Recording Checklist

- [ ] Docker and Docker Compose installed
- [ ] Node.js 18+ installed
- [ ] A GGUF model placed in `~/.cache/shard/models/` (or let daemon download automatically)
- [ ] Clean terminal with good contrast
- [ ] Screen recording software ready (OBS, QuickTime, etc.)

---

## Demo Script (3 minutes total)

### Scene 1: Introduction (30 seconds)

**[START SCREEN RECORDING]**

1. **Open terminal** and navigate to project root:
   ```bash
   cd ~/Shard
   ```

2. **Show the project structure** briefly:
   ```bash
   ls -la
   ```
   
   *Voiceover: "This is Shard — a distributed inference network that turns browsers into AI compute nodes."*

3. **Show the cost comparison** from README:
   *Voiceover: "Instead of paying $0.002-$0.06 per 1K tokens to cloud providers, Shard uses your users' browsers to generate draft tokens, verified by your own servers. The result: 60-80% cost reduction."*

---

### Scene 2: Start the Network (45 seconds)

4. **Start the demo**:
   ```bash
   ./demo.sh
   ```
   
   *Voiceover: "Let's spin up a local network in under 5 minutes."*

5. **While it starts, narrate what's happening**:
   - Docker pulls/starts the Rust daemon (verifier node)
   - npm starts the Next.js web app (Scout interface)

6. **Wait for both services** to show "healthy" / "running"

7. **Open browser** to `http://localhost:3000`

---

### Scene 3: Browser Scout Demo (60 seconds)

8. **Show the web UI** - point out the key elements:
   - Chat input area
   - Scout status indicator

9. **Enable Scout mode** (if there's a toggle):
   - Click "Enable Scout" or wait for auto-enrollment
   
   *Voiceover: "When a user opens this page, their browser automatically joins the network as a Scout. It downloads a small draft model via WebGPU — less than 500MB."*

10. **Type a prompt** (use something substantive):
    ```
    Explain how a CDN improves website performance in 3 bullet points.
    ```

11. **Hit Enter** and **capture the stream**:
    - Show tokens streaming in real-time
    - Point out the speed (Scout contribution makes it fast!)
    
    *Voiceover: "Tokens are streaming. The browser Scout generated draft candidates, the verifier checked them, and we're seeing the result. All without calling OpenAI."*

---

### Scene 4: Show the Backend (30 seconds)

12. **Open a new terminal tab** and show the daemon:
    ```bash
    curl http://localhost:9091/health
    curl http://localhost:9091/metrics
    ```
    
    *Voiceover: "Behind the scenes, the Rust daemon is tracking metrics — active nodes, tokens verified, queue depth."*

13. **Show peer topology**:
    ```bash
    curl http://localhost:9091/topology
    ```

---

### Scene 5: Benchmark Results (15 seconds)

14. **Run the benchmark** (pre-record this or do it live if time permits):
    ```bash
    python benchmarks/compare.py --skip-openai
    ```
    
    *Voiceover: "Our benchmarks show significant latency improvements compared to cloud APIs."*

---

### Scene 6: Wrap Up (15 seconds)

15. **Stop the demo** (Ctrl+C in terminal running demo.sh)

16. **Final statement**:
    *Voiceover: "That's Shard — browser-powered AI inference. No API keys, no per-token costs, no vendor lock-in. Contribute compute from your users, serve AI to everyone."*

17. **Show the GitHub link**:
    *Voiceover: "Find us on GitHub — contributions welcome."*

**[STOP SCREEN RECORDING]**

---

## Recording Tips

| Tip | How to Execute |
|-----|----------------|
| **Good lighting** | Record during daytime or ensure desk lamp faces you |
| **Clean desktop** | Close unnecessary windows before starting |
| **Consistent audio** | Use a microphone if available, otherwise speak clearly |
| **Practice first** | Run through the demo 2-3 times before recording |
| **Have fun** | Show enthusiasm — this is cool tech! |

## Editing Notes

- Cut out any long waits (model loading, etc.)
- Add captions for technical terms
- Include the GitHub URL as a final card
- Export at 1080p for best quality

---

## Post-Recording

1. Upload to YouTube (unlisted initially)
2. Add timestamps in description:
   - 0:00 Introduction
   - 0:30 Starting the network
   - 1:15 Browser Scout demo
   - 2:15 Backend metrics
   - 2:45 Wrap up

3. Replace `[DEMO VIDEO]` placeholder in README with YouTube link
