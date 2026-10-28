FROM rust:1.80-slim as builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/radial-code-agent /app/radial-code-agent
COPY data /app/data

EXPOSE 3001
ENV PORT=3001
CMD ["/app/radial-code-agent"]
