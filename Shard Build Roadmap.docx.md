

| 🧊 SHARD Complete Build Roadmap Copy-paste prompts to take the project from current state → production-ready |
| :---: |

| 5 Phases Structured execution plan | 26 Prompts Ready to paste into Claude | \~12 Weeks Concept → production ready |
| :---: | :---: | :---: |

# **How To Use This Document**

Each section below contains a ready-to-paste prompt for Claude. The prompts are written in order — complete them sequentially. Each builds on the output of the previous one.

| ⚠️ IMPORTANT:  Always share your GitHub repo link (https://github.com/TrentPierce/Shard) at the start of each new Claude session. Claude has no memory between sessions, so include relevant context from the previous prompt's output when starting a new one. |
| :---- |

| Step | What to do |
| :---- | :---- |
| 1 | Open a new Claude conversation |
| 2 | Copy the prompt block exactly as written |
| 3 | Paste it and send |
| 4 | Save Claude's output (code, files, docs) before closing |
| 5 | Move to the next prompt |

| PHASE 1  Make One Thing Work End-to-End   ·   Weeks 1–4 |
| :---- |

Goal: Get a working, demonstrable loop where a browser Scout drafts tokens and a local Rust daemon verifier produces a real streamed response. This is the "does it actually work" phase. Nothing else matters until this is done.

## **Prompt 1 — Audit Current Working State**

Use this first. It tells you exactly what's wired up and what's stubbed, so you're not wasting time building on broken foundations.

| PROMPT 1: Audit Current Working State |
| :---- |
| I'm working on a project called Shard: https://github.com/TrentPierce/Shard Shard is a distributed speculative decoding inference network. The concept: \- Browser tabs (Scouts) run small draft models via WebGPU/WebLLM \- Desktop/server nodes (Shards) run full verifier models (BitNet/GGUF) \- Scouts generate candidate tokens, Shards verify them in one parallel pass \- This produces quality output at a fraction of the GPU cost I need you to do a full audit of the repo and tell me: 1\. What is actually implemented and working today vs stubbed/simulated? 2\. Is there a real end-to-end inference path that goes: browser \-\> draft    \-\> verifier \-\> streamed response? If not, where does it break? 3\. What is the single most critical missing piece to get a working demo? 4\. What files should I look at first to understand the current state? Be specific. I need to know what code is real vs placeholder. |

## **Prompt 2 — Build the Core Speculative Decoding Loop**

Once you know what's missing from Prompt 1, use this to build the actual inference pipeline. This prompt also bakes in Scout churn fault tolerance from the start — it is far cheaper to handle it here than to retrofit it later.

| ⚠️ SCOUT CHURN:  Scouts will disconnect mid-draft — laptops close, networks drop, browser tabs crash. If the Verifier waits indefinitely for a draft that never arrives, the user's stream hangs forever. The fallback must be built into the core loop from day one, not added later. |
| :---- |

| PROMPT 2: Build the Core Speculative Decoding Loop |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard The core mechanism is speculative decoding across a distributed P2P network. Here is what the audit from my previous session found: \[PASTE AUDIT OUTPUT HERE\] I need you to implement the core speculative decoding loop. Requirements: SCOUT SIDE (browser/TypeScript): \- Load a small draft model via WebLLM (Phi-2 or TinyLlama preferred) \- Given a prompt, generate K draft tokens (K=4 is a good starting default) \- Submit the draft bundle {prompt, tokens, logprobs} to the verifier API \- If the Scout loses network mid-generation, it must send a cancellation   signal — or the Verifier's timeout (see below) handles it automatically VERIFIER SIDE (Rust daemon): \- Accept draft bundles at POST /v1/scout/draft \- Run the draft tokens through the full verifier model in one forward pass \- Accept tokens that pass, resample the first rejection, then return the   accepted prefix as a streamed Server-Sent Events response SCOUT CHURN FAULT TOLERANCE (critical — build this into the core loop): \- When the Verifier dispatches a draft job to a Scout, start a timeout   timer. Default timeout: 800ms (configurable via SHARD\_SCOUT\_TIMEOUT\_MS) \- If the Scout does not return a draft bundle within the timeout window:   a) Mark the Scout as 'timed out' in the peer registry   b) Immediately fall back to standard autoregressive generation:      the Verifier generates the next tokens itself, without speculative help   c) Stream the fallback tokens to the user — the stream must NOT pause      or stutter. The user should not be able to tell a timeout occurred.   d) Log: 'Scout \[node\_id\] timed out after Xms — falling back to local gen' \- After 3 consecutive timeouts from the same Scout node\_id, temporarily   remove it from the active Scout pool for 60 seconds before retrying Please implement this. Show me the TypeScript for the Scout side and the Rust handler for the verifier including the timeout/fallback logic. Focus on correctness over optimization. Use comments to explain non-obvious logic. |

## **Prompt 3 — Get a Local Demo Running**

The goal here is a single-machine demo you can record and share. Browser Scout \+ local Rust verifier, no P2P needed yet.

| PROMPT 3: Get a Local Demo Running |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard I have the speculative decoding loop implemented (see previous output). Now I need to get a working local demo running on one machine that I can record and share. The demo needs to show: 1\. Browser tab opens \-\> Scout mode activates \-\> WebGPU draft model loads 2\. User types a prompt in the web UI 3\. Scout generates 4 draft tokens and sends them to localhost:9091 4\. Rust daemon verifies and streams the response back 5\. User sees tokens stream in real-time in the browser Please help me: a) Write a docker-compose.yml that spins up just the daemon \+ web client    with no external dependencies beyond the local machine b) Write a shell script (demo.sh) that starts everything and opens the    browser to the right URL c) Tell me what GGUF model to download and where to place it for the    verifier to find it automatically d) Identify any current bugs or blockers that would prevent this from working I want to run: ./demo.sh and have a working inference loop in \< 5 minutes. |

## **Prompt 4 — Create the Demo Video Script & Benchmark**

Once the demo runs, document it. A recorded demo \+ benchmark numbers are what make contributors and companies take this seriously.

