# Qoin engineering backlog

The backlog contains only work explicitly marked **BLOCKED** in [docs/READINESS.md](docs/READINESS.md).

- [ ] **BLOCKED — PR integration:** reconcile PRs #12, #13 and #15 after the #11 integration; requires a repository checkout/terminal for conflict resolution.
- [ ] **BLOCKED — Verification baseline:** run the complete Rust, C++, web, React Native, Python, Tauri, Compose and integration matrix on the exact candidate commit.
- [ ] **BLOCKED — Drogon stability:** reproduce and root-cause `std::length_error`, add a regression test, then run at least 20 consecutive integration cycles.
- [ ] **BLOCKED — Research data and portfolio evaluation:** historical backfill, provenance/completeness, provider failover, multi-currency accounting, walk-forward/OOS benchmarks and independent algorithm runtime.
- [ ] **BLOCKED — User platform:** Firebase ID-token verification, UID-derived entitlements, durable algorithm selections, cross-device Archive/notification sync and full client E2E.
- [ ] **BLOCKED — Execution safety:** portfolio risk integration, verified account ownership, durable lifecycle, automatic reconciliation, exposure-managing kill switch, account/position sync and Binance testnet E2E.
- [ ] **BLOCKED — Release acceptance:** adversarial security/failure tests, Compose recovery, full CI and operational readiness review.

No other backlog items are open. Do not enable real-money execution, merge to `main`, or deploy to production.
