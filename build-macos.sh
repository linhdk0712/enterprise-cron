#!/bin/bash
# Build script for macOS Apple Silicon
# Fixes LLVM codegen crashes and compiler panics

set -e

echo "🍎 Building on macOS Apple Silicon..."

# Increase Rust stack size to prevent LLVM crashes
export RUST_MIN_STACK=16777216

# Reduce parallel jobs to avoid memory pressure
export CARGO_BUILD_JOBS=4

# Clear any corrupted cache
echo "🧹 Cleaning build cache..."
cargo clean

# Build with optimized settings
echo "🔨 Building project..."
cargo build "$@"

echo "✅ Build completed successfully!"
