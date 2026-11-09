FROM rust:1.88-bookworm AS build
WORKDIR /workspace
COPY Cargo.toml ./
COPY backend/rust-axum ./backend/rust-axum
COPY backend/rust-axum-api ./backend/rust-axum-api
RUN cargo build --release --manifest-path backend/rust-axum/Cargo.toml

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home qoin
COPY --from=build /workspace/target/release/qoin-flow-engine /usr/local/bin/qoin-flow-engine
USER 10001:10001
EXPOSE 8081
ENTRYPOINT ["/usr/local/bin/qoin-flow-engine"]
