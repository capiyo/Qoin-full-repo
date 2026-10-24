import unittest
from research.replay import ReplayConfig, ReplayEvent, baseline_strategies, deflated_sharpe_ratio, run_replay, walk_forward_splits

def event(ts, seq, price, age=0, regime=True, fill=1.0, kind="trade", funding=0.0):
    return ReplayEvent(ts, ts + age, "BTCUSDT", kind, price, price - 0.01, price + 0.01,
                       0.9, funding, 0.001, seq, regime, False, fill)

class ReplayTests(unittest.TestCase):
    def test_no_trade_baseline(self):
        result = run_replay([event(1000, 1, 100), event(2000, 2, 101)], baseline_strategies()["no_trade"])
        self.assertEqual(result.trade_count, 0)
        self.assertEqual(result.final_equity, result.initial_equity)

    def test_arrival_order_is_required(self):
        with self.assertRaises(ValueError):
            run_replay([event(2000, 2, 100), event(1000, 1, 101)], baseline_strategies()["ofi_only"])

    def test_sequence_gap_blocks_entries(self):
        items = [event(1000, 1, 100, regime=False), event(2000, 3, 101), event(3000, 4, 102)]
        result = run_replay(items, baseline_strategies()["ofi_only"])
        self.assertEqual(result.trade_count, 0)
        self.assertGreaterEqual(result.sequence_rejections, 1)

    def test_stale_event_is_rejected(self):
        result = run_replay([event(1000, 1, 100, age=5001)], baseline_strategies()["ofi_only"])
        self.assertEqual(result.trade_count, 0)
        self.assertEqual(result.stale_events, 1)

    def test_cost_stress_reduces_equity(self):
        items = [event(1000, 1, 100), event(2000, 2, 101)]
        strategy = lambda item: "buy" if item.event_ts_ms == 1000 else "close"
        normal = run_replay(items, strategy, ReplayConfig(total_cost_stress_multiplier=1.0))
        stressed = run_replay(items, strategy, ReplayConfig(total_cost_stress_multiplier=1.5))
        self.assertLess(stressed.final_equity, normal.final_equity)

    def test_partial_fill_and_funding(self):
        items = [event(1000, 1, 100, fill=0.5), event(2000, 1, 100, kind="funding", funding=0.01)]
        result = run_replay(items, lambda item: "buy")
        self.assertLess(result.funding_pnl, 0.0)

    def test_deflated_sharpe_needs_enough_returns(self):
        self.assertIsNone(deflated_sharpe_ratio([0.1, 0.2]))
        self.assertIsNotNone(deflated_sharpe_ratio([0.1, -0.02, 0.05, 0.04], trials=1))

    def test_walk_forward_gap(self):
        splits = walk_forward_splits(100, 30, 10, purge=3, embargo=2)
        self.assertTrue(splits)
        for train, test in splits:
            self.assertGreaterEqual(test.start - train.stop, 5)

if __name__ == "__main__":
    unittest.main()
