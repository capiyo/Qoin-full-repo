SHELL := /bin/bash
.PHONY: test smoke axum gateway
test:
	python3 -m unittest discover -s backend/tests -v
smoke:
	bash backend/scripts/smoke.sh
axum:
	cd backend/axum && cargo run
gateway:
	cmake -S backend/drogon -B build/drogon && cmake --build build/drogon
