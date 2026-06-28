# ── Stage 1: dependency planner ──────────────────────────────────────────────
FROM lukemathwalker/cargo-chef:latest-rust-1.88-slim-bookworm AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ── Stage 2: dependency compiler ─────────────────────────────────────────────
FROM lukemathwalker/cargo-chef:latest-rust-1.88-slim-bookworm AS builder
WORKDIR /app

# cmake + build-essential needed for vendored libgit2
RUN apt-get update && apt-get install -y cmake pkg-config build-essential \
    && rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN cargo build --release -p bastion-web -p bastion-ingest

# ── Stage 3: runtime image ────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# pdftotext (poppler-utils) + ca-certificates for HTTPS LLM calls
RUN apt-get update && apt-get install -y poppler-utils ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/bastion-web   /usr/local/bin/
COPY --from=builder /app/target/release/bastion-ingest /usr/local/bin/

RUN mkdir -p /data/wiki /data/raw /data/db

ENV BASTION_WIKI_PATH=/data/wiki
ENV BASTION_RAW_PATH=/data/raw
ENV BASTION_DB_PATH=/data/db/bastion.sqlite
ENV BASTION_BACKEND_PORT=8080

EXPOSE 8080
CMD ["bastion-web"]
