# Shard Project Audit — February 2026

**Goal:** Make the distributed P2P inference network easier for more people to discover, understand, and appreciate.

---

## Executive Summary

Shard is an ambitious, technically impressive project — a decentralized P2P inference network combining BitNet 1.58-bit quantization, hybrid speculative decoding, and libp2p mesh networking. The **technical depth is outstanding** (white paper, architecture docs, OpenAI-compatible API). However, the project's **discoverability, first-impression clarity, and "wow factor" for newcomers** have significant room for improvement.

**Current Strengths:**
- ✅ Solid multi-language architecture (Rust + Python + TypeScript)
- ✅ Comprehensive white paper and API documentation
- ✅ CI/CD pipeline with multi-platform releases
- ✅ OpenAI-compatible API (drop-in replacement)
- ✅ Docker Compose deployment with monitoring stack
- ✅ PWA support (manifest.json, service worker)
- ✅ Professional CONTRIBUTING.md, CLA, BUSL-1.1 license

**Critical Gaps:**
- ❌ README doesn't "sell" the project — lacks visuals, badges, and a clear value proposition
- ❌ No landing page or docs site — everything lives in raw Markdown
- ❌ Repo contains tracked binaries, debug logs, and 2,200+ files in `cpp/` vendored directory
- ❌ No demo GIF/video showing the product in action
- ❌ Web app hides behind a raw Vercel subdomain with no SEO
- ❌ No social proof (badges, star count display, contributor avatars)
- ❌ Python SDK is still a scaffold — listed prominently but doesn't work end-to-end

---

## 1. 🏠 README.md — The Front Door

### Problem
The current README immediately jumps into "Live Demo" URLs and env var configs. A first-time visitor has **no visual context** of what Shard is or why they should care. There's no hero image, no badges, no demo GIF, and the Mermaid diagram is plain text that doesn't render on many clients.

### Recommendations

#### 1.1 Add a Hero Section
```markdown
<div align="center">
  <img src="assets/shard-banner.png" alt="Shard" width="800" />
  <h3>Browser-Powered Distributed Inference</h3>
  <p>Free, unlimited LLM access through a decentralized P2P mesh.<br/>
  Contribute compute from your browser. Earn priority access.</p>

  <!-- Badges -->
  ![CI](https://github.com/TrentPierce/Shard/actions/workflows/ci.yml/badge.svg)
  ![License](https://img.shields.io/badge/license-BUSL--1.1-blue)
  ![Version](https://img.shields.io/badge/version-0.4.5-green)
  ![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen)

  [Live Demo](https://shard.network) · [White Paper](docs/Shard-White-Paper-Feb-2026.pdf) · [API Docs](docs/API.md) · [Get Started](#quick-start)
</div>
```

#### 1.2 Add a Demo GIF or Video
Record a 15–30 second screen capture showing:
1. Opening the web app
2. Sending a chat message
3. The network visualizer showing Scout nodes
4. Response streaming back

Place it directly after the hero section. Nothing sells a P2P network better than **seeing it work**.

#### 1.3 Add a "Why Shard?" Section
Before the architecture diagram, add a clear value proposition:
```markdown
## Why Shard?

| Feature | Traditional Cloud AI | Shard |
|---------|---------------------|-------|
| **Cost** | $0.002–$0.06/1K tokens | Free (compute-for-access) |
| **Privacy** | Your data on someone else's server | Localhost-first routing |
| **Scalability** | Buy more GPUs | More users = more GPUs |
| **Resilience** | Single point of failure | Self-healing P2P mesh |
| **Latency** | Network RTT + queue | Local draft + verification |
```

#### 1.4 Restructure for Scanning
Use this order:
1. Hero banner + badges + one-liner
2. Demo GIF
3. "Why Shard?" table
4. Architecture diagram (with a rendered PNG fallback for GitHub mobile)
5. Quick Start (collapsed sections for each platform)
6. Real-World Examples
7. Links (docs, community, contributing)

---

## 2. 🗑️ Repository Hygiene — Critical Cleanup

### Problem
The repo contains **2,459 tracked files**, many of which should NOT be in version control. This bloats the clone, makes contributions harder, and looks unprofessional.

### Files That Must Be Removed (via `git rm --cached`)

