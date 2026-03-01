# --- Build Stage ---
FROM rust:1.85-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev gcc g++ make pkgconf

WORKDIR /usr/src/app

# Copy Cargo.toml and Cargo.lock first for caching
COPY Cargo.toml ./
# Create dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

# Copy actual source
COPY src ./src
# Force cargo to detect source changes (COPY preserves original timestamps
# which may be older than the dummy binary, causing cargo to skip rebuild)
RUN touch src/main.rs && rm -f target/release/media-dashboard

# Build the real binary
RUN cargo build --release

# Verify it's statically linked
RUN ldd target/release/media-dashboard 2>&1 || true

# --- Runtime Stage ---
FROM alpine:3.21

# Install runtime dependencies
RUN apk add --no-cache ca-certificates tzdata su-exec shadow

# Create a non-root user and group
RUN addgroup -S dashboard && adduser -S dashboard -G dashboard

# Create data directory for SQLite
RUN mkdir -p /app/data && chown -R dashboard:dashboard /app && chmod 750 /app/data

WORKDIR /app

# Copy binary from builder
COPY --from=builder /usr/src/app/target/release/media-dashboard /app/
RUN chown dashboard:dashboard /app/media-dashboard && chmod +x /app/media-dashboard

# Copy static assets
COPY static /app/static
RUN chown -R dashboard:dashboard /app/static

# Copy entrypoint script
COPY entrypoint.sh /app/
RUN chmod +x /app/entrypoint.sh

# Note: config.json is no longer required in the image as settings are now database-driven.

# We do NOT use the "USER" directive here because the entrypoint script needs to run as root
# to fix the mounted volume permissions before dropping privileges via su-exec.

# Expose port
EXPOSE 7778

ENTRYPOINT ["/app/entrypoint.sh"]
CMD ["./media-dashboard"]
