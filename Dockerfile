FROM rust:latest AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev cmake build-essential && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
# Build with controlled concurrency to prevent OOM on Render free tier (512MB RAM)
RUN cargo build --release --jobs 2

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/radial-code-agent /app/radial-code-agent
COPY data /app/data

# Render uses port 10000 by default and dynamically overrides PORT at runtime
ENV PORT=10000
EXPOSE 10000

CMD ["/app/radial-code-agent"]
