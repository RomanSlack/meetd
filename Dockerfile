# Build stage
FROM rust:1.85-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconf

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock* ./

# Create dummy source to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies only (this layer will be cached)
RUN cargo build --release && rm -rf src

# Copy actual source
COPY src ./src

# Touch main.rs to trigger rebuild of our code
RUN touch src/main.rs

# Build the application
RUN cargo build --release

# Runtime stage
FROM alpine:3.19

# Install runtime dependencies
RUN apk add --no-cache ca-certificates

# Copy the binary
COPY --from=builder /app/target/release/meetd /usr/local/bin/meetd

# Create non-root user
RUN adduser -D -u 1000 meetd
USER meetd

# Create data directory
RUN mkdir -p /home/meetd/data
WORKDIR /home/meetd

# Expose default port
EXPOSE 8080

# Default command
CMD ["meetd", "serve", "--port", "8080", "--db", "/data/meetd.db", "--url", "https://meetd.fly.dev"]
