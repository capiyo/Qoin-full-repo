# State of repository

Audit date: 2026-11-28  
Authoritative branch: `master`

## Architecture

Qoin is a multi-client, multi-asset research and decision platform. Finance is the first implemented vertical. The current architecture is:

- Rust/Axum owns market-event validation, state reconstruction, vector decisions, set-ledger operations, research APIs, identity enforcement for personalized/private routes, and broker execution contracts.
- C++/Drogon is the public HTTP/WebSocket gateway. It proxies approved public-facing API operations; it is not a second decision engine.
- MongoDB persists raw observations, derived state, decisions, ledgers, user-scoped records, order intents, and operator controls.
- React web, Expo/React Native mobile, and Tauri desktop clients share TypeScript contracts through `@qoin/core`.
- Python contains reference/research tooling and tests; it is not an authoritative runtime decision service.

## Implemented source areas on master

- Deterministic Version 1 vector engine with causal event processing, feature calculations, decision diagnostics, and research signal output.
- Version 2 set-theoretic attention and relationship-ledger implementation, including set/measure and temporal relation operations.
- Shared market-vector-space contracts and attribution primitives used to express vector effects and set-measure contributions without presenting them as causal probabilities.
- Multi-horizon research evaluation with independent horizon inputs and explicit separation from order authority.
- Historical event replay/backtesting with venue isolation, deterministic event ordering, execution-cost assumptions, and documented simulation limitations.
- Market data ingestion and provider status for the configured public feeds. Provider coverage, credentials, entitlement and freshness remain provider-specific.
- Firebase Authentication integration, server-side Firebase ID-token verification, UID-scoped profile and broker-account operations, and Firestore/Storage rules for client-owned data.
- Server-side broker environment loading. Broker secrets are deployment environment values and are not accepted from the browser or serialized into API responses.
- Broker-neutral order, account, instrument, capability, risk-preflight, receipt, execution-lifecycle, allocation and reconciliation contracts.
- Broker catalogue and adapter scaffolding. The catalogue is not proof that a provider is executable.
- Web, mobile and desktop application shells, Archive/Explore, instrument mechanics, vector inspection, market notifications, and shared API contracts.
- CI workflows for Rust, gateway, web, mobile, desktop, and Axum–Drogon–MongoDB integration.

## Execution boundary and current safety state

The private Axum order and reconciliation routes exist in the source tree. They are not public Drogon routes. The order handler requires the execution token, a verified Firebase identity, an active UID-owned broker-account mapping, backend authorization for the selected account, a healthy operator trading control, durable order-intent persistence, and the shared risk-checked broker contract.

The runtime initializes the broker registry empty. The named provider adapter scaffolds deliberately return unsupported results and are not registered as executable adapters. Therefore the presence of an order route or broker catalogue must not be represented as a working broker integration. No provider is currently verified as execution-ready, and order dispatch must remain unavailable/fail-closed until a real adapter is implemented, registered through an approved operator path, and passes provider-specific conformance and sandbox tests.

Qoin is a research and paper-trading platform. Do not enable real-money execution or describe it as production-ready.

## Retired components

JEPA is retired. The active master tree does not contain the former Axum JEPA module or Python JEPA sidecar. Do not restore JEPA from historical branches. Version 2 means set-theoretic attention and the relationship/evidence ledger.

## Verification and operational limitations

The repository contains CI workflows and historical green CI records, but those records do not prove that the current `master` head is green. Verify every required workflow against the exact commit being released. No local build or runtime test is claimed by this document update.

Remaining release gates include:

- Real provider-specific market-data coverage, history/backfill quality, symbol/venue mapping, freshness monitoring, and provider failover.
- Independent chronological and out-of-sample research evaluation, robust portfolio accounting, cross-asset exposure controls, and reproducible datasets.
- Concrete broker adapters, account permission validation, provider conformance/sandbox testing, credential rotation, and operator provisioning.
- Durable reconciliation automation, partial-failure recovery, cancellation/amendment handling, and independently reviewed portfolio-level execution risk.
- Firebase revocation/authorization operations, payment and entitlement reconciliation where applicable, and security review of public ingress.
- Production hosting, secret management, TLS/DNS, monitoring, backups, disaster recovery, and desktop release verification.

## Branch policy

`master` is the sole authoritative development, integration, verification and release branch. Historical branches are read-only reference material. See [BRANCH_POLICY.md](BRANCH_POLICY.md).