| PROMPT 4: Create Demo Script and Benchmark Harness |
| :---- |
| I have a working local Shard demo. I need two things: 1\. BENCHMARK SCRIPT: Write a Python script (benchmarks/compare.py) that:    \- Sends the same 10 test prompts to: (a) direct OpenAI API,      (b) Shard local demo    \- Measures: time-to-first-token, total latency, tokens/second    \- Outputs a clean comparison table showing the cost/speed difference    \- Saves results to benchmarks/results\_latest.json 2\. DEMO SCRIPT (docs/demo-script.md): Write a step-by-step screen    recording script I can follow to make a 3-minute demo video showing:    \- Starting the network with one command    \- A browser tab joining as Scout    \- Sending a prompt and watching tokens stream    \- The benchmark comparison appearing at the end Make the benchmark prompts representative of real enterprise use: summarization, code generation, Q\&A. Nothing trivial like 'hello world'. |

| PHASE 2  Simplify & Reposition   ·   Week 5 |
| :---- |

Goal: Strip the complexity from the public face of the project. You have a brilliant architecture, but the README and website are losing people before they understand the value. Fix that.

## **Prompt 5 — Rewrite the README**

The README is your most important marketing document. It needs to answer one question in under 30 seconds: "Why should I care?"

| PROMPT 5: Rewrite the README for Maximum Clarity |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard The current README is too complex for someone landing cold. I need you to rewrite it from scratch following these rules: STRUCTURE (in this exact order): 1\. One-line description: what it is and who it's for 2\. A 3-row cost comparison table (Traditional Cloud AI vs Shard) 3\. A \[DEMO VIDEO\] placeholder badge (I'll add the real link later) 4\. 'How it works' — max 5 bullet points, plain English, no jargon 5\. 'Get started in 5 minutes' — 3 paths: Scout (browser), Shard node,    Enterprise (API drop-in). Each path is \< 5 steps. 6\. Architecture section (can be technical, for contributors) 7\. Links to whitepaper, API docs, contributing RULES: \- No mention of wallets, ledgers, or credits in the first half \- No mermaid flowcharts in the top section \- Every technical term must be explained the first time it appears \- The word 'speculative decoding' should appear AFTER the value prop Write the full README.md content. |

## **Prompt 6 — Create a One-Page Explainer for Companies**

This is what you send to a CTO when they ask 'what is this?' — a single page that explains the value without requiring technical knowledge.

| PROMPT 6: Create a One-Page Company Explainer |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard Create a one-page explainer document (docs/explainer.md) targeted at a VP Engineering or CTO at a company spending $5K+/month on AI APIs. The document should cover: THE PROBLEM (2-3 sentences): Every token your app generates is a cost. At scale, centralized GPU compute is the single biggest line item for AI companies. THE INSIGHT (2-3 sentences): Your users' browsers sit idle while your servers strain. What if the compute was already there? HOW SHARD WORKS (non-technical, 5 sentences max): Explain the Scout/Shard/speculative decoding concept as if talking to a smart non-engineer. WHAT YOU GET (bullet points): \- OpenAI-compatible drop-in API \- Estimated cost reduction (be honest about the range: 40-80%) \- No vendor lock-in \- Works with existing infrastructure PILOT OFFER: 'Set up a private Shard network for your org in one week, free. You need: 10+ active browser users \+ one server.' CONTACT: \[your contact info placeholder\] Keep it under 400 words. No code blocks. Plain language throughout. |

## **Prompt 7 — Simplify the Website Landing Page**

The Vercel-deployed site should convert visitors, not confuse them. This prompt rebuilds the landing page with a clear value proposition and single CTA.

| PROMPT 7: Redesign the Web Landing Page |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard Deployed at: https://shard-trents-projects-20e9a51a.vercel.app The current landing page has too much information and no clear CTA. I need you to redesign the web/app/page.tsx (Next.js) landing page. The new landing page should have exactly these sections: HERO: 'Serve AI to your users at 60% less cost.' Subheadline: 'Shard turns your users' browsers into AI compute nodes. OpenAI-compatible. Drop-in. No new infrastructure required.' Two buttons: \[See the Demo\] \[Read the Docs\] HOW IT WORKS (3-column visual): 1\. User opens your app \-\> their browser joins as a Scout 2\. Scout drafts candidate tokens locally via WebGPU 3\. Your server verifies and streams — paying only for verification STATS BAR: 60-80% cost reduction | OpenAI-compatible | Self-healing mesh USE CASES (4 cards): Community AI endpoint | Internal tools overflow |    Research labs | Hackathon infra CTA: 'Start your free pilot' \-\> links to docs/explainer.md Keep the existing nav. Remove anything not listed above. Use Tailwind. Keep it clean and fast. No animations. |

| PHASE 3  Fix the Three Critical Technical Risks   ·   Weeks 6–10 |
| :---- |

Goal: Address the three things that could kill the project in production. These need to be fixed before you pursue enterprise pilots. Don't skip this phase.

| 🔴 RISK 1:  WebGPU is unavailable on Firefox and Safari. That cuts your potential Scout pool by \~65%. You need CPU/WASM fallbacks. |
| :---- |

| 🔴 RISK 2:  If Scout and Verifier models are from different families, speculative acceptance rates collapse to near zero. You need validated model pairs. |
| :---- |

| 🔴 RISK 3:  NAT traversal fails silently in most enterprise environments. The current EC2 manual workaround is a dealbreaker for adoption. |
| :---- |

## **Prompt 8 — Fix WebGPU Compatibility (Risk 1\)**

| PROMPT 8: Fix WebGPU Compatibility — Add WASM Fallback |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard WebGPU is not available on Firefox or Safari, which limits Scout participation to Chrome/Edge only. I need a graceful fallback. Please implement: 1\. CAPABILITY DETECTION (web/lib/scout-init.ts or equivalent):    \- Detect WebGPU availability at startup    \- Detect WebAssembly availability as fallback    \- Return a capability tier: 'webgpu' | 'wasm' | 'unsupported' 2\. WASM FALLBACK: For browsers without WebGPU, use a smaller model    (e.g., a quantized TinyLlama via GGML/WASM) for draft generation.    The draft quality will be lower, but the node still contributes. 3\. UI INDICATOR: In the web client, show a small badge indicating the    Scout's current mode: 'WebGPU Active', 'WASM Mode', or    'This browser cannot contribute compute — try Chrome' 4\. GRACEFUL DEGRADATION: If a node is in WASM mode, the verifier    should apply a lower acceptance threshold or weight its drafts    differently. Document how this is configured. Show me the implementation for all four parts. |

## **Prompt 9 — Validate Model Pair Compatibility (Risk 2\)**

