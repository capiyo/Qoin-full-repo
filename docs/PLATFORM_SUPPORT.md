# Qoin platform support

Qoin has three first-class client targets sharing the same account, instrument, archive, market-data and engine contracts.

| Client | Technology | Role |
|---|---|---|
| Web | React + TypeScript + Vite | Browser access |
| Mobile | Expo + React Native | Phone-first access |
| Desktop | Tauri 2 + shared React/Vite client | Native workstation for multi-instrument research |

## Desktop

The desktop app lives at `frontend/apps/desktop`. It is a Tauri 2 shell, not a second frontend implementation. It packages the existing `@qoin/react-web` client and uses the same `@qoin/core` package, Firebase identity and Drogon/Axum API contracts. Desktop-only CSS widens the research workspace, moves primary navigation into a vertical rail, and gives the instrument/vector inspection surface more room. The Force map invokes the Rust `qoin-vector-motion` crate through Tauri IPC. That solver uses a uniform spatial hash for local-neighbour repulsion, bounded integration, finite-input checks and a 2,000-particle ceiling; the browser continues to use its SVG fallback.

### Local commands

```bash
pnpm install
pnpm dev:desktop
pnpm build:desktop
```

Requirements: Node.js 22, pnpm 9, Rust stable and the platform prerequisites listed in the [Tauri v2 prerequisites guide](https://v2.tauri.app/start/prerequisites/).

The desktop app expects the existing gateway at `http://127.0.0.1:8080` by default. Start MongoDB, Axum and Drogon separately. Do not expose the Axum internal API publicly.

### Native build matrix

The Desktop GitHub Actions workflow builds on Windows, macOS and Ubuntu and uploads the generated platform bundles as workflow artifacts. Each operating system's installer should be built on its native runner.

### Security boundary

- No provider, exchange, Firebase-admin or MongoDB secrets are bundled into the desktop app.
- No filesystem, shell, updater or order-placement capability is granted by the default Tauri capability.
- Desktop uses the same Firebase client identity and shared API contracts as the other clients. Server-side Firebase ID-token verification and paid entitlement enforcement are not yet complete; desktop packaging does not grant privileged access.
- The app remains research/paper-only. A native installer does not enable live trading.
