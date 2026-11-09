# Engineering decisions

These decisions apply to the Qoin integration candidate unless superseded by an explicit repository decision.

1. **Architecture is fixed.** Rust/Axum remains authoritative for vector calculation, decisions, risk and state. C++/Drogon remains an HTTP/WebSocket gateway only. MongoDB is persistence. React web and React Native share `@qoin/core`. Python is for research/backtesting/analysis. Tauri 2 is the desktop shell and the vector-motion solver remains Rust.
2. **One deterministic decision authority.** The active engine is the deterministic Rust vector engine. No second model service or client-side decision implementation may affect decisions.
3. **Multi-market support stays active.** The current multi-asset registry and provider adapters are retained. Capability differences must be explicit; unavailable book/funding/liquidation/OI fields must never be synthesized.
4. **Real-money execution stays disabled in code.** Environment variables, credentials and the operator gate alone cannot enable live execution. Paper and exchange testnet are the only allowed modes until every release gate is independently verified.
5. **No production merge or deployment.** Work is isolated on `qoin/integration-completion`; `main` and production are not changed.
6. **PR integration order is #11, #12, #13, #15.** The PRs were found to share a common base rather than form a linear ancestry. PR #11 was merged into the isolated `qoin/integration-chain` branch. Retargeting PR #12 to that branch exposed merge conflicts, so the remaining PRs must be reconciled in a real checked-out workspace rather than force-merging divergent snapshots.
7. **Evidence is commit-specific.** A green workflow on an older SHA is historical evidence only. A release claim requires all relevant jobs to pass on the exact final PR head.
8. **Unknown failures remain open.** A retry that passes does not resolve the intermittent Drogon `std::length_error`; root cause and repeatability evidence are required before closure.