| PROMPT 9: Validate and Document Model Pair Compatibility |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard Speculative decoding only works well when the draft model's token distribution is close to the verifier's. Mismatched model families result in near-zero acceptance rates and actually \*slower\* inference than direct generation. I need you to: 1\. WRITE A COMPATIBILITY TEST (benchmarks/model-pair-test.py):    \- Given a draft model and a verifier model    \- Run 50 test prompts through both    \- Calculate: speculative acceptance rate, effective speedup vs baseline    \- Flag pairs with acceptance rate \< 0.6 as 'not recommended' 2\. DOCUMENT VALIDATED PAIRS (docs/model-pairs.md):    \- List which Scout model \+ Verifier model combinations are known good    \- Include: expected acceptance rate, VRAM requirements, use case    \- Start with the two most practical pairs for our setup      (WebLLM draft models paired with BitNet/GGUF verifiers) 3\. CONFIGURATION ENFORCEMENT: In the Rust daemon, if a Scout connects    with a model\_id that isn't in the approved pairs list, log a warning    and apply a conservative acceptance threshold rather than rejecting    the connection entirely. Show me the test script, the docs, and the Rust configuration enforcement. |

## **Prompt 10 — Fix NAT Traversal (Risk 3\)**

| PROMPT 10: Fix NAT Traversal — Make P2P Just Work |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard The current docs have a manual EC2 workaround section for NAT/firewall issues. This is a dealbreaker for enterprise adoption. NAT traversal needs to be automatic and invisible to the user. Please implement: 1\. AUTO-DETECTION: At daemon startup, detect the node's network situation:    \- Am I behind a NAT?    \- Can I do hole-punching with the bootstrap peer?    \- Do I need a relay/TURN fallback? 2\. RELAY FALLBACK: If hole-punching fails, automatically route through    the bootstrap peer as a relay. Performance is lower but it works.    Log this clearly: 'Operating in relay mode — direct P2P unavailable' 3\. CLOUD CONFIG HELPER: Write a script (scripts/cloud-setup.sh) that    automatically configures the correct firewall rules for the three    major cloud providers (AWS, GCP, Azure) using their CLIs.    It should detect which cloud it's running on and apply the right config. 4\. HEALTH CHECK: Add a /connectivity endpoint to the daemon that    returns: nat\_type, relay\_mode, reachable\_from\_public (bool),    recommended\_action (string) — so operators can diagnose issues. Show me the implementation for all four parts. |

## **Prompt 11 — Fix the Python SDK**

The Python SDK is currently in 'scaffolding stage.' It needs to actually work — it's how developers will integrate Shard into their apps.

| PROMPT 11: Build a Working Python SDK |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard The Python SDK is currently scaffolding/experimental. I need it to be a real, installable package that developers can use to call the Shard network exactly like they'd call the OpenAI SDK. Requirements: 1\. INSTALLATION: pip install shard-inference    (Package name to register on PyPI: shard-inference) 2\. CORE API (must be OpenAI SDK-compatible drop-in):    from shard import Shard    client \= Shard(base\_url='http://localhost:9091', api\_key='optional')    response \= client.chat.completions.create(      model='shard-hybrid',      messages=\[{'role':'user','content':'Hello'}\],      stream=True    ) 3\. ASYNC SUPPORT: async/await variant of the above 4\. ERROR HANDLING: Clear errors when the daemon is unreachable, when    no verifier nodes are available, and when credit balance is 0 5\. DOCS: sdk/python/README.md with 5 working examples (sync, async,    streaming, error handling, custom model selection) Write the full SDK implementation and README. |

| PHASE 4  Get 3 Enterprise Pilots   ·   Weeks 10–12 |
| :---- |

Goal: Get three real organizations running Shard in a production or near-production workload. This is where you validate the economics and find the real edge cases. You don't need 1,000 users — you need three that will give honest feedback.

## **Prompt 12 — Build a Self-Serve Pilot Setup Flow**

Enterprises won't manually configure firewall rules and download binaries. Make the pilot setup a single command.

| PROMPT 12: Build a Self-Serve Pilot Setup Flow |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard I need to make it trivially easy for an enterprise team to set up a private Shard network for internal use. The target user is a DevOps engineer who has never heard of Shard before. Create a pilot setup package that includes: 1\. SETUP SCRIPT (scripts/pilot-setup.sh):    \- Asks 3 questions: org name, server IP or hostname, expected users    \- Generates a complete docker-compose.yml preconfigured for their setup    \- Generates a .env file with correct settings    \- Prints a URL the team can share with users to join as Scouts    \- Total setup time: under 10 minutes 2\. USER ONBOARDING PAGE (web/app/join/\[org\]/page.tsx):    \- Accessible at /join/\[org-name\]    \- Explains what the user is joining in plain English    \- Shows browser compatibility (WebGPU check runs automatically)    \- One button: 'Start Contributing Compute'    \- Shows a live counter of the org's current Scout count 3\. PILOT METRICS DASHBOARD (/admin):    \- Scouts active right now    \- Requests served today / this week    \- Estimated cost saved vs. equivalent OpenAI API calls    \- Simple, no login required for internal use Build all three components. |

## **Prompt 13 — Write the Pilot Outreach Template**

You need to actually contact companies. This prompt creates the outreach sequence you'll use to get pilots.

| PROMPT 13: Write the Pilot Outreach Materials |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard I need to reach out to 3 types of organizations for free pilots: Type A: AI startups spending $5K+/month on OpenAI Type B: University research labs with student compute to offer Type C: Mid-size companies with internal AI tools teams Write the following outreach materials: 1\. COLD EMAIL (for each type) — max 150 words each:    \- Subject line that gets opened    \- Personalized opening for each type    \- The offer: free private network setup, 1 week    \- Single CTA: 'Can I send you a 5-minute demo?' 2\. FOLLOW-UP EMAIL (for non-responders after 5 days):    \- 3 sentences max    \- Lead with the cost number 3\. PILOT AGREEMENT (docs/pilot-agreement.md):    \- What Shard provides (setup, support, monitoring)    \- What the pilot org provides (server, users, 30-min feedback call)    \- Duration: 30 days    \- No cost, no commitment    \- Simple language, no legal jargon Write all materials. |

| PHASE 5  Harden and Open   ·   Ongoing post-Week 10 |
| :---- |

Goal: Turn pilot learnings into a production-grade system. Add proper testing, observability, and community infrastructure so the project can grow without you doing everything yourself.

## **Prompt 14 — Build the Test Suite**

A project without tests can't accept external contributors. This is the minimum viable test coverage to open the project for PRs.

