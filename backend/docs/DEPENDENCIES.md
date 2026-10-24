# Dependency inventory

## Runtime components
- Rust 2021, Axum 0.7, Tokio 1, Serde 1, MongoDB Rust driver 2.8.2, Reqwest 0.12, HMAC 0.12, SHA-256 0.10.
- C++17 and Drogon, discovered through `find_package(Drogon CONFIG REQUIRED)`.
- MongoDB 7 for local development.
- Python 3.10+ for reference tests (uses modern union type annotations).

## Provenance and licensing
Use upstream package registries and official project repositories to confirm current licenses before production deployment. The snapshot does not include a Rust lockfile, C++ dependency lock, SBOM, or pinned compiler/toolchain versions. Add `Cargo.lock`, pin CI toolchains, and produce an SBOM before release. No third-party research code should be copied into the runtime without license and security review.

## Environment validation status
At the time of audit, Python tests ran, but Cargo/Rust and Drogon were unavailable. Native builds and dependency resolution remain unverified.
