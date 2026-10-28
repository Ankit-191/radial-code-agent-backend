FROM rust:latest as builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev cmake build-essential && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/radial-code-agent /app/radial-code-agent
COPY data /app/data

EXPOSE 3001
ENV PORT=3001
CMD ["/app/radial-code-agent"]
