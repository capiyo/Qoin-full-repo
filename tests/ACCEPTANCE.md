# Acceptance gates

- Python reference tests pass.
- Rust `cargo test` and `cargo clippy --all-targets -- -D warnings` pass on a pinned toolchain.
- Drogon CMake configure/build passes on a supported host.
- MongoDB integration tests cover event idempotency, persistence failures, restart replay, and concurrent events.
- The same signal implementation is used in live/paper/backtest modes.
- Backtest includes fees, spread, slippage, funding, +50% cost stress, walk-forward splits, purge/embargo, bootstrap confidence intervals, no-trade and simple baselines.
- Paper/testnet and shadow mode pass before any live-capital review.
- Live order submission remains disabled until a human explicitly approves it.
