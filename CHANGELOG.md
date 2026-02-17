# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **OpenAPI Documentation**: Enhanced FastAPI app with comprehensive OpenAPI 3.1 spec generation, detailed Field() documentation with examples, organized endpoint tags, and enhanced descriptions
- **Tag Organization**: Added endpoint tags (chat, scouts, system, admin) for better documentation organization in Swagger UI
- **Response Examples**: Added comprehensive examples to all request/response schemas matching OpenAI API specification
- **Error Documentation**: Enhanced error response documentation with detailed status codes and error types
- **Developer Experience**: Added Makefile with `setup`, `dev`, `test`, `lint`, and `docker` targets
- **Dev Container**: Added `.devcontainer/devcontainer.json` for GitHub Codespaces support

### Changed
- **README**: Complete redesign with hero section, logo, badges, "Why Shard?" table, and structured quick-start guide
- **API Documentation Enhancement**: Improved API.md with comprehensive OpenAI compatibility details, architecture diagrams, and deployment guidance
- **Model Documentation**: Enhanced Pydantic models (Message, ChatRequest, Choice, ChatResponse) with detailed Field() descriptions and examples

### Fixed
- **Repository Hygiene**: Removed tracked debug logs, binary files, and test artifacts from version control
- **Documentation Clarity**: Fixed typos and improved clarity in API.md and README.md

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
- **Python Shard API**: FastAPI-based driver API on port 8000
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
- **Golden Ticket Security**: Sybil attack prevention through verification prompts
- **Reputation System**: Scout accuracy tracking for trust management

### Changed
- **Architecture**: Moved from monolithic to hybrid Python/Rust architecture
- **Networking**: Replaced HTTP-based networking with libp2p P2P mesh
- **Verification**: Improved verification logic with stricter prefix matching
- **API Structure**: Restructured API.md with comprehensive architecture documentation
- **Node Classification**: Improved node class documentation (Shard, Scout, Leech)

### Fixed
- **Connection Handling**: Fixed connection timeout handling and reconnection logic
- **Rate Limiting**: Improved rate limiter precision and error reporting
- **Metrics**: Enhanced Prometheus metrics with more granular counters
- Connection timeout handling for peer bootstrap
- Memory leak in draft token verification
- Race condition in gossipsub subscription
- CORS preflight handling
- Rate limit header propagation

### Security
- **Golden Tickets**: Implemented Golden Ticket mechanism for Sybil attack prevention
- **Reputation System**: Added scout reputation tracking and banning mechanism
- Added input validation for all endpoints
- Implemented proper error handling and logging
- Added API key authentication support
- Rate limiting to prevent abuse
- Prompt size limits (`SHARD_MAX_PROMPT_CHARS`)

### Removed
- Legacy HTTP-based work distribution
- Direct file IPC (replaced with HTTP control plane)
- Experimental REST API (replaced with OpenAI-compatible endpoint)

### Dependencies
- **Python**: Added FastAPI (0.115.0+), Pydantic (2.9.0+), httpx (0.27.0+)
- **Rust**: Added libp2p (0.54+), axum (0.7+), tokio (1.0+)
- **Web**: Added Next.js 14, React 18, libp2p for browser
- **WebLLM**: Added @mlc-ai/web-llm for browser draft token generation

### Performance
- **Efficiency**: Improved cooperative generation loop efficiency
- **Parallel Processing**: Added concurrent Scout task processing

### Documentation
- **README**: Comprehensive project documentation with architecture diagrams
- **API.md**: Detailed API reference with examples
- **Production Readiness Plan**: Phased production deployment roadmap
- **Deployment Guide**: Multi-component deployment instructions
- **Troubleshooting**: Common issues and solutions guide

[unreleased]: https://github.com/TrentPierce/Shard/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/TrentPierce/Shard/compare/v0.3.0...v0.4.0

---

## [0.3.0] - 2024-11-15

### Added
- Initial proof-of-concept P2P architecture
- Basic Python FastAPI endpoints
- Simple file-based IPC between components
- **Golden Ticket Framework**: Foundation for Sybil attack prevention

### Changed
- Project structure reorganization
- Initial handshake and verification protocol design

[0.3.0]: https://github.com/TrentPierce/Shard/compare/v0.2.0...v0.3.0

---

## [0.2.0] - 2024-10-01

### Added
- **BitNet Runtime**: ctypes bridge for local model verification
- **Cooperative Generation**: Hybrid Shard+Scout inference loop
- Basic Chat API implementation
- Single-machine inference support
- **Basic Authentication**: API key authentication framework

### Changed
- **API Design**: Initial FastAPI-based API structure

[0.2.0]: https://github.com/TrentPierce/Shard/compare/v0.1.0...v0.2.0

---

## [0.1.0] - 2024-09-01

### Added
- Initial project scaffolding
- Basic README documentation
- Project structure setup

[0.1.0]: https://github.com/TrentPierce/Shard/releases/tag/v0.1.0

---

## Versioning

Shard follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html):

- **MAJOR**: Breaking changes (incompatible API changes, data format changes)
- **MINOR**: Backward-compatible new features (new endpoints, optional parameters)
- **PATCH**: Backward-compatible bug fixes

### Release Cadence

- **Alpha**: Early development, may have breaking changes
- **Beta**: Feature complete, minor breaking changes possible
- **Release Candidate (RC)**: Stable, only critical bug fixes
- **Stable**: Production-ready, follows semantic versioning

---

## How to Contribute

When contributing to Shard, please add entries to the "Unreleased" section following the format above.

### Guidelines

- Use one line per change
- Keep descriptions concise and clear
- Use present tense ("Add" not "Added")
- Link to relevant issues or pull requests when helpful
- Include user-facing changes only

---

## Links

- [Releases](https://github.com/TrentPierce/Shard/releases)
- [Issues](https://github.com/TrentPierce/Shard/issues)
- [Pull Requests](https://github.com/TrentPierce/Shard/pulls)
- [Documentation](https://github.com/TrentPierce/Shard/tree/main/docs)