| PROMPT 14: Build the Minimum Viable Test Suite |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard The project currently has no meaningful test coverage. I need a test suite that covers the critical paths before I start accepting PRs. Please write tests for: 1\. SPECULATIVE DECODING CORRECTNESS (Python or Rust):    \- Given a known draft sequence and verifier logprobs, does the      acceptance algorithm correctly identify which tokens to keep?    \- Edge cases: all tokens accepted, first token rejected, empty draft 2\. API CONTRACT (Rust integration tests):    \- POST /v1/chat/completions returns a valid SSE stream    \- POST /v1/scout/draft with malformed payload returns 400    \- GET /health returns 200 with expected fields    \- GET /topology returns current peer count 3\. PYTHON SDK (pytest):    \- Basic completion call succeeds against a local mock server    \- Streaming works correctly (chunks arrive in order)    \- Unreachable daemon raises ShardConnectionError 4\. CI CONFIGURATION (.github/workflows/test.yml):    \- Runs all tests on PR and push to main    \- Shows test coverage badge in README Write all tests and the CI config. Aim for the critical paths only, not 100% coverage. |

## **Prompt 15 — Build the Network Explorer**

A public network explorer makes the project feel real and alive. It's also the social proof that attracts new contributors.

| PROMPT 15: Build a Public Network Explorer |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard I need a public network explorer page (web/app/network/page.tsx) that shows the current state of the Shard network in real-time. The page should show: 1\. NETWORK STATS (top of page, auto-refreshes every 10s):    \- Active Scouts (browser nodes right now)    \- Active Shards (verifier nodes right now)    \- Total requests served in last 24h    \- Estimated GPU hours saved vs. centralized inference 2\. NODE LIST (table, paginated):    \- Node ID (truncated), type (Scout/Shard), region (if known),      status (active/idle), uptime, drafts contributed today    \- Sort by: uptime, contribution, region 3\. LIVE ACTIVITY FEED (right panel, last 20 events):    \- 'Scout \[abc123\] joined from Chrome/Windows'    \- 'Request completed — 47 tokens, 3 rounds, accepted 89%'    \- 'Shard node \[def456\] went offline' 4\. HOW TO JOIN (bottom of page):    \- Three steps with copy-pasteable commands    \- Links to the browser Scout path and daemon download Fetch real data from the daemon's /topology and /metrics endpoints. Mock the data if the endpoints don't exist yet, clearly marked as mock. Use Tailwind. No chart libraries — plain numbers and tables only. |

## **Prompt 16 — Write the Contributing Guide**

If the project can't be contributed to, it stays a one-person project. This guide sets up the contributor experience properly.

| PROMPT 16: Write the Contributor Onboarding Guide |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard I need a proper CONTRIBUTING.md that makes it easy for developers to make their first PR without asking me questions. The guide should cover: 1\. ARCHITECTURE MAP (one paragraph per component):    \- web/ — Next.js frontend \+ Scout logic    \- desktop/rust — Shard daemon (libp2p, verifier, API)    \- sdk/ — Python SDK    \- benchmarks/ — test scripts    How they connect to each other 2\. LOCAL SETUP (exact commands, no ambiguity):    From a fresh machine to running all tests in \< 15 minutes    Prerequisites: Rust 1.75+, Node 18+, Python 3.10+ 3\. HOW TO FIND GOOD FIRST ISSUES:    \- What the 'good first issue' label means for this project    \- 3 example issues with scope, difficulty, and where to start 4\. PR PROCESS:    \- Branch naming convention    \- What every PR needs: description, tests, benchmark (if perf change)    \- Review turnaround expectation 5\. WHAT WE DO NOT ACCEPT:    \- Changes to the credit/ledger system without discussion    \- New external token integrations    \- New draft model types without a model-pair compatibility test Write the full CONTRIBUTING.md. |

## **Prompt 17 — Set Up Shard Cloud Tier**

Once pilots are running, the next revenue path is a hosted version for companies that want the economics without managing the infrastructure. This prompt also adds token-bucket rate limiting at the API gateway — without it, a single API key or IP can monopolize the entire Verifier fleet.

| 🛡️ API GATEWAY RISK:  Prompt 20 protects the P2P mesh layer from malicious Scouts. But the public /v1/chat/completions endpoint is unprotected from a different attack: a legitimate (or stolen) API key sending thousands of requests per second, exhausting Verifier capacity for all other users. Rate limiting must sit at the gateway, not just the mesh. |
| :---- |

| PROMPT 17: Design the Shard Cloud Architecture |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard I have enterprise pilots running on self-hosted Shard networks. The next step is a hosted 'Shard Cloud' tier for companies that want the cost savings without managing their own verifier nodes. Help me design and begin implementing Shard Cloud: 1\. ARCHITECTURE DESIGN (docs/cloud-architecture.md):    \- How does Shard Cloud work differently from self-hosted?    \- Who runs the verifier nodes? (Shard-managed fleet)    \- How does customer traffic stay isolated?    \- What is the billing model? (per verified token makes sense) 2\. API KEY MANAGEMENT:    \- Customer signs up \-\> gets an API key    \- Key is included in requests as X-Shard-API-Key header    \- Daemon validates key against a central auth service    \- Design the auth service interface (can be simple initially) 3\. API GATEWAY RATE LIMITING (token-bucket algorithm):    Implement rate limiting on POST /v1/chat/completions to prevent    any single API key or IP from monopolizing Verifier capacity:    \- Per API key: 60 requests/minute (burst allowance: 10\)    \- Per IP (unauthenticated or unknown key): 10 requests/minute    \- On limit exceeded: return HTTP 429 with headers:      Retry-After: \<seconds until bucket refills\>      X-RateLimit-Limit: 60      X-RateLimit-Remaining: 0      X-RateLimit-Reset: \<unix timestamp\>    \- Enterprise tier API keys (flagged in auth service): 600 req/min    \- The limits must be configurable via environment variables:      SHARD\_RATE\_LIMIT\_DEFAULT=60      SHARD\_RATE\_LIMIT\_BURST=10      SHARD\_RATE\_LIMIT\_ENTERPRISE=600    \- Expose current rate limit state per key at GET /v1/usage/rate-limit    \- Implement using an in-memory token bucket in Rust (no Redis required      initially — add a Redis backend option for multi-node deployments) 4\. USAGE METERING:    \- Track tokens verified per customer per day    \- Expose this at GET /v1/usage for customers to self-serve    \- Design the data schema for metering records 5\. PRICING PAGE (docs/pricing.md):    \- Free tier: up to 100K tokens/month (pilot conversion)    \- Pro: $X per million verified tokens    \- Enterprise: custom pricing    \- Always show the comparison to equivalent OpenAI cost Produce the architecture doc, the auth service interface design, the rate limiter implementation, the metering schema, and the pricing page. |

