import json
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from research.replay import ReplayEvent, baseline_strategies, record_experiment, run_replay

class ReplayAuditTests(unittest.TestCase):
    def test_missing_edge_estimate_vetoes_entry(self):
        item = ReplayEvent(1000, 1000, "BTCUSDT", "trade", 100, 99.99, 100.01,
                           0.9, 0.0, 0.001, 1, True, False, 1.0, expected_edge_bps=None)
        result = run_replay([item], lambda current: "buy")
        self.assertEqual(result.trade_count, 0)
        self.assertEqual(result.risk_vetoes, 1)

    def test_experiment_results_append_to_jsonl(self):
        item = ReplayEvent(1000, 1000, "BTCUSDT", "trade", 100, 99.99, 100.01,
                           0.9, 0.0, 0.001, 1, True, False, 1.0, expected_edge_bps=100.0)
        result = run_replay([item], baseline_strategies()["no_trade"])
        with tempfile.TemporaryDirectory() as folder:
            destination = Path(folder) / "experiments.jsonl"
            record_experiment({"experiment_id": "unit-test"}, result, str(destination))
            payload = json.loads(destination.read_text(encoding="utf-8"))
            self.assertEqual(payload["metadata"]["experiment_id"], "unit-test")
            self.assertEqual(payload["metrics"]["trade_count"], 0)

if __name__ == "__main__":
    unittest.main()
