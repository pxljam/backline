# syntax=docker/dockerfile:1
# API Rust + export PDF Typst. Un seul binaire par role (§15).

FROM rust:1.90-bookworm AS build
WORKDIR /src
COPY api/Cargo.toml api/Cargo.lock ./
COPY api/src src
COPY api/migrations migrations
# Les caches BuildKit evitent de recompiler tout l'arbre a chaque changement.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --bins && \
    mkdir -p /out && \
    cp target/release/server target/release/bot target/release/migrate \
       target/release/seed target/release/healthcheck /out/

FROM debian:bookworm-slim
ARG TYPST_VERSION=0.15.1
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libssl3 curl xz-utils && \
    arch="$(uname -m)" && \
    curl -fsSL "https://github.com/typst/typst/releases/download/v${TYPST_VERSION}/typst-${arch}-unknown-linux-musl.tar.xz" \
      | tar -xJ -C /tmp && \
    mv /tmp/typst-*/typst /usr/local/bin/typst && \
    rm -rf /tmp/typst-* /var/lib/apt/lists/* && \
    apt-get purge -y curl xz-utils && apt-get autoremove -y

WORKDIR /app
# Les modeles Typst voyagent avec l'image : un PDF ne depend d'aucun reseau.
COPY pdf /app/pdf
COPY --from=build /out/ /usr/local/bin/

ENV TYPST_TEMPLATE_DIR=/app/pdf \
    BIND_ADDR=0.0.0.0:8080
EXPOSE 8080
CMD ["/usr/local/bin/server"]
