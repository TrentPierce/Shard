# Contributing to Shard

Thank you for your interest in contributing to Shard! We welcome contributions from everyone, whether you're fixing bugs, adding features, improving documentation, or sharing ideas.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Project Structure](#project-structure)
- [How to Contribute](#how-to-contribute)
- [Style Guides](#style-guides)
- [Testing Requirements](#testing-requirements)
- [Commit Message Format](#commit-message-format)
- [Release Process](#release-process)
- [Community Guidelines](#community-guidelines)

---

## Code of Conduct

We are committed to providing a welcoming and inclusive environment for all contributors. Please:

- Be respectful and constructive
- Welcome newcomers and help them learn
- Focus on what is best for the community
- Show empathy towards other community members

If you encounter any issues or have concerns, please contact the maintainers privately.

---

## Getting Started

### Prerequisites

Before contributing, ensure you have the following installed:

- **Rust** (1.75+) — [rustup.rs](https://rustup.rs)
- **Python** (3.11+) — with pip
- **Node.js** (18+) — with npm
- **Git** — for version control

### Fork and Clone

1. Fork the repository on GitHub
2. Clone your fork locally:

```bash
git clone https://github.com/YOUR_USERNAME/Shard.git
cd Shard
```

3. Add the upstream remote:

```bash
git remote add upstream https://github.com/ShardNetwork/Shard.git
```

### Build the Project

#### Build Rust Daemon

```bash
cd desktop/rust
cargo build --release
```

The compiled binary will be at `desktop/rust/target/release/shard-daemon.exe` (Windows) or `shard-daemon` (Linux/Mac).

#### Setup Python Environment

```bash
cd desktop/python
python -m venv venv

# Windows
venv\Scripts\activate

# Linux/macOS
source venv/bin/activate

pip install -r requirements.txt
pip install -r requirements-dev.txt  # Development dependencies
```

#### Setup Web Client

```bash
cd web
npm install
```

### Run Tests

```bash
# Rust tests
cd desktop/rust
cargo test

# Python tests
cd desktop/python
pytest

# Web tests
cd web
npm test
```

---

## Development Workflow

### Branching Strategy

We use a simplified Git workflow:

- **`main`** — Production-ready code
- **`develop`** — Integration branch for features
- **`feature/*`** — Feature branches (e.g., `feature/webgpu-support`)
- **`bugfix/*`** — Bug fix branches (e.g., `bugfix/memory-leak`)
- **`docs/*`** — Documentation updates

### Workflow Steps

1. **Create a feature branch** from `develop`:

```bash
git checkout develop
git pull upstream develop
git checkout -b feature/your-feature-name
```

2. **Make your changes** following the style guides
3. **Commit your changes** with proper messages (see [Commit Message Format](#commit-message-format))
4. **Push to your fork**:

```bash
git push origin feature/your-feature-name
```

5. **Create a Pull Request** to `develop` (for new features) or `main` (for urgent bug fixes)

### Pull Request Guidelines

- **Title**: Clear and descriptive
- **Description**: Explain the "why" and "how"
- **Linked Issues**: Reference related issues using `#123`
- **Screenshots/GIFs**: For UI changes
- **Tests**: Include tests for new functionality
- **Docs**: Update relevant documentation

### Code Review Process

1. All PRs require at least one approval
2. Address all review comments
3. Ensure CI checks pass
4. Maintain minimal conflicts with target branch

---

## Project Structure

```
Shard/
├── desktop/
│   ├── control_plane/         # Protocol definitions (protobuf)
│   ├── python/                # Shard API (FastAPI)
│   │   ├── run.py            # Entry point
│   │   ├── shard_api.py     # API endpoints
│   │   ├── inference.py      # Cooperative generation
│   │   └── bitnet/           # BitNet runtime bridge
│   └── rust/                 # P2P networking daemon
│       ├── src/
│       │   └── main.rs       # Main daemon implementation
│       └── Cargo.toml        # Rust dependencies
├── web/                      # Next.js web client
│   ├── app/                  # Next.js App Router
│   ├── components/           # React components
│   └── package.json
├── docs/                     # Documentation
│   ├── deployment-guide.md
│   ├── troubleshooting.md
│   └── ...
├── CONTRIBUTING.md           # This file
├── README.md                 # Main project README
└── CHANGELOG.md              # Version history
```

### Component Responsibilities

| Component | Language | Purpose |
|-----------|----------|---------|
| **Python Shard API** | Python | OpenAI-compatible API, inference orchestration |
| **Rust Daemon** | Rust | P2P networking (libp2p), peer discovery |
| **Web Client** | TypeScript/React | Browser UI, WebLLM integration |
| **BitNet Bridge** | Python | Local model verification bridge |

---

## How to Contribute

### Reporting Bugs

Before reporting a bug:

1. Check existing [issues](https://github.com/ShardNetwork/Shard/issues)
2. Search the documentation for known solutions
3. Reproduce the issue with the latest version

When reporting, include:

- Clear title and description
- Steps to reproduce
- Expected vs actual behavior
- Environment details (OS, versions)
- Logs and error messages
- Screenshots if applicable

Use the bug report template:

```markdown
**Bug Description**
[Description of the bug]

**Steps to Reproduce**
1. ...
2. ...

**Expected Behavior**
[What should happen]

**Actual Behavior**
[What actually happens]

**Environment**
- OS: [e.g., Windows 11]
- Rust version: [e.g., 1.75.0]
- Python version: [e.g., 3.11.5]
- Node version: [e.g., 18.17.0]

**Logs**
[Relevant log output]
```

### Suggesting Features

We love feature suggestions! When proposing:

- Check for existing feature requests
- Clearly describe the use case
- Explain why it would benefit the project
- Consider implementation complexity

Use the feature request template:

```markdown
**Feature Description**
[Description of the feature]

**Use Case**
[Why is this needed? What problem does it solve?]

**Proposed Solution**
[How should it work?]

**Alternatives Considered**
[Other approaches considered and why they were rejected]
```

### Improving Documentation

Documentation contributions are highly valued:

- Fix typos and grammar
- Clarify confusing sections
- Add missing examples
- Translate to other languages
- Create tutorials and guides

---

## Style Guides

### Python Style Guide

Follow [PEP 8](https://pep8.org/) and [PEP 257](https://peps.python.org/pep-0257/) with these additions:

#### Formatting

- Use 4 spaces for indentation (no tabs)
- Maximum line length: 88 characters (Black default)
- Use `black` for auto-formatting:

```bash
pip install black
black desktop/python/
```

#### Imports

```python
# Standard library imports
import asyncio
import logging
from typing import Any

# Third-party imports
import httpx
from fastapi import FastAPI

# Local imports
from bitnet.ctypes_bridge import BitNetRuntime
```

#### Type Hints

Use type hints for all function signatures:

```python
from typing import AsyncIterator

async def generate_tokens(
    prompt: str,
    max_tokens: int,
) -> AsyncIterator[str]:
    """Generate tokens from the given prompt."""
    ...
```

#### Docstrings

Use Google-style docstrings:

```python
def verify_draft(generated: list[str], draft: list[str]) -> tuple[list[str], str | None]:
    """Verify a draft token sequence against the generated context.

    Args:
        generated: The already-generated token context.
        draft: The draft tokens to verify.

    Returns:
        A tuple of (accepted_tokens, correction_or_none).

    Raises:
        RuntimeError: If verification fails unexpectedly.
    """
    ...
```

#### Linting

```bash
pip install pylint pylint-pytest mypy
pylint desktop/python/
mypy desktop/python/
```

### Rust Style Guide

Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) and use standard formatting:

#### Formatting

Use `rustfmt` for consistent formatting:

```bash
cd desktop/rust
cargo fmt
```

#### Naming

- Types: `PascalCase` (e.g., `WorkRequest`)
- Functions: `snake_case` (e.g., `validate_work_request`)
- Constants: `SCREAMING_SNAKE_CASE` (e.g., `MAX_TOKENS`)
- Modules: `snake_case`

#### Error Handling

Use `Result<T, E>` and `anyhow` for application errors:

```rust
use anyhow::Result;

fn validate_work_request(req: &WorkRequest) -> Result<(), String> {
    if req.request_id.trim().is_empty() {
        return Err("request_id must be non-empty".into());
    }
    Ok(())
}
```

#### Documentation

Use `///` for public APIs:

```rust
/// Validates a work request before processing.
///
/// # Arguments
///
/// * `req` - The work request to validate
///
/// # Returns
///
/// Returns `Ok(())` if valid, `Err(String)` with details otherwise.
pub fn validate_work_request(req: &WorkRequest) -> Result<(), String> {
    ...
}
```

#### Clippy

Run Clippy for additional linting:

```bash
cd desktop/rust
cargo clippy -- -D warnings
```

### TypeScript/React Style Guide

Follow standard practices for Next.js and React:

#### Formatting

Use [Prettier](https://prettier.io/) and [ESLint](https://eslint.org/):

```bash
cd web
npm run lint
npm run format
```

#### Component Structure

```typescript
// Prefer functional components with hooks
import { useState, useEffect } from 'react';

interface MyComponentProps {
  title: string;
  onAction: () => void;
}

export function MyComponent({ title, onAction }: MyComponentProps) {
  const [count, setCount] = useState(0);

  useEffect(() => {
    // Effect logic
  }, [count]);

  return (
    <div className="my-component">
      <h2>{title}</h2>
      <button onClick={onAction}>Action</button>
    </div>
  );
}
```

#### TypeScript Rules

- Enable strict mode in `tsconfig.json`
- Avoid `any` type
- Use proper interface definitions
- Prefer `const` assertions for literals

---

## Testing Requirements

### Python Tests

- Use `pytest` as the test framework
- Write tests for all new functionality
- Maintain at least 80% code coverage

```bash
cd desktop/python
pytest tests/
pytest --cov=. --cov-report=html
```

### Rust Tests

- Write unit tests alongside code
- Use integration tests for external interactions

```bash
cd desktop/rust
cargo test
cargo test --release
```

### Web Tests

- Use React Testing Library for component tests
- Write E2E tests for critical user flows

```bash
cd web
npm test
npm run test:e2e
```

### Test Structure

```
tests/
├── unit/
│   ├── test_shard_api.py
│   └── test_inference.py
├── integration/
│   └── test_p2p_network.py
└── fixtures/
    └── mock_data.py
```

---

## Commit Message Format

We follow a simplified [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

| Type | Usage |
|------|-------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation changes |
| `style` | Code style changes (formatting) |
| `refactor` | Code refactoring |
| `test` | Adding or updating tests |
| `chore` | Build process, dependencies, etc. |
| `perf` | Performance improvements |

### Examples

```
feat(shard): add support for streaming responses

Implements SSE streaming for the chat completions endpoint
to reduce latency and improve user experience.

Closes #123
```

```
fix(p2p): handle connection timeouts gracefully

Previously, connection timeouts would cause the daemon to crash.
Now they are logged and retried automatically.

Fixes #456
```

```
docs(readme): update installation instructions

Clarified the prerequisite steps and added troubleshooting tips.
```

---

## Release Process

Releases follow [Semantic Versioning](https://semver.org/) (MAJOR.MINOR.PATCH):

- **MAJOR**: Breaking changes
- **MINOR**: New features (backward compatible)
- **PATCH**: Bug fixes (backward compatible)

### Release Checklist

1. Update version in all relevant files
2. Update `CHANGELOG.md`
3. Run full test suite
4. Tag the release:

```bash
git tag -a v0.5.0 -m "Release v0.5.0"
git push origin v0.5.0
```

5. Create GitHub release with notes
6. Update documentation
7. Announce to community

---

## Community Guidelines

### Communication Channels

- **GitHub Issues**: Bug reports, feature requests
- **GitHub Discussions**: Questions, ideas, general discussion
- **Pull Requests**: Code contributions

### Asking for Help

- Search existing issues and discussions first
- Provide context and details
- Be patient and respectful
- Help others when you can

### Recognition

Contributors will be recognized in:

- CONTRIBUTORS.md file
- Release notes
- README acknowledgments section

---

## Getting Help

If you need assistance:

1. Check the [documentation](https://github.com/ShardNetwork/Shard/tree/main/docs)
2. Search existing [issues](https://github.com/ShardNetwork/Shard/issues)
3. Start a [discussion](https://github.com/ShardNetwork/Shard/discussions)
4. Ask in the community chat (if available)

---

## License

By contributing to Shard, you agree that your contributions will be licensed under the project's license (see LICENSE file).

---

**Happy contributing! 🚀**
