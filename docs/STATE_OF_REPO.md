# State of repository

Updated: 2026-11-04

## Product direction

Qoin is being built as an investment-intelligence platform for ordinary people. Customers browse instruments, build a basket and state their own goals and contribution plans. They should not need to operate a retail trading terminal or monitor short-duration market moves.

Qoin's deterministic vector engine is a research and measurement system. Signals do not submit orders, choose customer allocations or promise returns. Live execution remains disconnected.

## Active implementation branch

- Branch: `feat/funspot-inspired-archive-feed-portfolio-ui`
- Base: `main`
- Active branch is ahead of `main`; it contains the current web/mobile product work, Firebase identity, user portfolio persistence and JEPA removal.
- Do not merge without reviewing the complete branch diff and all required checks.

## Implemented

### Frontend
- React + TypeScript + Vite web client.
- Expo / React Native mobile client.
- Shared TypeScript API contracts.
- Archive and Feed product navigation.
- Archive filters for Forex, Crypto and Stocks; it loads symbols observed in explicitly asset-classed market events, while retaining the small crypto starter list.
- User instrument basket with local persistence and authenticated server synchronization.
- Firebase email/password sign-in and account creation on web and mobile.
- Investment-intention panel on web and mobile: user-authored goal, currency, monthly contribution, optional target amount and time horizon.
- The investment plan is not used to generate recommendations or transactions.
- Synthetic chart fixtures are labelled as development data; no fixture is presented as a live exchange feed.

### Backend
- Rust/Axum owns market event ingestion, state estimation, deterministic vector decisions, strategy-map persistence and the observed instrument catalogue. Market events accept optional explicit `asset_class` metadata (forex/crypto/stocks); `GET /api/v1/instruments` returns observed symbols by class.
- C++/Drogon is the HTTP gateway; the Axum services bind to loopback by default.
- MongoDB persists market events, state, decisions, strategy maps, authenticated user baskets and investment plans.
- Firebase ID tokens are verified server-side against Google's rotating RS256 signing keys, with issuer, audience, expiry and subject checks.
- User portfolio and investment-plan records are scoped by the UID extracted from the verified token; clients cannot choose the owner UID.
- The public gateway forwards authenticated portfolio and investment-plan requests to the private research API.
- JEPA model, sidecar, active dependencies, routes and active-branch documentation were removed as previously directed.

## Known gaps — not production-ready

- The instrument catalogue is intentionally incomplete: it contains the small crypto starter list plus symbols explicitly observed in ingested events. Forex and stock filters are wired, but they remain empty until their data providers publish classified events.
- No production market-data provider adapters or verified real-time quotes are connected.
- No production portfolio valuations, corporate-action handling, currency conversion, dividend/interest accounting or authoritative holdings reconciliation.
- No licensed fund-manager, broker, custodian or investment-adviser integrations are connected.
- Qoin is not yet licensed to distribute products or provide regulated advice/management; the exact Kenyan licence perimeter must be confirmed before launch.
- No customer money is held by Qoin; no live execution route is enabled.
- Authentication and persistence have a functional foundation, but rate limiting, security monitoring, privacy review, access-control review, backup/restore drills and independent penetration testing remain required.
- The investment-plan API has unit-level input validation; its full end-to-end authenticated persistence flow still requires CI/integration verification.
- Production provider-driven instrument metadata, venue-aware stable instrument identifiers, catalogue search/pagination and a complete cross-asset research pipeline remain outstanding.

## Next implementation sequence

1. Verify the latest Rust, Axum/MongoDB integration, Drogon, web and mobile CI runs after the investment-plan changes.
2. Build a provider-backed instrument catalogue for Forex, Crypto and Stocks. Do not invent instruments or label an incomplete list as the full market universe.
3. Connect normalized market-data adapters and explicit asset-class metadata to the event pipeline.
4. Extend the user portfolio from a symbol basket to transparent provider-linked holdings only after licensed partner/custody integration exists.
5. Add market and portfolio risk diagnostics with auditable explanations; do not convert research signals into automatic customer transactions.
6. Complete Kenyan legal/regulatory review and partner agreements before public investment distribution.
7. Complete production security, privacy, resilience and operational reviews.

## Safety status

Research preview only. No live order submission, customer-fund custody, guaranteed returns or production investment-management claim is permitted. Keep all live execution disabled until the relevant licensing, partner, risk, testing and explicit written-approval gates are satisfied.
