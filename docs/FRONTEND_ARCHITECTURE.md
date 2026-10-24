# Frontend branch boundaries

The `frontend` branch owns browser and mobile presentation only.

## Applications
- `apps/react-web`: Vite React web client.
- `apps/web`: existing Next.js React client.
- `apps/mobile`: Expo / React Native client.

## Shared package
`packages/core` is the shared TypeScript source for API access, types, mock data, and theme tokens. Keep API payload shapes aligned with `contracts/openapi.yaml` and `contracts/market-event.schema.json`.

## Backend contract
- Clients call Drogon as the public API gateway.
- Drogon forwards owned requests to Axum.
- Axum validates market events, computes state/vector/strategy decisions, and persists strategy and decision state in MongoDB.
- Clients must not connect directly to MongoDB or duplicate strategy mathematics.
