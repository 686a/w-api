# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS builder

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && printf 'fn main() {}\n' > src/main.rs \
    && cargo fetch --locked \
    && rm -rf src

COPY src ./src

RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && useradd --create-home --uid 10001 appuser \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/w-api /usr/local/bin/w-api

USER appuser

EXPOSE 8080

CMD ["/usr/local/bin/w-api"]