## **Prompt 18 — The Final Health Check**

Run this last. It gives you a clear picture of what's done, what's still missing, and what to prioritize next.

| PROMPT 18: Final Project Health Check |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard I've completed a full build cycle based on my roadmap. I need you to do a final audit and give me an honest health check. Please check and report on: 1\. WORKING DEMO: Is there a single command that spins up a working    local inference demo? Test it and report what happens. 2\. README CLARITY: Read the README as if you've never seen Shard.    At what point do you understand the value? What still confuses you? 3\. TECHNICAL RISKS: Are the three risks addressed?    \- WebGPU fallback implemented?    \- Model pair validation in place?    \- NAT traversal automatic? 4\. ENTERPRISE READINESS: Could a DevOps engineer at a 100-person    company set up a private Shard network in one afternoon?    What would stop them? 5\. CONTRIBUTION READINESS: Could a developer make a first PR    without talking to me? What's missing from CONTRIBUTING.md? 6\. WHAT TO DO NEXT: Given the current state, what are the top 3    things to work on in the next two weeks? Be specific. Tell me what you actually find, not what I want to hear. |

| ROADMAP ADDITIONS Three gap-filling prompts identified by external audit. These close the remaining production readiness gaps. |
| :---- |

| PHASE A  Automated CI/CD & Releases   ·   Add to Phase 5 |
| :---- |

Prompt 14 adds PR testing, but there is no automated release pipeline. Right now, releasing new binaries requires manual steps. This prompt automates the full release process: Rust binaries for all platforms, Docker image publishing, and Python SDK deployment to PyPI — all triggered by a version tag.

| PROMPT 19: Automated CI/CD Pipeline and Releases |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard The project has basic CI for testing PRs but no automated release pipeline. Every release currently requires manual steps. I need to automate this completely. Please create the following GitHub Actions workflows: 1\. .github/workflows/release.yml — TRIGGERED ON: push of a version tag    (e.g. v0.5.0)    MUST DO:    a) Build the Rust daemon binary for all three targets in parallel,       WITH HARDWARE ACCELERATION FLAGS:       \- x86\_64-unknown-linux-gnu         Compile with: RUSTFLAGS='-C target-feature=+avx2'         Include CUDA feature flag: \--features cuda         Use: CUDA\_VERSION=12.2 on the ubuntu-latest runner       \- x86\_64-pc-windows-msvc         Compile with: RUSTFLAGS='-C target-feature=+avx2'         Include CUDA feature flag: \--features cuda       \- aarch64-apple-darwin         Compile with Metal acceleration: \--features metal         Do NOT use CUDA on this target       IMPORTANT: Each Cargo feature flag (cuda, metal, cpu-only) must       be defined in Cargo.toml. Add them if they don't exist yet.       The cpu-only feature must also build cleanly for environments       without GPU hardware (used in CI testing and Docker base images).    b) Run the full test suite using the cpu-only feature.       Abort release if any test fails.    c) Create a GitHub Release with:       \- Auto-generated changelog from commit messages since last tag       \- All three binaries attached as release assets, named clearly:         shard-daemon-linux-cuda, shard-daemon-windows-cuda,         shard-daemon-macos-metal       \- SHA256 checksums file for each binary    d) Build and push a Docker image to ghcr.io/trentpierce/shard       tagged as both :latest and :v0.5.0       The Docker image should use the cpu-only build for portability.       Add a separate :cuda tag built with \--features cuda for GPU hosts.    e) Publish the Python SDK to PyPI using Trusted Publishing       (no API key stored in secrets — use OIDC) 2\. .github/workflows/docker-preview.yml — TRIGGERED ON: push to main    \- Build and push a :main-latest Docker image for preview deploys    \- Do NOT publish to PyPI on this trigger 3\. scripts/release.sh — LOCAL RELEASE HELPER:    \- Takes one argument: the new version (e.g. ./release.sh 0.5.0)    \- Updates VERSION file    \- Runs make version-sync to propagate version everywhere    \- Commits, tags, and pushes — triggering the release workflow Include setup instructions for the required GitHub repository secrets and the PyPI Trusted Publishing configuration. Add a note in docs/installation.md explaining which binary to download based on the user's hardware (CUDA GPU, Apple Silicon, or CPU-only). |

| PHASE B  Mesh Security & Sybil Resistance   ·   Add to Phase 3 |
| :---- |

The roadmap addresses NAT traversal and model pair validation, but not deliberate adversarial behavior. A public P2P mesh is a target. Malicious nodes can submit garbage drafts to waste verifier compute, or flood the network to degrade quality for legitimate users. This prompt hardens the mesh against those attacks.

| PROMPT 20: Mesh Security and Sybil Resistance |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard Shard is a public P2P mesh. This makes it a target for adversarial behavior. I need to harden the network against three specific attacks: ATTACK 1 — GARBAGE DRAFT FLOODING: A malicious Scout submits random tokens as drafts, wasting verifier compute and degrading throughput for legitimate users. FIX: Track per-node acceptance rate over a sliding window (last 100 drafts). If a node's acceptance rate drops below 0.3, automatically:   \- Increase its PoW difficulty requirement by 4x   \- Log: 'Scout \[node\_id\] flagged: acceptance rate {rate}'   \- If rate stays below 0.3 for 10 minutes, temporarily ban the node ID ATTACK 2 — SYBIL NODES (fake identity farming): An attacker spins up many fake node identities to game the credit/ priority system without contributing real compute. FIX: Enforce the existing Golden Ticket Protocol more strictly.   \- Each new node identity must solve a calibrated PoW challenge     before its first request is accepted (not just on mesh ingress)   \- Node identity (identity.json) must be at least 24 hours old     before the node can accumulate credits — add a created\_at field   \- Document these rules in docs/security.md ATTACK 3 — VERIFIER RESOURCE EXHAUSTION: A flood of simultaneous draft submissions overwhelms a verifier node. FIX: Add per-IP and per-node-ID rate limiting to the Rust daemon:   \- Max 10 draft submissions per node per second   \- Global queue cap: if pending drafts exceed 500, return 503 with     Retry-After header rather than queuing indefinitely   \- Expose current queue depth at GET /metrics Implement all three fixes. Add tests for each. Document the security model in docs/security.md covering what Shard does and does not protect against. |

| PHASE C  Automated Model Management   ·   Moved to Phase 3 ← was Phase 4 |
| :---- |

