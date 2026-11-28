"""Run one immutable, deterministic Qoin replay from an input JSONL dataset."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from research.replay import ReplayConfig, ReplayEvent, baseline_strategies, record_experiment, run_replay


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, help="Chronological JSONL market events with event_ts_ms and arrival_ts_ms")
    parser.add_argument("--experiment-id", required=True)
    parser.add_argument("--strategy", choices=("no_trade", "ofi_only", "linear_baseline"), default="ofi_only")
    parser.add_argument("--cost-stress", type=float, default=1.0, help="Use 1.5 for +50% cost stress")
    parser.add_argument("--volatile-slippage", type=float, default=4.0, help="Stress multiplier between 3 and 5 for volatile periods")
    parser.add_argument("--fee-bps", type=float, default=4.0)
    parser.add_argument("--slippage-bps", type=float, default=2.0)
    parser.add_argument("--log", default="backend/research/experiment_runs.jsonl")
    args = parser.parse_args()

    source = Path(args.input)
    raw = source.read_bytes()
    events = [ReplayEvent(**json.loads(line)) for line in raw.decode("utf-8").splitlines() if line.strip()]
    if not events:
        parser.error("input dataset contains no events")

    config = ReplayConfig(
        fee_bps=args.fee_bps,
        base_slippage_bps=args.slippage_bps,
        volatile_slippage_multiplier=args.volatile_slippage,
        total_cost_stress_multiplier=args.cost_stress,
    )
    result = run_replay(events, baseline_strategies()[args.strategy], config)
    metadata = {
        "experiment_id": args.experiment_id,
        "dataset_path": str(source),
        "dataset_sha256": hashlib.sha256(raw).hexdigest(),
        "strategy": args.strategy,
        "configuration": vars(args),
        "event_count": len(events),
        "engine": "research.replay",
    }
    record_experiment(metadata, result, args.log)
    print(json.dumps({
        "experiment_id": args.experiment_id,
        "total_return": result.total_return,
        "max_drawdown": result.max_drawdown,
        "trade_count": result.trade_count,
        "wins": result.wins,
        "losses": result.losses,
        "profit_factor": result.profit_factor if result.profit_factor != float("inf") else None,
        "fees_paid": result.fees_paid,
        "spread_cost": result.spread_cost,
        "slippage_cost": result.slippage_cost,
        "funding_pnl": result.funding_pnl,
        "rejected_orders": result.rejected_orders,
        "stale_events": result.stale_events,
        "sequence_rejections": result.sequence_rejections,
        "risk_vetoes": result.risk_vetoes,
        "dataset_sha256": metadata["dataset_sha256"],
        "log_path": args.log,
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
