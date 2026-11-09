# Vector engine and MongoDB addendum

## Collections
- `market_events`: validated source events, deduplicated by `(venue, symbol, event_id)`.
- `market_states`: append-only state snapshots keyed by venue and symbol; the latest snapshot per pair is restored at service startup.
- `trade_decisions`: append-only strategy decisions including venue, features, attention, vector magnitude/angles, action, reason, and strategy version.
- `edge_strategy`: active versioned strategy map loaded from MongoDB.
- `backtest_runs` and `backtest_decisions`: historical replay summaries and per-event Mirror traces.
- `operating_cycles`: persistent observation / diagnostic / paper-session cycle state.
- `paper_sessions`: active forward paper account ledgers and open/pending positions.
- `paper_session_runs`: archived final account snapshots for completed paper cycles.
- `paper_session_decisions`: append-only forward paper Mirror records.
- `paper_session_trades`: closed simulated paper trades and net outcomes.
- `trade_orders`: reserved for a future broker order lifecycle; no live order endpoint is registered.

## Current implementation
Live state is isolated by `(venue, symbol)`. The latest persisted state for each pair is restored on startup. An event older than the current event-time watermark is retained in `market_events` for chronological historical replay but is not applied to the forward state or paper simulator.

Historical backtesting is event-driven and uses the same active `EdgeStrategy::decide` implementation as ingestion. It replays in event-time order, requires a valid observed two-sided quote for simulated fills, applies configured fee/slippage assumptions, supports compounding, and writes its run summary and Mirror decisions to MongoDB.

The operating cycle supports up to 50 explicitly configured venue/symbol paper accounts. The five-minute observation counter is populated only in the observation phase; feature, attention, score and bullish/bearish weight-contribution diagnostics are populated only in the following one-hour evaluation phase. Strategy updates are rejected while a cycle is active, so the evaluated coefficients remain fixed. Neither phase creates fills. During the paper-session phase, incoming events for configured accounts are passed through the active strategy and an incremental long-only paper simulator. Each account has a separate quote-currency ledger; no FX conversion or shared-wallet allocation is inferred. Account state, decision traces and closed trade outcomes are persisted and exposed through read-only paper-session endpoints. Live broker order submission remains disconnected.

## Remaining engineering caveats
Mongo writes across raw event, state snapshot, research decision, paper Mirror, trade record and paper account state are not a single multi-document transaction. The API returns partial-pipeline status on failures, but a durable reconciliation worker is still needed to recover every partially completed event automatically. Strategy configuration is versioned by strategy name/version, but every run should also persist a canonical config hash and historical strategy snapshots before walk-forward research can be considered reproducible. The forward simulator is a conservative baseline, not a broker-specific fill model.

## Next implementation steps
1. Add an idempotent event-processing journal and recovery worker to reconcile partial Mongo write failures by event ID.
2. Add a canonical strategy/config hash and retain immutable strategy snapshots for each run and session.
3. Add past-only normalization, bounded causal windows and explicit stale-data / feed-quality abstention gates.
4. Extend paper simulation with independently tested risk controls, stop/target policies, funding/financing, contract multipliers, and broker-specific cost models.
5. Add chronological walk-forward and regime-split evaluation with untouched holdout periods.
6. Add an independent risk gate, order lifecycle, exchange reconciliation, protective exchange-side stops, circuit breakers and crash recovery before any testnet integration.

The current system remains research and paper-only. None of the remaining controls should be assumed implemented, and no live trading should be enabled until a separate risk and operational review passes.
