SHELL := /bin/bash
.PHONY: test smoke axum gateway
test:
	PYTHONPATH=backend python3 -m unittest discover -s backend/tests -v
smoke:
	bash backend/scripts/smoke.sh
axum:
	cargo run --manifest-path backend/axum/Cargo.toml
gateway:
	cmake -S backend/drogon -B build/drogon && cmake --build build/drogon