| 🔀 REORDERED:  This prompt was originally slotted for Phase 4\. It has been moved into Phase 3 because Prompt 12 (enterprise pilot setup) promises a sub-10-minute one-command install — which is impossible as long as model downloading is a manual step. Automated model management must exist before pilots begin. |
| :---- |

Prompt 3 asks the user to manually find and place a GGUF model. Prompt 12 promises a one-command pilot setup in under 10 minutes. These two are in conflict. As long as model downloading is a manual step, the enterprise pilot flow is broken. This prompt wires automated model management directly into the Rust daemon so verifiers come up ready with no human intervention.

| PROMPT 21: Automated Model Downloading and Caching |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard Currently, setting up a Shard verifier node requires manually finding, downloading, and placing a GGUF model file. This breaks the goal of a one-command, sub-10-minute pilot setup. I need the Rust daemon to handle model management automatically. Please implement a model manager in the Rust daemon: 1\. MODEL REGISTRY (deploy/config/models.json):    A JSON file listing the validated model pairs. Each entry contains:    \- model\_id (e.g. 'bitnet-b1.58-3b')    \- download\_url (Hugging Face direct link or mirror)    \- sha256 (hash for integrity verification)    \- size\_bytes    \- min\_vram\_mb (so the daemon can skip models it can't fit)    \- role: 'verifier' | 'draft'    \- paired\_with: \[list of compatible model\_ids for the other role\] 2\. DAEMON STARTUP BEHAVIOR:    On first run (no model found in cache):    a) Read models.json to find the best verifier model for the       available hardware (detect VRAM via sysinfo or nvidia-smi)    b) Download the model to a cache directory:       Linux: \~/.cache/shard/models/       macOS: \~/Library/Caches/shard/models/       Windows: %LOCALAPPDATA%\\shard\\models\\    c) Verify the SHA256 hash. If it fails, delete and abort with error.    d) Print clear progress to stdout: 'Downloading bitnet-b1.58-3b       (2.1 GB)... 47%'    On subsequent runs: check cache first, skip download if hash matches. 3\. CLI COMMANDS:    ./shard-daemon model list        \# show available models \+ download status    ./shard-daemon model download \<id\>  \# pre-download a specific model    ./shard-daemon model clear       \# wipe the cache 4\. ENVIRONMENT OVERRIDE:    SHARD\_MODEL\_PATH=/path/to/custom.gguf should bypass auto-download    entirely, for users who already have models or air-gapped setups. 5\. INTEGRATION WITH PILOT SETUP:    Update scripts/pilot-setup.sh to remove the manual model step.    The setup script should just run 'shard-daemon model download'    as part of its flow. Implement the model manager, the CLI commands, and update the pilot setup script. Include a test that mocks the download and verifies the hash check catches a corrupted file. |

| ENTERPRISE & NETWORK MATURITY Five blind spots identified through enterprise adoption and mature P2P network design review. These must be addressed before scaling. |
| :---- |

| PHASE D  Privacy & End-to-End Encryption   ·   Add to Phase 3 |
| :---- |

Enterprise queries contain proprietary code, PII, and trade secrets. On the public mesh, a prompt flows from the user's browser through a Scout to a Verifier node. Nothing currently prevents a malicious Verifier from logging every prompt it receives. Before any enterprise pilot goes live, there must be a clear answer to the question: 'Can the Verifier read our prompts?'

| 🔐 THE THREAT:  A malicious actor runs a Verifier node. They accept legitimate inference jobs and serve correct results — but silently log every prompt payload they process. The enterprise client has no way to detect this. |
| :---- |

| PROMPT 22: Design and Implement End-to-End Encryption |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard Enterprise clients will send sensitive prompts (proprietary code, PII, internal data) through the Shard mesh. Currently, Verifier nodes can read every prompt payload they process. I need an E2EE scheme that protects prompt privacy without breaking the speculative decoding flow. The fundamental constraint: the Verifier MUST be able to run the model over the prompt to verify drafts. Full E2EE where the Verifier is blind is impossible. So the threat model is: minimize exposure, make logging detectable, and give enterprises a private-mesh escape hatch. Please design and implement: 1\. THREAT MODEL DOCUMENT (docs/security/threat-model.md):    \- What Shard does and does not protect against    \- The trusted vs. untrusted node distinction    \- Recommendations by deployment type (public mesh vs. private mesh) 2\. TRANSPORT ENCRYPTION (already partially present via libp2p Noise):    \- Audit the current libp2p Noise protocol setup in the Rust daemon    \- Confirm that prompt payloads are encrypted in transit between peers    \- Document this in the threat model: 'All peer traffic is encrypted      in transit. Verifier nodes decrypt payloads to run inference.' 3\. PRIVATE MESH MODE (X-Shard-Route: private — extend this feature):    \- When an enterprise deploys a private mesh (their own Verifier nodes      only), add a signed allowlist: only Verifier nodes whose public keys      appear in the enterprise's allowlist.json can accept their jobs    \- The client signs each request with its private key; Verifiers check      the signature against the allowlist before processing    \- Document setup in docs/deployment-guide.md under 'Private Mesh' 4\. AUDIT LOG DETECTION (optional, document the approach):    \- Describe how an enterprise could use canary prompts (known-unique      strings) to detect if a Verifier is exfiltrating data — if the      canary appears elsewhere, the Verifier is compromised 5\. README/DOCS ADDITION (docs/security/privacy-faq.md):    \- 5 Q\&A pairs addressing the questions an enterprise security team      will ask before approving Shard for internal use Implement items 2 and 3\. Write items 1, 4, and 5 as documentation. |

| PHASE E  Credit Ledger Implementation   ·   Add to Phase 3 |
| :---- |

The roadmap correctly hides the credit/wallet system from the front page to avoid confusion with crypto projects. But the ledger is what makes the economic model work — Scouts earn priority access by contributing compute, and without a real implementation, the whole incentive loop is missing. The CRYPTO\_TRUST\_MODEL.md, ledger.wal, and wallet CLI already exist in the repo as scaffolding. This prompt makes them real.

| ⚠️ WITHOUT THIS:  There is no actual reason for users to run Scout nodes. The incentive (priority access / credits) is mentioned throughout the docs but the tracking and redemption logic is not implemented. Pilots will notice. |
| :---- |

| PROMPT 23: Implement the Credit Ledger System |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard The repository has scaffolding for a credit/ledger system (ledger.wal, ledger.snapshot.json, wallet CLI commands) but the actual earn/spend logic is not implemented. Scouts are supposed to earn credits for contributing compute, and spend them for priority inference access. I need this to actually work. IMPORTANT CONSTRAINT: Shard is NOT a cryptocurrency. Credits are an internal priority-access accounting unit. No blockchain. No external token. No speculative value. Think airline miles, not ETH. Please implement: 1\. CREDIT EARNING (Rust daemon — Scout side):    When a Scout's draft tokens are accepted by the Verifier:    \- Award credits proportional to accepted tokens (not submitted tokens)    \- Formula: credits \+= accepted\_tokens \* quality\_multiplier    \- quality\_multiplier \= acceptance\_rate over last 100 drafts (0.5–1.5x)    \- Write the credit event to ledger.wal as a signed entry 2\. CREDIT SPENDING (Rust daemon — request routing):    When a node submits a /v1/chat/completions request:    \- Nodes with credit balance \> 100: routed immediately (priority queue)    \- Nodes with credit balance 1–100: routed normally    \- Nodes with credit balance 0 (Leeches): queued behind contributors    \- Never fully block Leeches — just deprioritize them 3\. LEDGER INTEGRITY:    \- Each ledger entry must be signed with the node's private key    \- The daemon must verify signatures on startup when replaying WAL    \- If any entry fails verification, halt and log: 'LEDGER TAMPERED'    \- Periodic snapshots must hash the current state for fast replay 4\. LEDGER API ENDPOINTS (already scaffolded, implement the logic):    GET /credits/\<wallet\>         — current balance    GET /credits/tx/\<tx\_id\>       — single transaction detail    GET /ledger/head              — latest ledger entry hash    GET /ledger/stats             — total credits issued, total nodes    GET /ledger/export?from\_height=1\&limit=100 — paginated export 5\. DOCS (docs/credits.md):    \- How credits are earned (plain English)    \- How credits affect routing priority    \- That credits have no monetary value and cannot be transferred      to external wallets or systems    \- How to check your balance Implement items 1–4 in Rust. Write item 5 as documentation. Add tests for: earning on acceptance, priority routing logic, and ledger signature verification. |

| PHASE F  Protocol Versioning & Network Upgrades   ·   Add to Phase 5 |
| :---- |

P2P networks are notoriously difficult to upgrade. If v0.5.0 changes the draft submission format and half the network is still on v0.4.x, nodes will silently exchange malformed messages or crash. This is how distributed networks fracture. libp2p provides protocol negotiation primitives — they need to be used correctly from the start, before the network has real users who cannot all be forced to upgrade simultaneously.

| PROMPT 24: Implement Protocol Versioning and Graceful Upgrades |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard As Shard's network grows, protocol versions will diverge. A node on v0.4.x connecting to a v0.5.0 node with a changed message format will cause silent failures or crashes. I need strict protocol versioning built into the libp2p layer before we have real users. Please implement: 1\. PROTOCOL VERSION IDENTIFIERS:    Register all libp2p protocols with versioned identifiers:    \- /shard/draft/1.0.0    (Scout \-\> Verifier draft submission)    \- /shard/verify/1.0.0   (Verifier response stream)    \- /shard/mesh/1.0.0     (peer discovery and topology)    When a message format changes in a breaking way, bump the version.    Minor/patch changes that are backward-compatible do not bump. 2\. VERSION NEGOTIATION ON CONNECT:    When two nodes connect via libp2p:    \- Exchange supported protocol versions during handshake    \- If no common version exists, close the connection gracefully    \- Log: 'Peer \[id\] rejected: no compatible protocol version.      Peer supports \[versions\], we support \[versions\]. Upgrade required.'    \- Do NOT crash. Do NOT silently drop messages. 3\. OPERATOR UPGRADE PROMPT:    If \>30% of connected peers are running a newer major protocol    version than the local node:    \- Log a persistent WARNING on every startup:      'WARNING: Your node is running protocol v1.x. The majority of       the network has upgraded to v2.x. Upgrade at: \[release URL\]'    \- Expose this at GET /node/status as: upgrade\_recommended: true 4\. MIGRATION GUIDE TEMPLATE (docs/protocol-versioning.md):    \- How to bump a protocol version (checklist for contributors)    \- The deprecation policy: old versions supported for 90 days    \- How to test cross-version compatibility locally      (spin up two Docker containers on different versions) 5\. CROSS-VERSION TEST:    Write an integration test that spins up a v1.0.0 node and a    v2.0.0 node (simulated with a flag) and confirms they:    a) Reject each other's connections gracefully    b) Both remain healthy and connected to same-version peers Implement items 1–3 in Rust. Write items 4–5 as docs and tests. |

