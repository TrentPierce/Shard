.PHONY: help setup dev test lint docker clean

# Default target
help: ## Show this help message
	@echo ""
	@echo "  Shard — Distributed Inference Network"
	@echo "  ======================================"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'
	@echo ""

# ── Setup ──────────────────────────────────────────────────────────

setup: setup-rust setup-python setup-web ## Install all dependencies
	@echo "\n✅ All dependencies installed."

setup-rust: ## Build the Rust daemon
	@echo "🦀 Building Rust daemon..."
	cd desktop/rust && cargo build --release

setup-python: ## Install Python dependencies
	@echo "🐍 Setting up Python environment..."
	cd desktop/python && python -m pip install -r requirements.txt

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
	@echo "Starting Python API on :8000..."
	@cd desktop/python && python run.py --rust-url http://127.0.0.1:9091 &
	@sleep 1
	@echo "Starting web UI on :3000..."
	@cd web && npm run dev

dev-web: ## Start only the web UI
	cd web && npm run dev

dev-api: ## Start only the Python API
	cd desktop/python && python run.py --rust-url http://127.0.0.1:9091

dev-daemon: ## Start only the Rust daemon
	cd desktop/rust && cargo run --release -- --control-port 9091 --tcp-port 4001

# ── Tests ──────────────────────────────────────────────────────────

test: test-rust test-python test-web ## Run all test suites
	@echo "\n✅ All tests passed."

test-rust: ## Run Rust tests
	@echo "🦀 Running Rust tests..."
	cd desktop/rust && cargo test --all-targets

test-python: ## Run Python tests
	@echo "🐍 Running Python tests..."
	python -m pytest -q tests/

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