#### Debug/Test Artifacts (tracked in git)
- `build_log.txt`
- `desktop/python/inference_output.txt`
- `desktop/python/init_full_log.txt`
- `desktop/python/test_init_out.txt` through `test_init_out_6.txt`
- `desktop/python/test_init_trace.txt`
- `desktop/python/shard_bridge.lib`
- `desktop/python/shard_engine.pdb`

#### Vendored C++ Directory
- `cpp/` contains **2,210 files** including llama.cpp source and vocab GGUF files
- This should be a **git submodule** or fetched at build time, not committed directly

#### Binary Files
- `shard_bridge.lib` (root)
- `shard_engine.dll` (root)
- `shard_engine.pdb` (root)
- `desktop/python/ggml-base.dll`, `ggml-cpu.dll`, `ggml.dll`, `llama.dll`
- These belong in CI release artifacts, not in the source tree

#### `nul` file at root
- Likely an accident from Windows — should be deleted and gitignored

#### Other
- `test_req.json` at root
- `shard_context_clean.txt` (25 MB), `shard_context_clean_no_json.txt` (73 MB), `shard_whitepaper_context.txt` (4.7 MB) — if these are tracked, they must be removed immediately

### Updated `.gitignore` Additions
```gitignore
# ─── Debug/Test Artifacts ───
**/test_init_out*.txt
**/inference_output.txt
**/init_full_log.txt
**/test_init_trace.txt
build_log.txt
test_req.json
test_output.txt

# ─── Binaries ───  
*.dll
*.pdb
*.lib
```

---

## 3. 🌐 Web Presence & Discoverability

### 3.1 Custom Domain
**Current:** `https://shard-trents-projects-20e9a51a.vercel.app`
**Problem:** This URL is unmemorable, unprofessional, and unshareable.

**Recommendation:** Register `shard.network` or `shardai.dev` and point it to Vercel. This is a **single-digit dollar investment** that 10x's credibility.

### 3.2 Landing Page
The current web app immediately boots into the technical chat UI. For first-time visitors who don't know what Shard is, this is confusing.

**Recommendation:** Create a `/` landing page route that explains the project before users opt in:
1. Hero with tagline
2. "How It Works" 3-step visual
3. Live network stats (peer count, requests served)
4. "Start Contributing" CTA → navigates to `/chat` (the current UI)
5. Links to GitHub, white paper, API docs

### 3.3 Open Graph / SEO
The current `layout.tsx` has basic metadata, but:
- No OG image for social sharing (Twitter cards, Discord embeds)
- No `robots.txt` or `sitemap.xml`
- No structured data (JSON-LD)

### 3.4 Public Icon Assets
`manifest.json` references `/icon-72.png` through `/icon-512.png` but the `web/public/` directory only has `manifest.json`, `sw.js`, and `swarm-worker.js`. **These icon files are missing.** Generate and add them.

---

## 4. 📖 Documentation Architecture

### 4.1 Docs Site
The `docs/` folder contains 19 Markdown files and a white paper. This is excellent content trapped in a hard-to-navigate format.

**Recommendation:** Deploy a docs site using one of:
- **Docusaurus** (React-based, great for API docs)
- **mkdocs-material** (Python-based, beautiful out of the box)
- **Nextra** (Next.js-based, could live in the existing web project)

Structure:
```
docs-site/
├── Getting Started
│   ├── What is Shard?
│   ├── Quick Start
│   └── Architecture Overview
├── Guides
│   ├── Running a Shard Node
│   ├── Running a Scout Node (Browser)
│   ├── Deploying with Docker
│   └── AWS / EC2 Deployment
├── API Reference
│   ├── Chat Completions
│   ├── System Endpoints
│   └── OpenAPI Spec
├── White Paper
├── Contributing
└── Troubleshooting
```

