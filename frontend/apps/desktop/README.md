# Qoin Desktop

Qoin Desktop is the native desktop client in the monorepo. It packages the shared React/Vite client in a Tauri 2 shell; it does not fork market, portfolio, authentication, strategy, or risk logic.

## Development

Requirements: Node.js, pnpm 9, Rust stable, and the native build prerequisites for Tauri on your operating system.

From the repository root:

```bash
pnpm install
pnpm dev:desktop
```

The desktop shell starts the shared web app on Vite and opens it in a native window. On desktop, the Force map calls the native Rust spatial-force solver through Tauri IPC; browser builds retain the SVG fallback. The local Drogon gateway remains at `http://127.0.0.1:8080` by default; start the Rust engine and gateway separately using the root scripts.

## Build installers

```bash
pnpm build:desktop
```

Tauri builds platform-native bundles for the operating system on which the command runs. Windows installers should be built on Windows, macOS bundles on macOS, and Linux packages on a supported Linux runner. CI should use a matrix for cross-platform release artifacts.

## Security and execution boundary

The desktop app uses the same Firebase identity and gateway/API contracts as web and mobile. It does not embed provider secrets, MongoDB credentials, exchange credentials, or model keys. It has no native filesystem, shell, updater, or order-execution permissions in its default capability set. Qoin remains in research/paper mode; desktop packaging does not enable live trading.
