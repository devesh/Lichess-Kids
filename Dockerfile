# Stage 1: Build the Rust binary
FROM rust:1.79-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/lichesskids

# Copy manifest files
COPY Cargo.toml Cargo.lock ./

# Copy source code and library exports
COPY src ./src

# Build for release
RUN cargo build --release

# Stage 2: Create final slim runtime container
FROM debian:bookworm-slim

# Install SSL certificates and SQLite runtime dependency
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy release binary from builder
COPY --from=builder /usr/src/lichesskids/target/release/lichesskids /usr/local/bin/lichesskids

# Copy static frontend assets
COPY static ./static

# Set runtime environment defaults
ENV PORT=3000
ENV DATABASE_URL=/app/lichesskids.db

EXPOSE 3000

# Run the server
CMD ["lichesskids"]
