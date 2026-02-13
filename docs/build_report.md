# Shard Release Build Report

**Generated:** February 12, 2026  
**Version:** 0.3.0 → 0.4.0  
**Status:** IMPLEMENTATION COMPLETE

---

## Summary

This document records all changes made to prepare Shard for production release and VC pitch demonstration.

---

## Phase 1: Visual Proof (Frontend & Demo)

### ✅ NetworkVisualizer.tsx
**Location:** `web/src/components/NetworkVisualizer.tsx`

Implemented a live force-directed graph visualization:
- Real-time peer network visualization using HTML5 Canvas
- Local Oracle node at center (green)
- Connected Scout nodes (blue)
- Force-directed physics simulation
- TPS and latency stats overlay
- Pitch Mode indicator

### ✅ Pitch Mode Controls (Ctrl+Shift+P)
**Location:** `web/src/app/page.tsx`

Added demo controls:
- **Spawn Bot**: Adds simulated Scout node to graph, spikes TPS counter
- **Kill Bot**: Randomly removes a peer node with rerouting animation
- Toast notifications for all events
- Keyboard shortcut: `Ctrl+Shift+P`

### ✅ Demo Resilience (Backend)
**Location:** `desktop/python/inference.py`

Modified cooperative generation:
- Added `PITCH_MODE` environment variable detection
- Immediate peer rerouting on failure (0ms delay in pitch mode)
- Rerouting event logging for toast notifications
- Throttled logging to prevent spam

### ✅ Toast Notification System
**Location:** `web/src/app/page.tsx`

Added toast notification UI:
- Fixed position overlay at top of screen
- Auto-dismiss after 4 seconds
- Used for peer join/leave events, rerouting notifications

---

## Phase 2: Core Hardening (Backend)

### ✅ ResourceFetcher
**Location:** `desktop/python/resource_fetcher.py`

First-run model downloader:
- Checks for `models/Llama-3.2-1B-Instruct-Q4_K_M.gguf`
- Tkinter splash screen with progress bar
- Downloads from HuggingFace (with fallback URLs)
- SHA256 checksum verification
- Retry on failure

### ✅ GlobalCrashHandler
**Location:** `desktop/python/crash_handler.py`

Crash handling and recovery:
- Global exception handler (`sys.excepthook`)
- Crash logging to `%APPDATA%/Shard/crash.log`
- Auto-restart after 3 seconds
- Detailed crash reports with traceback

### ✅ Auto-Updater
**Location:** `desktop/python/crash_handler.py`

GitHub release checking:
- Queries `api.github.com/repos/TrentPierce/Shard/releases/latest`
- Compares `tag_name` vs local version
- Prompts user for update
- Downloads new version

---

## Phase 3: Global Pipeline (CI/CD)

### ✅ global_release.yml
**Location:** `.github/workflows/global_release.yml`

Multi-platform CI/CD pipeline:

| Platform | Artifact | Build Tool |
|----------|----------|------------|
| Windows | `Shard_Setup_v*.exe` | PyInstaller |
| Linux | `Shard_v*.AppImage` | AppImage |
| macOS | `Shard_v*.dmg` | DMG |

**Triggers:**
- Tag push (`v*`)
- Manual workflow dispatch

**Jobs:**
1. `build-windows` - Windows x64 executable
2. `build-linux` - Linux AppImage
3. `build-macos` - Universal macOS DMG
4. `release` - Creates GitHub Release
5. `verify` - Confirms release assets

---

## Phase 4: Local Gold Master

### ✅ run_pitch_demo.bat
**Location:** `run_pitch_demo.bat`

Windows launcher script:
- Sets `SHARD_PITCH_MODE=1`
- Auto-opens browser at localhost:3000
- Displays demo controls
- Supports debug mode

### Build Script (Existing)
**Location:** `scripts/build_release.py`

Already functional:
- Builds Rust daemon
- Builds C++ engine
- PyInstaller bundling
- Version manifest generation

---

## Phase 5: Verification

### Release Tests
**Location:** `tests/release_test.py`

Existing tests:
- `test_symbol_check_shard_rollback_callable` - Engine DLL exports
- `test_boot_test_shard_api_launches` - App starts and binds port
- `test_loop_double_dip_and_remote_gate_contract` - Worker assets

---

## Files Created/Modified

### New Files
| File | Purpose |
|------|---------|
| `web/src/components/NetworkVisualizer.tsx` | Force-directed network graph |
| `desktop/python/resource_fetcher.py` | Model downloader with splash |
| `desktop/python/crash_handler.py` | Crash handler + auto-updater |
| `.github/workflows/global_release.yml` | Multi-platform CI/CD |
| `run_pitch_demo.bat` | Windows pitch launcher |

### Modified Files
| File | Changes |
|------|---------|
| `web/src/app/page.tsx` | Added pitch mode, toast, visualizer |
| `desktop/python/inference.py` | Added pitch mode resilience |

---

## Build Instructions

### Local Build (Windows)
```bash
# Install dependencies
pip install pyinstaller
pip install -r desktop/python/requirements.txt
npm install
npm run build

# Run build
python scripts/build_release.py

# Launch pitch demo
.\run_pitch_demo.bat
```

### CI/CD Release
```bash
# Create version tag
git tag v0.4.0
git push origin v0.4.0
```

---

## Verification Checklist

- [ ] NetworkVisualizer renders force-directed graph
- [ ] Ctrl+Shift+P toggles pitch mode
- [ ] Spawn Bot adds node and spikes TPS
- [ ] Kill Bot removes node with log message
- [ ] Toast notifications appear for events
- [ ] ResourceFetcher shows splash on missing model
- [ ] Crash handler logs to %APPDATA%/Shard/crash.log
- [ ] Auto-updater checks GitHub releases
- [ ] CI workflow builds all platforms
- [ ] run_pitch_demo.bat launches app

---

## Next Steps

1. **Update version** in `Cargo.toml` to 0.4.0
2. **Update checksums** in `resource_fetcher.py` with real model hashes
3. **Configure GitHub secrets** for release signing (optional)
4. **Test build** locally before tagging release

---

## Known Limitations

- Model checksum is placeholder (must update with real hash)
- AppImage creation requires `appimgetool` on Linux
- Universal binary for macOS requires both architectures built

---

*End of Report*
