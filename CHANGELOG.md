# Changelog

All notable changes to Shard will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- (Coming soon) Full WebLLM integration in browser Scouts
- (Coming soon) Reputation scoring system for peer trust
- (Coming soon) Golden Ticket testing for Sybil attack prevention
- (Coming soon) gRPC control plane (replacing HTTP/JSON)

### Changed
- (Coming soon) Improve peer discovery efficiency

### Fixed
- (Coming soon) Fix connection timeout handling

### Removed
- None

---

## [0.4.0] - 2024-12-20

### Added
- **P2P Networking**: Full libp2p implementation with TCP and WebSocket transports
- **Gossipsub Protocol**: Distributed pub/sub for work distribution (`shard-work`, `shard-work-result` topics)
- **Kademlia DHT**: Peer discovery and content routing
- **OpenAI-Compatible API**: Full `/v1/chat/completions` endpoint with streaming support
- **SSE Streaming**: Server-sent events for real-time token streaming
- **Handshake Protocol**: PING/PONG verification for peer health
- **Request/Response Protocol**: Work request forwarding and draft verification
- **Rust Daemon Control Plane**: HTTP API on port 9091 for daemon management
- **Python Oracle API**: FastAPI-based driver API on port 8000
- **BitNet Bridge**: In-process ctypes bridge for local model verification
- **API Authentication**: Optional API key authentication via `SHARD_API_KEYS`
- **Rate Limiting**: Per-client rate limiting via `SHARD_RATE_LIMIT_PER_MINUTE`
- **CORS Support**: Configurable CORS origins
- **Prometheus Metrics**: Built-in metrics endpoint at `/metrics`
- **Health Endpoints**: `/health`, `/v1/system/topology`, `/v1/system/peers`
- **Bootstrap Configuration**: Bootstrap peers via CLI or file
- **Periodic Reconnection**: Automatic peer reconnection
- **Data Persistence**: Peer and topology persistence
- **Systemd Support**: Linux service integration
- **WebRTC Support**: WebRTC-direct transport (Linux/macOS)
- **Control Plane Proto**: Protocol buffer definitions for future gRPC migration
- **Comprehensive Documentation**: API docs, deployment guides, and architecture documentation

### Changed
- **Architecture**: Moved from monolithic to hybrid Python/Rust architecture
- **Networking**: Replaced HTTP-based networking with libp2p P2P mesh
- **Verification**: Improved verification logic with stricter prefix matching

### Fixed
- Connection timeout handling for peer bootstrap
- Memory leak in draft token verification
- Race condition in gossipsub subscription
- CORS preflight handling
- Rate limit header propagation

### Removed
- Legacy HTTP-based work distribution
- Direct file IPC (replaced with HTTP control plane)
- Experimental REST API (replaced with OpenAI-compatible endpoint)

### Security
- Added input validation for all endpoints
- Implemented proper error handling and logging
- Added API key authentication support
- Rate limiting to prevent abuse
- Prompt size limits (`SHARD_MAX_PROMPT_CHARS`)

---

## [0.3.0] - 2024-11-15

### Added
- Initial proof-of-concept P2P architecture
- Basic Python FastAPI endpoints
- Simple file-based IPC between components

### Changed
- Project structure reorganization

---

## [0.2.0] - 2024-10-01

### Added
- Basic Chat API implementation
- Single-machine inference support

---

## [0.1.0] - 2024-09-01

### Added
- Initial project scaffolding
- Basic README documentation
- Project structure setup

---

## Versioning

Shard follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html):

- **MAJOR**: Breaking changes (incompatible API changes, data format changes)
- **MINOR**: Backward-compatible new features (new endpoints, optional parameters)
- **PATCH**: Backward-compatible bug fixes

Pre-release versions use `-alpha`, `-beta`, or `-rc` suffixes.

### Release Cadence

Releases are typically made on a monthly basis or when significant features are complete:

- **Alpha**: Early development, may have breaking changes
- **Beta**: Feature complete, minor breaking changes possible
- **Release Candidate (RC)**: Stable, only critical bug fixes
- **Stable**: Production-ready, follows semantic versioning

---

## How to Contribute

When contributing to Shard, please add entries to the "Unreleased" section following the format above.

### Adding Changelog Entries

```markdown
### Added
- Short description of new feature

### Changed
- Short description of change

### Fixed
- Short description of fix

### Removed
- Short description of removal

### Security
- Short description of security fix
```

### Guidelines

- Use one line per change
- Keep descriptions concise and clear
- Use present tense ("Add" not "Added")
- Link to relevant issues or pull requests when helpful
- Include user-facing changes only
- Separate sections with headers

### Release Process

Before releasing:

1. Move all entries from "Unreleased" to new version section
2. Update version number in all relevant files
3. Add release date
4. Review and edit entries for clarity
5. Commit and tag the release:
   ```bash
   git tag -a v0.5.0 -m "Release v0.5.0"
   git push origin v0.5.0
   ```
6. Create GitHub release with changelog excerpt
7. Announce to community

---

## Links

- [Releases](https://github.com/ShardNetwork/Shard/releases)
- [Issues](https://github.com/ShardNetwork/Shard/issues)
- [Pull Requests](https://github.com/ShardNetwork/Shard/pulls)
- [Documentation](https://github.com/ShardNetwork/Shard/tree/main/docs)
