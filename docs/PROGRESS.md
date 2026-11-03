# Qoin implementation progress

Updated: 2026-11-04

## Completed on the active feature branch

- Web Archive + Feed experience and mobile equivalent.
- Firebase email/password identity flows for web and React Native.
- Server-side Firebase RS256 token verification using Google's rotating signing keys.
- MongoDB persistence for the authenticated user's instrument basket, keyed by verified Firebase UID.
- Shared TypeScript API methods for authenticated portfolio persistence.
- Explicit asset-class metadata on ingested market events, MongoDB observed-instrument catalogue, and public `/api/v1/instruments?asset_class=...` route through Drogon. The list is event-observed and explicitly marked incomplete; it does not pretend to be a full market universe.
- User-authored investment-intention API and web/mobile panels:
  - goal label;
  - currency (KES, USD, EUR or GBP);
  - monthly contribution stored in minor units;
  - optional target amount stored in minor units;
  - time horizon in months.
- Investment-plan data is stored in a separate MongoDB collection and cannot be used as an order or allocation instruction.
- Drogon gateway route forwards authenticated investment-plan GET/PUT requests to the private Axum research API.
- Input validation and unit tests for goal length, currency, contribution/target bounds and time horizon.
- Product README and investment-platform regulatory-boundary documentation.
- JEPA removed from the active branch as previously instructed.

## CI verification

The latest implementation has triggered GitHub Actions for the Rust workspace, Axum/Drogon/MongoDB integration, Drogon gateway, React web and React Native mobile. Consult the run status for the current branch head before treating any of these changes as fully verified. Earlier runs passed the web production build, Drogon gateway build and Rust workspace tests; the full investment-plan integration run is still required.

## Outstanding work

1. Complete CI and integration verification for authenticated investment-plan persistence.
2. Connect production provider adapters so the observed Forex/Crypto/Stocks catalogue becomes broad, venue-aware and continuously refreshed; keep the incomplete status visible until coverage is proven.
3. Connect real market-data adapters, venue normalization, explicit asset-class metadata and freshness/quality checks.
4. Build instrument-specific multi-horizon research and causality-safe historical evaluation across supported asset classes.
5. Add user-facing risk explanations, portfolio concentration/correlation diagnostics and transparent research history.
6. Integrate licensed investment product providers, advisers/managers, brokers and custodians only after legal review and written agreements.
7. Add provider-linked holdings, valuation and independent custody reconciliation.
8. Complete production security hardening: rate limits, audit logs, key/secret management, monitoring, backups, recovery drills, privacy review and penetration testing.

## Non-negotiable boundary

Qoin is not a retail trading terminal. The research engine does not place orders. Qoin does not hold customer investment money. No live execution or public investment-product distribution is enabled by this implementation.
