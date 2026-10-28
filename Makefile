SHELL := /bin/bash
.PHONY: test smoke axum gateway
test:
	python3 -m unittest discover -s backend/tests -v
smoke:
	bash scripts/smoke.sh
axum:
	cd backend/rust-axum && cargo run
gateway:
	cmake -S backend/cpp-drogon -B build/drogon && cmake --build build/drogon
