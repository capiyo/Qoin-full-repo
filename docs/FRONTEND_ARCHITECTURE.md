# Frontend branch boundaries

The `frontend` branch owns client presentation and shared TypeScript code only.

## Applications
- `apps/web`: React DOM, Vite, TypeScript, and Tailwind-compatible CSS tooling.
- `apps/mobile`: Expo / React Native application.

## Shared package
`packages/core` contains shared API access, types, mock data, and theme tokens. Keep API payload shapes aligned with `contracts/openapi.yaml` and `contracts/market-event.schema.json`.

## Backend contract
- Both clients call Drogon as the public API gateway.
- Drogon forwards owned requests to Axum.
- Axum validates market events, computes state/vector/strategy decisions, and persists strategy and decision state in MongoDB.
- Clients must not connect directly to MongoDB or duplicate strategy mathematics.

## Workspace boundary
This branch contains only the React web client, React Native mobile client, shared frontend package, and API contracts. Rust/C++ services and server runtime code belong on the `backend` branch.
