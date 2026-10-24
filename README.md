# Qoin Frontend Monorepo

This branch contains the React web and React Native applications in one frontend workspace, with shared TypeScript logic and API contracts.

## Layout

- `apps/web` — React DOM + Vite + TypeScript web client.
- `apps/mobile` — Expo / React Native app.
- `packages/core` — shared types, themes, mock data, and API client.
- `contracts/` — HTTP/event schemas shared with the backend.

Backend implementation is maintained on the `backend` branch. Clients call the API contract; they do not implement trading decisions or connect directly to MongoDB. Axum owns strategy evaluation and persistence, and Drogon is the public gateway.

## Requirements

- Node.js 20+
- pnpm 9.12.0

## Commands

```sh
pnpm install
pnpm dev:web
pnpm dev:mobile
pnpm --filter @qoin/web build
```

Configure API URLs using the environment variables documented by the backend/API contract.

## Branch policy

- `frontend`: React web, React Native, shared frontend package, and API contracts.
- `backend`: Axum, Drogon, vector engine, MongoDB integration, research and backend tests.
- `main`: integration snapshot containing both sides; use pull requests to bring tested branch work together.
