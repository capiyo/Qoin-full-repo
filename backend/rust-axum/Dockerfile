FROM rust:1.88-bookworm AS build
WORKDIR /workspace

# Cargo resolves every workspace member even when building one package.
# Copy all manifests and source trees declared by the root workspace.
COPY Cargo.toml ./Cargo.toml
COPY backend/rust-axum ./backend/rust-axum
COPY backend/rust-axum-api ./backend/rust-axum-api
COPY backend/rust-axum/crates/qoin-vector-engine ./backend/rust-axum/crates/qoin-vector-engine
COPY backend/rust-axum/crates/qoin-vector-motion ./backend/rust-axum/crates/qoin-vector-motion
COPY backend/rust-axum/crates/qoin-domain-core ./backend/rust-axum/crates/qoin-domain-core

RUN cargo build --release --manifest-path backend/rust-axum/Cargo.toml

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home qoin
COPY --from=build /workspace/target/release/qoin-flow-engine /usr/local/bin/qoin-flow-engine
USER 10001:10001
EXPOSE 8081
ENTRYPOINT ["/usr/local/bin/qoin-flow-engine"]
