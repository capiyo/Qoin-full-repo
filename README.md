# Qoin Frontend Monorepo

This branch is the **frontend-only workspace** for Qoin. It keeps the React web clients and React Native mobile client together, with shared TypeScript API contracts and UI types.

## Layout

- `apps/react-web` — React + TypeScript + Vite web client.
- `apps/web` — existing Next.js React web client.
- `apps/mobile` — Expo / React Native app.
- `packages/core` — shared types, themes, mock data, and API client.
- `contracts/` — HTTP/event schemas shared with the backend team.

Backend implementation is maintained on the `backend` branch. Clients should call the API contract, not reimplement trading decisions. Axum owns strategy evaluation and persistence; Drogon is the public gateway. MongoDB is never accessed directly from a client.

## Requirements

- Node.js 20+
- pnpm 9.12.0

## Commands

```sh
pnpm install
pnpm dev:react-web
pnpm dev:web
pnpm dev:mobile
pnpm build
```

The root scripts target the frontend packages only. Configure API URLs using the environment variables documented by the backend/API contract.

## Branch policy

- `frontend`: React and React Native applications plus shared frontend package and contracts.
- `backend`: Axum, Drogon, vector engine, MongoDB integration, research and backend tests.
- `main`: integration snapshot containing both sides; use pull requests to bring tested branch work together.