| PHASE G  Enterprise Observability (OpenTelemetry)   ·   Add to Phase 4 |
| :---- |

Prompt 12 builds a simple pilot dashboard and Prompt 15 builds a network explorer. But enterprise DevOps teams already have monitoring stacks — Grafana, Datadog, Prometheus, New Relic. They will not adopt a second dashboard. They need Shard to emit standard telemetry they can ingest into their existing tools. OpenTelemetry is the industry standard. Without it, Shard is invisible to enterprise infrastructure teams.

| PROMPT 25: Implement OpenTelemetry Metrics and Traces |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard The Rust daemon already has a /metrics endpoint and Prometheus/Grafana in the docker-compose monitoring profile. But enterprise DevOps teams need standard OpenTelemetry instrumentation they can plug into Grafana, Datadog, New Relic, or any other OTEL-compatible backend. Please implement full OpenTelemetry instrumentation in the Rust daemon: 1\. METRICS (use opentelemetry-prometheus crate):    Expose the following as OTEL metrics at GET /metrics (Prometheus    exposition format — keep the existing endpoint, add OTEL backing):    Counters:    \- shard\_requests\_total{status='success|error|queued'}    \- shard\_tokens\_verified\_total    \- shard\_tokens\_accepted\_total    \- shard\_tokens\_rejected\_total    \- shard\_scout\_drafts\_received\_total{node\_id}    Gauges:    \- shard\_active\_scouts (current connected Scout nodes)    \- shard\_active\_verifiers (current connected Verifier nodes)    \- shard\_queue\_depth (pending draft submissions)    \- shard\_credits\_issued\_total    Histograms:    \- shard\_request\_latency\_ms (time-to-first-token)    \- shard\_verification\_latency\_ms (time to verify a draft bundle)    \- shard\_acceptance\_rate (per Scout node, rolling window) 2\. DISTRIBUTED TRACES (use opentelemetry-otlp crate):    Instrument the critical path with spans:    \- 'request.received' \-\> 'draft.dispatched' \-\> 'draft.verified'      \-\> 'tokens.streamed'    \- Each span should include: node\_id, model\_id, token\_count, latency    \- Export via OTLP to a configurable endpoint:      OTEL\_EXPORTER\_OTLP\_ENDPOINT=http://localhost:4317    \- If the endpoint is not configured, traces are silently disabled 3\. CONFIGURATION:    Add to shard-node.yaml.example:    observability:      metrics\_backend: prometheus  \# or 'otel' or 'none'      otlp\_endpoint: ''            \# empty \= disabled      service\_name: shard-daemon      service\_version: 0.5.0       \# auto-populated from VERSION file 4\. GRAFANA DASHBOARD (deploy/grafana/shard-dashboard.json):    A pre-built Grafana dashboard JSON that enterprise teams can import.    Panels: request rate, token throughput, acceptance rate over time,    active node count, queue depth, p50/p95/p99 latency.    The dashboard already exists in docker-compose — enhance it with    these specific panels using the new metric names. 5\. DOCS (docs/observability.md):    \- How to connect Shard to Grafana (already in docker-compose)    \- How to connect Shard to Datadog (OTLP endpoint config)    \- How to connect Shard to New Relic (OTLP endpoint config)    \- The full list of emitted metrics with descriptions Implement items 1–3 in Rust. Write items 4–5. |

