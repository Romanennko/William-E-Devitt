FROM rust:1.75-slim AS builder

WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations

RUN apt-get update && apt-get install -y pkg-config libssl-dev sqlite3

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libssl-dev sqlite3 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /usr/src/app/target/release/William /usr/local/bin/william
COPY --from=builder /usr/src/app/migrations /app/migrations

RUN mkdir -p /app/data

ENV RUST_LOG=info
ENV DATABASE_URL=sqlite:/app/data/william.db

CMD ["william"]
