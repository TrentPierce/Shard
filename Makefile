.PHONY: help setup dev test lint docker clean version version-sync version-set

# Default target
help: ## Show this help message
	@echo ""

version: ## Show current unified project version
	@cat VERSION

version-sync: ## Sync all component versions from VERSION file
	python scripts/sync_versions.py

version-set: ## Set a new version and sync all component versions (usage: make version-set V=0.6.0)
	@if [ -z "$(V)" ]; then echo "Usage: make version-set V=0.6.0"; exit 1; fi
	python scripts/sync_versions.py --set $(V)
	@echo "  Shard — Distributed Inference Network"
	@echo "  ======================================"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk -F ':.*?## ' '{printf "  %-15s %s\n", $$1, $$2}'
	@echo ""

# ── Setup ──────────────────────────────────────────────────────────

setup: setup-rust setup-web ## Install all dependencies
	@echo "\n✅ All dependencies installed."

setup-rust: ## Build the Rust daemon
	@echo "🦀 Building Rust daemon..."
	cd desktop/rust && cargo build --release

setup-web: ## Install web dependencies
	@echo "🌐 Installing web dependencies..."
	cd web && npm install

# ── Development ────────────────────────────────────────────────────

dev: ## Start all services for local development
	@echo "🚀 Starting Shard services..."
	@echo ""
	@echo "Starting Rust daemon on :9091..."
	@cd desktop/rust && cargo run --release -- --control-port 9091 --tcp-port 4001 &
	@sleep 2
	@echo "Starting web UI on :3000..."
	@cd web && npm run dev

dev-web: ## Start only the web UI
	cd web && npm run dev

dev-daemon: ## Start only the Rust daemon
	cd desktop/rust && cargo run --release -- --control-port 9091 --tcp-port 4001

# ── Tests ──────────────────────────────────────────────────────────

test: test-rust test-web ## Run all test suites
	@echo "\n✅ All tests passed."

test-rust: ## Run Rust tests
	@echo "🦀 Running Rust tests..."
	cd desktop/rust && cargo test --all-targets

test-web: ## Run web tests
	@echo "🌐 Running web tests..."
	cd web && npm test -- --passWithNoTests

# ── Linting ────────────────────────────────────────────────────────

lint: lint-rust lint-web ## Run all linters
	@echo "\n✅ All linting passed."

lint-rust: ## Run Rust clippy
	cd desktop/rust && cargo clippy -- -D warnings

lint-web: ## Run web linter
	cd web && npm run lint 2>/dev/null || true

# ── Docker ─────────────────────────────────────────────────────────

docker: ## Start with Docker Compose
	docker-compose up --build

docker-monitoring: ## Start with monitoring stack (Prometheus + Grafana)
	docker-compose --profile monitoring up --build

# ── Cleanup ────────────────────────────────────────────────────────

clean: ## Remove build artifacts
	@echo "🧹 Cleaning build artifacts..."
	rm -rf desktop/rust/target/
	rm -rf web/.next/ web/out/ web/node_modules/
	rm -rf build/ dist/
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	@echo "✅ Clean."
