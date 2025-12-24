# Makefile for Vietnam Enterprise Cron System
# Optimized for macOS Apple Silicon

.PHONY: help build build-release clean test run-scheduler run-worker run-api docker-build

# Default target
help:
	@echo "Vietnam Enterprise Cron - Build Commands"
	@echo ""
	@echo "Development:"
	@echo "  make build          - Build debug binaries (fast)"
	@echo "  make build-release  - Build optimized binaries (slow, small)"
	@echo "  make clean          - Clean build artifacts"
	@echo "  make test           - Run all tests"
	@echo ""
	@echo "Run services:"
	@echo "  make run-scheduler  - Start scheduler"
	@echo "  make run-worker     - Start worker"
	@echo "  make run-api        - Start API server"
	@echo ""
	@echo "Docker:"
	@echo "  make docker-build   - Build Docker image"
	@echo ""

# Build debug version (fast compilation)
build:
	@echo "🔨 Building debug binaries..."
	@export RUST_MIN_STACK=16777216 && \
	export CARGO_BUILD_JOBS=4 && \
	export CARGO_INCREMENTAL=0 && \
	cargo build
	@echo "✅ Build completed: target/debug/{scheduler,worker,api}"

# Build release version (optimized)
build-release:
	@echo "🚀 Building release binaries..."
	@export RUST_MIN_STACK=16777216 && \
	export CARGO_BUILD_JOBS=4 && \
	export CARGO_INCREMENTAL=0 && \
	cargo build --release
	@echo "✅ Build completed: target/release/{scheduler,worker,api}"
	@echo "📦 Binary sizes:"
	@ls -lh target/release/ | grep -E "(scheduler|worker|api)$$"

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	@cargo clean
	@echo "✅ Clean completed"

# Run tests
test:
	@echo "🧪 Running tests..."
	@export RUST_MIN_STACK=16777216 && \
	cargo test --lib
	@echo "✅ Tests completed"

# Run scheduler
run-scheduler:
	@echo "📅 Starting scheduler..."
	@export RUST_MIN_STACK=16777216 && \
	cargo run --bin scheduler

# Run worker
run-worker:
	@echo "⚙️  Starting worker..."
	@export RUST_MIN_STACK=16777216 && \
	cargo run --bin worker

# Run API server
run-api:
	@echo "🌐 Starting API server..."
	@export RUST_MIN_STACK=16777216 && \
	cargo run --bin api

# Build Docker image
docker-build:
	@echo "🐳 Building Docker image..."
	@docker build -t vietnam-cron:latest .
	@echo "✅ Docker image built: vietnam-cron:latest"

# Check code quality
check:
	@echo "🔍 Checking code..."
	@cargo check --all-targets
	@echo "✅ Check completed"

# Format code
fmt:
	@echo "✨ Formatting code..."
	@cargo fmt --all
	@echo "✅ Format completed"

# Run clippy lints
clippy:
	@echo "📎 Running clippy..."
	@cargo clippy --all-targets -- -D warnings
	@echo "✅ Clippy completed"
