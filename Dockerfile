# =============================================================================
# Stage 1: Build the Redis module
# =============================================================================
FROM rust:1.88-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    clang \
    build-essential \
    file \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /build

# Copy manifests
COPY Cargo.toml ./

# Copy source code
COPY src ./src

# Build the module in release mode
RUN cargo build --release

# Verify the module was built
RUN ls -lah /build/target/release/ && \
    file /build/target/release/libredis_ttl_bloom.so

# =============================================================================
# Stage 2: Create the final Redis image with the module
# =============================================================================
FROM redis:7.2-alpine

# Install runtime dependencies (minimal)
RUN apk add --no-cache libgcc

# Create directory for the module
RUN mkdir -p /usr/lib/redis/modules

# Copy the compiled module from builder stage
COPY --from=builder /build/target/release/libredis_ttl_bloom.so /usr/lib/redis/modules/

# Copy custom Redis configuration
COPY redis.conf /usr/local/etc/redis/redis.conf

# Verify module file exists and has correct permissions
RUN ls -lah /usr/lib/redis/modules/libredis_ttl_bloom.so && \
    chmod 755 /usr/lib/redis/modules/libredis_ttl_bloom.so

# Expose Redis port
EXPOSE 6379

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD redis-cli ping || exit 1

# Start Redis with custom config that loads the module
CMD ["redis-server", "/usr/local/etc/redis/redis.conf"]
