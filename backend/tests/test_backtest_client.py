import json
import unittest
from urllib.parse import parse_qs, urlparse
from unittest.mock import patch

from research.backtest_client import run_rust_backtest


class FakeResponse:
    def __init__(self, value):
        self.value = value

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return False

    def read(self):
        return json.dumps(self.value).encode("utf-8")


class RustBacktestClientTests(unittest.TestCase):
    @patch("research.backtest_client.urlopen")
    def test_delegates_full_replay_to_rust_with_execution_assumptions(self, urlopen):
        expected = {"symbol": "BTCUSDT", "fills": [], "assumptions": {"fee_bps": 10.0}}
        urlopen.return_value = FakeResponse(expected)

        result = run_rust_backtest(
            symbol="BTCUSDT",
            venue="binance-usdm",
            from_ts_ms=1000,
            to_ts_ms=2000,
            limit=500,
            initial_capital=5000.0,
            fee_bps=8.0,
            slippage_bps=4.0,
            latency_ms=20,
            max_participation_rate=0.2,
            market_impact_bps=6.0,
            base_url="http://127.0.0.1:8081/",
        )

        self.assertEqual(result, expected)
        request = urlopen.call_args.args[0]
        self.assertEqual(request.full_url.split("?", 1)[0], "http://127.0.0.1:8081/internal/backtest")
        params = parse_qs(urlparse(request.full_url).query)
        self.assertEqual(params["symbol"], ["BTCUSDT"])
        self.assertEqual(params["venue"], ["binance-usdm"])
        self.assertEqual(params["latency_ms"], ["20"])
        self.assertEqual(params["max_participation_rate"], ["0.2"])

    @patch("research.backtest_client.urlopen")
    def test_invalid_replay_window_is_rejected_before_http(self, urlopen):
        with self.assertRaises(ValueError):
            run_rust_backtest(symbol="BTCUSDT", from_ts_ms=2000, to_ts_ms=1000)
        urlopen.assert_not_called()

    @patch("research.backtest_client.urlopen")
    def test_invalid_cost_assumptions_are_rejected(self, urlopen):
        with self.assertRaises(ValueError):
            run_rust_backtest(symbol="BTCUSDT", to_ts_ms=2000, slippage_bps=10000)
        urlopen.assert_not_called()

    @patch("research.backtest_client.urlopen")
    def test_rejects_malformed_rust_response(self, urlopen):
        urlopen.return_value = FakeResponse({"symbol": "ETHUSDT"})
        with self.assertRaises(RuntimeError):
            run_rust_backtest(symbol="BTCUSDT", to_ts_ms=2000)


if __name__ == "__main__":
    unittest.main()
