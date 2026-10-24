# Qoin — organized monorepo

The repository is grouped by platform and service so each part is easy to open directly on GitHub.

## Frontend
- `frontend/web/` — React DOM + Vite + TypeScript + Tailwind web client.
- `frontend/mobile/` — Expo / React Native mobile client.
- `frontend/shared/core/` — shared TypeScript types, API client, theme and mock data.

## Backend
- `backend/axum/` — Rust trading, strategy and vector-engine service.
- `backend/axum-api/` — Rust API service.
- `backend/drogon/` — C++ public gateway.
- `backend/crates/` — reusable Rust engine crates.
- `backend/research/` — Python research and reference models.
- `backend/tests/` — backend tests.

Shared API contracts are in `contracts/`. Canonical trading safety and ownership documents are in `docs/RISK_POLICY.md`, `docs/RESPONSIBILITIES.md`, and `docs/EXPERIMENT_LOG.md`; detailed backend runbooks and architecture notes remain in `backend/docs/`.

## Commands
Install the JavaScript workspace with pnpm. Run `pnpm dev:web`, `pnpm dev:mobile`, or `pnpm --filter @qoin/web build`.