### 4.2 Document Version Drift
Several documents reference URLs and patterns that are inconsistent:
- `pyproject.toml` uses `ShardNetwork/Shard` as the GitHub org
- README uses `TrentPierce/Shard`
- API.md references `api.shard.network` (domain doesn't exist yet)
- `vercel.json` hardcodes IP `54.224.107.75` for API rewrites

**Align all references** to a single canonical source.

---

## 5. 👩‍💻 Developer Experience

### 5.1 One-Command Setup
Currently, getting the project running requires:
1. Clone the repo
2. `cd desktop/rust && cargo build --release`
3. Start the daemon
4. `cd desktop/python && python -m venv .venv && pip install -r requirements.txt && python run.py`
5. `cd web && npm install && npm run dev`

**Recommendation:** Create a root-level `Makefile` or a `justfile` with:
```makefile
.PHONY: dev
dev:           ## Start all services for local development
	@echo "Starting Shard daemon..."
	cd desktop/rust && cargo run --release &
	@echo "Starting Python API..."
	cd desktop/python && python run.py &
	@echo "Starting web UI..."
	cd web && npm run dev

.PHONY: setup
setup:         ## Install all dependencies
	cd desktop/rust && cargo build --release
	cd desktop/python && pip install -r requirements.txt
	cd web && npm install

.PHONY: test
test:          ## Run all test suites
	cd desktop/rust && cargo test
	cd desktop/python && pytest ../../tests/
	cd web && npm test

.PHONY: docker
docker:        ## Start with Docker Compose
	docker-compose up --build
```

### 5.2 Dev Container / Codespaces
Add a `.devcontainer/devcontainer.json` so contributors can spin up a preconfigured environment in GitHub Codespaces or VS Code:
```json
{
  "name": "Shard Dev",
  "image": "mcr.microsoft.com/devcontainers/universal:2",
  "features": {
    "ghcr.io/devcontainers/features/rust:1": {},
    "ghcr.io/devcontainers/features/python:1": { "version": "3.11" },
    "ghcr.io/devcontainers/features/node:1": { "version": "20" }
  },
  "postCreateCommand": "make setup"
}
```

### 5.3 Clean Up Test Files in `desktop/python/`
The `desktop/python/` directory has **38 files** including 12+ test/debug files (`test_dll.py`, `test_inference.py`, `test_inference_v2.py`, `test_inference_v3.py`, `test_init.py`, `test_init_local.py`, `test_init_v3.py`, `test_init_v4.py`, `diag.py`, `inspect_gguf.py`). These should be:
- Moved to `tests/` (the proper test directory)
- Or deleted if they're one-off debugging scripts

---

## 6. 🎯 Showcase & Demo Strategy

### 6.1 Interactive Demo Page
Create a `/demo` route on the web app that walks through the key concepts:
1. **Step 1:** "Your browser is loading a tiny AI model…" (WebLLM progress)
2. **Step 2:** "You're now a Scout node on the Shard mesh" (P2P visualization)
3. **Step 3:** "Ask anything — your browser drafts, the network verifies" (chat)

This guided experience would transform the app from "confusing technical tool" to "wow, I'm part of a distributed supercomputer."

### 6.2 Network Stats Dashboard
Add a public-facing stats page showing:
- Total Scout nodes online
- Total tokens generated
- Total peer connections
- Uptime / requests served
- Geographic distribution (if geolocation is tracked)

This provides **social proof** and makes the network feel alive.

### 6.3 "Pitch Mode" Improvements
The existing `Ctrl+Shift+P` pitch mode is great but hidden. Consider:
- Making it a URL parameter: `?pitch=true`
- Adding a "Watch Demo" button on the landing page that triggers it
- Adding commentary/annotations to the visualization

### 6.4 Badges for GitHub
Add these to README:
```markdown
[![Discord](https://img.shields.io/discord/YOUR_SERVER_ID?color=7289da&label=Discord&logo=discord)](https://discord.gg/YOUR_INVITE)
[![Twitter Follow](https://img.shields.io/twitter/follow/ShardNetwork?style=social)](https://twitter.com/ShardNetwork)
[![GitHub Stars](https://img.shields.io/github/stars/TrentPierce/Shard?style=social)](https://github.com/TrentPierce/Shard)
```

---

## 7. 🔧 Technical Debt

### 7.1 Scaffolded Inference Path
Per `AUDIT_REPORT.md`, `shard_api.py` still emits hardcoded scaffold tokens via `_stub_local_generate` and `_stub_verify`. For credibility:
- Wire the real BitNet path end-to-end
- If that's not ready, clearly label the scaffold in the API response (e.g., `"engine": "scaffold"`)
- Add a `/v1/system/status` endpoint showing whether real or scaffolded inference is active

### 7.2 `npm ci` Lockfile Mismatch
The CI reports `npm ci` failures due to `@types/react` mismatch. Fix:
```bash
cd web
rm package-lock.json
npm install
# Commit the new lockfile
```

### 7.3 Python SDK Completion
The `python-sdk/` is prominently listed in README but is a scaffold. Either:
- **Ship it** — wire it to the actual API endpoints
- **Remove it from README** until it works
- Add a clear "🚧 Experimental" badge

### 7.4 Environment Variable Documentation
The root `.env.example` and `desktop/python/.env.example` overlap and diverge. Consolidate into one source of truth with clear grouping and documentation.

### 7.5 CI Improvements
- Add a **web build** job (`npm run build`) — currently only lockfile verification exists
- Add **Rust clippy** linting: `cargo clippy -- -D warnings`
- Add **Python linting** with ruff: `ruff check desktop/python/`
- Add a **coverage report** step

---

## 8. 📊 Changelog Cleanup

The `CHANGELOG.md` has **duplicate version entries** (two `[0.3.0]` sections, two `[0.2.0]` sections) and orphaned content below `[0.0.1]`. This should be cleaned up to maintain credibility.

---

## 9. 🎨 Brand Assets

### Current State
- `assets/logo.png` exists (61 KB) — this is the only brand asset
- No banner image, no social card, no favicon variants
- No consistent color identity documented

### Recommendations
1. Create a `assets/` directory with:
   - `shard-banner.png` (1200×630 for social sharing)
   - `shard-og.png` (OG meta image)
   - `favicon.ico`, `icon-192.png`, `icon-512.png`
   - `shard-avatar.png` (for GitHub org profile)
2. Document the brand colors (the CSS already uses a beautiful palette: `#06060e`, `#00d4ff`, `#8b5cf6`)

---

## 10. 🗺️ Recommended Priority Roadmap

| Priority | Action | Impact | Effort |
|----------|--------|--------|--------|
| 🔴 P0 | Remove tracked binaries and debug logs from git | Repo credibility | 1 hour |
| 🔴 P0 | Fix `npm ci` lockfile mismatch | CI green | 15 min |
| 🔴 P0 | Register a custom domain | Professional presence | 30 min |
| 🟡 P1 | Redesign README with hero, badges, demo GIF | First-impression | 2 hours |
| 🟡 P1 | Create OG image and missing PWA icons | Social sharing | 1 hour |
| 🟡 P1 | Add landing page to web app | Onboarding | 4 hours |
| 🟡 P1 | Convert `cpp/` to git submodule | Repo size ↓ 90% | 1 hour |
| 🟢 P2 | Deploy docs site (Nextra or Docusaurus) | Discoverability | 4 hours |
| 🟢 P2 | Create Makefile/justfile for dev setup | Contributor DX | 1 hour |
| 🟢 P2 | Add devcontainer for Codespaces | Barrier to entry | 1 hour |
| 🟢 P2 | Interactive demo/onboarding flow | Wow factor | 6 hours |
| 🔵 P3 | Public network stats dashboard | Social proof | 4 hours |
| 🔵 P3 | Complete Python SDK | Ecosystem | 8 hours |
| 🔵 P3 | Add Discord/Twitter and community channels | Engagement | 2 hours |
| 🔵 P3 | Clean up changelog duplicates | Polish | 30 min |

---

## Summary Rating

| Area | Current | Potential |
|------|---------|-----------|
| **Technical Architecture** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| **Documentation Depth** | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| **README First Impression** | ⭐⭐ | ⭐⭐⭐⭐⭐ |
| **Repo Hygiene** | ⭐⭐ | ⭐⭐⭐⭐⭐ |
| **Developer Experience** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| **Web Presence / SEO** | ⭐⭐ | ⭐⭐⭐⭐⭐ |
| **Demo / Showcase-ability** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| **Community Ready** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| **Overall** | **6/10** | **10/10** |

The technical foundations are **outstanding**. The gap is entirely in packaging, presentation, and approachability. Executing on the P0 and P1 items above would move this project from "impressive to engineers who read the source" to "impressive to anyone who visits the repo."
