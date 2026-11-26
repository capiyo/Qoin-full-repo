# Qoin broker provider scope and implementation status

Updated: 2026-11-26  
Source of truth: `backend/rust-axum/src/broker_catalogue.rs`, `broker_adapter_skeleton.rs`, `broker_contract.rs`, `broker_env.rs`, and `credential_vault.rs`.

## Selected provider scope

The selected integration scope is deliberately balanced across ten crypto venues and ten FX/CFD brokers.

| # | Provider | Market group | Current adapter status |
|---:|---|---|---|
| 1 | Binance USD-M | Crypto | Scaffold only |
| 2 | Coinbase Advanced Trade | Crypto | Scaffold only |
| 3 | Kraken Spot | Crypto | Scaffold only |
| 4 | Bybit | Crypto | Scaffold only |
| 5 | OKX | Crypto | Scaffold only |
| 6 | Bitget | Crypto | Scaffold only |
| 7 | Gate.io | Crypto | Scaffold only |
| 8 | KuCoin | Crypto | Scaffold only |
| 9 | Deribit | Crypto derivatives | Scaffold only |
| 10 | Hyperliquid | Crypto perpetuals | Scaffold only |
| 11 | OANDA | FX/CFD | Scaffold only |
| 12 | IG | FX/CFD | Scaffold only |
| 13 | FXCM | FX/CFD | Scaffold only |
| 14 | Capital.com | FX/CFD | Scaffold only |
| 15 | Deriv | FX/CFD | Scaffold only |
| 16 | XTB | FX/CFD | Scaffold only |
| 17 | Pepperstone | FX/CFD | Scaffold only |
| 18 | IC Markets | FX/CFD | Scaffold only |
| 19 | FOREX.com | FX/CFD | Scaffold only |
| 20 | CMC Markets | FX/CFD | Scaffold only |

The directory is a provider-selection catalogue, not proof that an account is eligible, connected, or executable. Kenya eligibility and the actual contracting legal entity must be checked for each user and provider.

## What is implemented

- A broker-neutral Rust contract covers normalized instruments, account snapshots, capabilities, quotes, margin preflight, normalized orders, protective-stop requirements, client-order lookup, status and cancellation.
- A runtime broker registry exists, but the current default registry is empty.
- Twenty provider-specific Rust adapter types are declared in `broker_adapter_skeleton.rs`. Every operation returns an explicit unsupported error until a real provider integration is implemented and conformance-tested. They are not registered.
- `broker_catalogue.rs` lists all twenty providers and official documentation links, marks them selectable for connection preference only, and hard-codes `execution_ready=false`.
- `broker_env.rs` reads optional provider credentials only from deployment environment secrets, zeroizes secret buffers on drop, validates optional HTTPS base URLs, and never returns credentials or account identifiers from public status.
- `credential_vault.rs` provides versioned AES-256-GCM envelopes, authenticated additional data, 96-bit random nonces, key IDs, previous-key decryption during rotation, and tamper/unknown-key rejection tests.
- User-owned broker connection records are stored with Firebase UID ownership filters. Create, list, rotate and revoke routes never return encrypted credential material. New or rotated connections enter `pending_validation`, with trading disabled and `execution_ready=false`.

## Explicitly not implemented or not verified

- No provider adapter is currently registered in the runtime registry.
- The twenty provider types are fail-closed scaffolds, not working HTTP/WebSocket adapters.
- The credential vault currently stores encrypted user credentials, but no provider adapter consumes those per-user credentials yet.
- Account discovery, account selection, instrument discovery, provider capability negotiation, live quote retrieval, real order submission, cancellation, status polling, execution retrieval and reconciliation are not complete for these twenty providers.
- `broker_env.rs` deployment credentials are not a substitute for per-user credential onboarding.
- The separate operator-provisioned `broker_accounts` collection remains an allowlist for deployment-configured accounts; it is not automatic broker account discovery.
- Sandbox credentials, production credentials, rate-limit entitlements, fees, minimum sizes, margin rules, legal entity and Kenya account eligibility have not been verified for every provider.
- No provider is execution-ready. Live-money execution remains disabled by code policy.

## Market data is a separate integration surface

The market-data ingestion adapters are separate from broker account/execution adapters. The current code documents:
- Binance USD-M public market stream and REST polling.
- Coinbase Advanced Trade public trade stream.
- Kraken Spot public trade stream.
- Finnhub authenticated feed for entitled equity/FX/index/commodity data.

A working public market-data feed does not imply that Qoin can authenticate a user's broker account or submit orders through that provider.

## Credential key configuration and rotation

Set `QOIN_CREDENTIAL_ENCRYPTION_KEY` to a base64-encoded 32-byte random key in the deployment secret manager and set `QOIN_CREDENTIAL_ENCRYPTION_KEY_ID` to a stable identifier. Never commit the key or put it in frontend/mobile configuration.

During rotation:
1. Deploy the new active key and include old keys in `QOIN_CREDENTIAL_ENCRYPTION_PREVIOUS_KEYS` as a JSON object mapping key IDs to base64 keys.
2. Re-encrypt stored envelopes using the active key.
3. Verify no stored envelope references the old key.
4. Remove the old key only after backup/restore and rollback windows have expired.

If the active key is absent, credential-connection writes fail closed. If a previous key is unavailable, decryption of an envelope using that key fails closed.

## Completion gate for each provider

A provider may only move from scaffold to registered after all of the following exist and pass tests:

1. Official API documentation and correct legal entity/region are recorded.
2. Authentication and credential lifecycle are implemented without logging secrets.
3. Account discovery and explicit account selection are ownership-scoped.
4. Instrument specifications, precision, minimums, fees, margin and supported order types are retrieved from the provider.
5. Quotes and historical market data have source timestamps, freshness checks and provenance.
6. Submit, idempotent client-order lookup, status, cancellation and execution retrieval are implemented.
7. Protective exits and required order semantics are proven by sandbox contract tests.
8. Reconciliation and restart recovery are tested against recorded/sandbox fixtures.
9. Rate limits, retry/backoff and unknown-submit-outcome behavior are verified.
10. The adapter is registered only after its capability report is truthful and all safety tests pass.

Until that gate passes, the adapter must remain unregistered and all order paths must reject it.
