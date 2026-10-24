SHELL := /bin/bash
.PHONY: test smoke axum gateway
test:
	python3 -m unittest discover -s tests -v
smoke:
	bash scripts/smoke.sh
axum:
	cd services/axum && cargo run
gateway:
	cmake -S services/drogon -B build/drogon && cmake --build build/drogon