| PHASE H  Persistent Scout — Browser Extension   ·   Add to Phase 5 |
| :---- |

Browser tab Scouts disappear the moment a user navigates away or closes the tab. This makes the compute pool unreliable — it fluctuates wildly based on how many people happen to have the Shard tab open. A browser extension runs persistently in the background using a Service Worker, surviving tab closes and navigation. This is the difference between a flaky demo and a stable compute layer.

| 📊 THE IMPACT:  Tab-based Scouts provide compute only while the tab is visible and focused. An extension-based Scout runs 24/7 whenever the browser is open. A network of 500 extension Scouts is roughly equivalent to 5,000 tab Scouts in terms of reliable availability. |
| :---- |

| PROMPT 26: Build a Persistent Scout Browser Extension |
| :---- |
| I'm building Shard: https://github.com/TrentPierce/Shard Browser tab Scouts drop off the network every time a user navigates away. I need a browser extension that runs the Scout logic persistently as a background Service Worker, surviving tab closes. Build a Manifest V3 browser extension (works in Chrome and Edge, with a Firefox variant for MV3-compatible Firefox versions): 1\. EXTENSION STRUCTURE (desktop/extension/):    manifest.json          — MV3 manifest    background.js          — Service Worker: Scout loop \+ WebGPU draft    popup/popup.html       — Simple status popup    popup/popup.js         — Reads status from background    icons/                 — 16, 48, 128px icons (generate SVG placeholders) 2\. BACKGROUND SERVICE WORKER (background.js):    \- On install: register as a Scout with the configured Shard endpoint      (default: https://shard-trents-projects-20e9a51a.vercel.app)    \- Poll GET /v1/scout/work every 2 seconds for pending draft jobs    \- If WebGPU is available in the Service Worker context: use it    \- If not (most current browsers): use a WASM draft model    \- On receiving a job: generate draft tokens, POST to /v1/scout/draft    \- Handle offline gracefully: pause polling, resume on reconnect    \- Store stats in chrome.storage.local: drafts submitted, accepted,      credits earned, uptime 3\. POPUP UI (popup.html):    Keep it minimal. Show:    \- Status indicator: green (contributing) / yellow (idle) / red (error)    \- Drafts submitted today / accepted today    \- Credits earned (from ledger API)    \- Toggle: Enable/Disable Scout    \- Link: 'View network' \-\> opens the /network explorer page 4\. CONFIGURATION:    On first install, show an options page where the user sets:    \- Shard endpoint URL (default to public network)    \- Max CPU % to use (default: 25%)    \- Only run when charging (checkbox, default: on) 5\. PACKAGING:    \- Write desktop/extension/build.sh that bundles the extension      into a .zip ready for Chrome Web Store submission    \- Write a GitHub Actions workflow that builds the extension zip      on every release tag alongside the Rust binaries    \- Include instructions in docs/extension.md for loading unpacked      during development and submitting to the Chrome Web Store Note: Service Workers do not have persistent WebGPU access in all browsers today. Implement with a feature check and fall back to WASM. Document the current browser support matrix in docs/extension.md. |

# **Full Roadmap Summary**

| \# | Phase | Prompts | Timeline |
| :---- | :---- | :---- | :---- |
| **1** | **Working Demo** | 1: Audit · 2: Build loop · 3: Local demo · 4: Benchmark | Weeks 1–4 |
| **2** | **Simplify & Reposition** | 5: README · 6: Explainer · 7: Landing page | Week 5 |
| **3** | **Fix Critical Risks** | 8: WebGPU fallback · 9: Model pairs · 10: NAT · 11: Python SDK | Weeks 6–10 |
| **4** | **Enterprise Pilots** | 12: Pilot setup · 13: Outreach | Weeks 10–12 |
| **5** | **Harden & Open** | 14: Tests · 15: Explorer · 16: Contributing · 17: Cloud · 18: Health check | Ongoing |
| **A** | **CI/CD & Releases** | 19: Automated build, Docker, PyPI release pipeline | Phase 5 |
| **B** | **Mesh Security** | 20: Sybil resistance, garbage draft defense, rate limiting | Phase 3 |
| **C** | **Model Management** | 21: Auto model download, hash verification, CLI commands | Phase 3 ← |
| **D** | **E2E Encryption** | 22: Private mesh allowlist, transport audit, threat model docs | Phase 3 |
| **E** | **Credit Ledger** | 23: Earn/spend logic, WAL integrity, ledger API endpoints | Phase 3 |
| **F** | **Protocol Versioning** | 24: Versioned libp2p protocols, upgrade negotiation, migration guide | Phase 5 |
| **G** | **OpenTelemetry** | 25: OTEL metrics \+ traces, Grafana dashboard, Datadog/New Relic docs | Phase 4 |
| **H** | **Browser Extension** | 26: MV3 extension, persistent Service Worker Scout, Chrome Web Store | Phase 5 |

| The core idea behind Shard is real, the architecture is solid, and the problem you're solving is genuinely valuable. Execute the phases in order. The demo comes first. Everything else follows from that. |
| ----- |

