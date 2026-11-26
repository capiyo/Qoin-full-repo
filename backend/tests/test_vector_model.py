import json
import unittest
from unittest.mock import patch

from research.vector_model import evaluate_signal


class FakeResponse:
    def __init__(self, value):
        self.value = value

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return False

    def read(self):
        return json.dumps(self.value).encode("utf-8")


class RustVectorBridgeTests(unittest.TestCase):
    def setUp(self):
        self.observation = {
            "ofi": 0.4,
            "liquidity": -0.1,
            "volatility": 0.2,
            "funding": -0.5,
            "open_interest_change": 0.1,
            "liquidation_pressure": 0.0,
            "basis": 0.0,
            "spread_bps": 3.0,
            "spread_age_ms": 100,
        }

    @patch("research.vector_model.urlopen")
    def test_delegates_decision_to_authoritative_rust_endpoint(self, urlopen):
        signal = {"action": "buy", "config_version": "baseline-v2", "attention": [[1.0]]}
        urlopen.return_value = FakeResponse(signal)

        result = evaluate_signal(
            self.observation,
            has_open_position=False,
            base_url="http://127.0.0.1:8081/",
        )

        self.assertEqual(result, signal)
        request = urlopen.call_args.args[0]
        self.assertEqual(request.full_url, "http://127.0.0.1:8081/v1/signal")
        body = json.loads(request.data)
        self.assertEqual(body["observation"]["ofi"], 0.4)
        self.assertEqual(body["observation"]["spread_age_ms"], 100)
        self.assertFalse(body["has_open_position"])

    @patch("research.vector_model.urlopen")
    def test_missing_spread_is_passed_to_rust_as_null(self, urlopen):
        urlopen.return_value = FakeResponse({"action": "wait", "config_version": "baseline-v2"})
        observation = {**self.observation, "spread_bps": None, "spread_age_ms": None}

        result = evaluate_signal(observation)

        self.assertEqual(result["action"], "wait")
        body = json.loads(urlopen.call_args.args[0].data)
        self.assertIsNone(body["observation"]["spread_bps"])
        self.assertIsNone(body["observation"]["spread_age_ms"])

    @patch("research.vector_model.urlopen")
    def test_invalid_numeric_input_is_rejected_before_http_request(self, urlopen):
        observation = {**self.observation, "ofi": float("nan")}
        with self.assertRaises(ValueError):
            evaluate_signal(observation)
        urlopen.assert_not_called()

    @patch("research.vector_model.urlopen")
    def test_invalid_rust_response_is_rejected(self, urlopen):
        urlopen.return_value = FakeResponse({"action": "buy"})
        with self.assertRaises(RuntimeError):
            evaluate_signal(self.observation)


if __name__ == "__main__":
    unittest.main()
