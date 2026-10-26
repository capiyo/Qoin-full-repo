"""Loopback-only HTTP wrapper for JEPA research inference.

This endpoint is intended for local development/shadow experiments. It is not
an authenticated public API and must not be exposed beyond loopback.
"""
from __future__ import annotations

import argparse
import json
from http.server import BaseHTTPRequestHandler, HTTPServer
from typing import Any

from .inference import MarketJEPAInference


class _LoopbackHTTPServer(HTTPServer):
    allow_reuse_address = True


def make_handler(predictor: MarketJEPAInference) -> type[BaseHTTPRequestHandler]:
    class Handler(BaseHTTPRequestHandler):
        server_version = "QoinJEPAResearch/0.1"

        def _send(self, status: int, payload: dict[str, Any]) -> None:
            body = json.dumps(payload, allow_nan=False).encode("utf-8")
            self.send_response(status)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self) -> None:
            if self.path != "/health":
                self._send(404, {"error": "not_found"})
                return
            self._send(200, {
                "status": "research_only",
                "model_version": predictor.model_version,
                "feature_names": list(predictor.feature_names),
                "context_steps": predictor.context_steps,
            })

        def do_POST(self) -> None:
            if self.path != "/predict":
                self._send(404, {"error": "not_found"})
                return
            if self.headers.get("Content-Type", "").split(";", 1)[0].strip().lower() != "application/json":
                self._send(415, {"error": "content_type_must_be_application_json"})
                return
            raw_length = self.headers.get("Content-Length", "")
            try:
                length = int(raw_length)
            except ValueError:
                self._send(411, {"error": "content_length_required"})
                return
            if length <= 0 or length > 1_048_576:
                self._send(413, {"error": "request_body_size_invalid"})
                return
            try:
                payload = json.loads(self.rfile.read(length))
                if not isinstance(payload, dict) or not isinstance(payload.get("rows"), list):
                    raise ValueError("body must be an object containing rows")
                if payload.get("feature_names") != list(predictor.feature_names):
                    raise ValueError("feature_names must exactly match checkpoint feature order")
                horizon = payload.get("horizon", 1)
                if isinstance(horizon, bool) or not isinstance(horizon, int):
                    raise ValueError("horizon must be an integer")
                result = predictor.predict(payload["rows"], horizon=horizon)
            except (json.JSONDecodeError, UnicodeDecodeError, ValueError, TypeError) as exc:
                self._send(400, {"error": "invalid_prediction_request", "detail": str(exc)})
                return
            self._send(200, result)

        def log_message(self, format: str, *args: object) -> None:
            # Avoid logging potentially sensitive market/account payloads.
            return

    return Handler


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", required=True)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8765)
    args = parser.parse_args()
    if args.host not in {"127.0.0.1", "::1", "localhost"}:
        parser.error("research inference server may bind only to loopback addresses")
    if not 1 <= args.port <= 65535:
        parser.error("port must be in [1, 65535]")
    predictor = MarketJEPAInference(args.checkpoint)
    server = _LoopbackHTTPServer((args.host, args.port), make_handler(predictor))
    try:
        print(f"JEPA research inference listening on loopback port {args.port}")
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
