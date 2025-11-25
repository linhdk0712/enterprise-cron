# Multi-stage Dockerfile for Vietnam Enterprise Cron System
# Target: < 50MB final image size
# Requirements: 9.1

# Stage 1: Build stage with Rust compiler
# Using Rust nightly for edition 2024 support
FROM rustlang/rust:nightly-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy dependency manifests first for better caching
COPY Cargo.toml Cargo.lock ./
COPY api/Cargo.toml ./api/
COPY common/Cargo.toml ./common/
COPY scheduler/Cargo.toml ./scheduler/
COPY worker/Cargo.toml ./worker/
COPY integration-tests/Cargo.toml ./integration-tests/

# Create dummy source files to build dependencies
RUN mkdir -p api/src common/src scheduler/src worker/src integration-tests/tests && \
    echo "fn main() {}" > api/src/main.rs && \
    echo "fn main() {}" > scheduler/src/main.rs && \
    echo "fn main() {}" > worker/src/main.rs && \
    echo "pub fn dummy() {}" > common/src/lib.rs && \
    echo "#[test] fn dummy() {}" > integration-tests/tests/dummy.rs

# Build dependencies only (this layer will be cached)
RUN cargo build --release --workspace

# Remove dummy source files and cached builds
RUN rm -rf api/src common/src scheduler/src worker/src integration-tests/tests
RUN rm -rf target/release/.fingerprint/common-* target/release/.fingerprint/api-* target/release/.fingerprint/scheduler-* target/release/.fingerprint/worker-*
RUN rm -rf target/release/deps/libcommon-* target/release/deps/api-* target/release/deps/scheduler-* target/release/deps/worker-*

# Copy actual source code
COPY api ./api
COPY common ./common
COPY scheduler ./scheduler
COPY worker ./worker
COPY integration-tests ./integration-tests
COPY migrations ./migrations
COPY config ./config
COPY .sqlx ./.sqlx

# Build the actual binaries with optimizations
# Use --locked to ensure Cargo.lock is respected
# Increase stack size to avoid compiler crashes
# Use offline mode for sqlx (requires .sqlx cache files)
ENV RUST_MIN_STACK=16777216
ENV SQLX_OFFLINE=true
RUN cargo build --release --locked --workspace

# Strip symbols to reduce binary size
RUN strip /app/target/release/api && \
    strip /app/target/release/scheduler && \
    strip /app/target/release/worker

# Stage 2: Minimal runtime image
# Using debian:trixie-slim for GLIBC 2.38+ support
FROM debian:trixie-slim

# Install only runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    tzdata \
    libssl3 \
    wget \
    && rm -rf /var/lib/apt/lists/* && \
    # Create non-root user
    groupadd -g 1000 cronuser && \
    useradd -u 1000 -g cronuser -s /bin/bash -m cronuser && \
    # Create necessary directories
    mkdir -p /app/config /app/migrations && \
    chown -R cronuser:cronuser /app

# Copy MinIO certificate and add to trust store (for development with self-signed certs)
COPY config/certs/minio/public.crt /usr/local/share/ca-certificates/minio.crt
RUN update-ca-certificates

# Copy binaries from builder
COPY --from=builder --chown=cronuser:cronuser /app/target/release/api /usr/local/bin/api
COPY --from=builder --chown=cronuser:cronuser /app/target/release/scheduler /usr/local/bin/scheduler
COPY --from=builder --chown=cronuser:cronuser /app/target/release/worker /usr/local/bin/worker

# Copy configuration and migrations
COPY --chown=cronuser:cronuser config/default.toml /app/config/
COPY --chown=cronuser:cronuser migrations /app/migrations/

# Copy templates for dashboard
COPY --chown=cronuser:cronuser api/templates /app/api/templates/

# Set working directory
WORKDIR /app

# Switch to non-root user
USER cronuser

# Set environment variables
ENV RUST_LOG=info
ENV APP_CONFIG_PATH=/app/config/default.toml

# Expose API port (default 8080)
EXPOSE 8080

# Expose metrics port (default 9090)
EXPOSE 9090

# Default command runs the API server
# Override with scheduler or worker for those components
CMD ["api"]

# Health check for API server
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1

# Labels for metadata
LABEL maintainer="Vietnam Enterprise Cron Team"
LABEL description="Distributed job scheduling and execution platform"
LABEL version="1.0.0"
LABEL org.opencontainers.image.source="https://github.com/vietnam-enterprise/cron-system"
