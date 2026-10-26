import json
from pathlib import Path
from threading import Thread
from urllib.error import HTTPError
from urllib.request import Request, urlopen
from http.server import HTTPServer

from qoin_jepa.inference import MarketJEPAInference
from qoin_jepa.serve import make_handler
from qoin_jepa.train import train


def _trained_predictor(tmp_path: Path) -> MarketJEPAInference:
    csv_path = tmp_path / "features.csv"
    with csv_path.open("w", encoding="utf-8") as handle:
        handle.write("timestamp,flow,spread\n")
        for i in range(60):
            handle.write(f"2026-04-{(i // 24) + 1:02d}T{(i % 24):02d}:00:00Z,{i * 0.2},{i % 3}\n")
    checkpoint = tmp_path / "model.pt"
    train(
        str(csv_path), str(checkpoint), context_steps=4, horizon=1,
        train_fraction=0.75, epochs=1, batch_size=8, latent_dim=4, hidden_dim=8,
    )
    return MarketJEPAInference(checkpoint)


def test_loopback_handler_health_and_prediction(tmp_path: Path) -> None:
    predictor = _trained_predictor(tmp_path)
    server = HTTPServer(("127.0.0.1", 0), make_handler(predictor))
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    base = f"http://127.0.0.1:{server.server_port}"
    try:
        with urlopen(f"{base}/health", timeout=3) as response:
            health = json.loads(response.read())
            assert health["status"] == "research_only"
        request = Request(
            f"{base}/predict",
            data=json.dumps({"rows": [[float(i), float(i % 3)] for i in range(4)]}).encode(),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urlopen(request, timeout=3) as response:
            result = json.loads(response.read())
            assert result["decision"] is None
            assert len(result["predicted_latent_state"]) == 4
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=3)


def test_loopback_handler_rejects_invalid_content_type(tmp_path: Path) -> None:
    predictor = _trained_predictor(tmp_path)
    server = HTTPServer(("127.0.0.1", 0), make_handler(predictor))
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        request = Request(
            f"http://127.0.0.1:{server.server_port}/predict",
            data=b'{"rows": []}',
            headers={"Content-Type": "text/plain"},
            method="POST",
        )
        try:
            urlopen(request, timeout=3)
            raise AssertionError("expected HTTP 415")
        except HTTPError as error:
            assert error.code == 415
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=3)
