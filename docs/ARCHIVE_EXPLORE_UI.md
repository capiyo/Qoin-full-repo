# Archive and Explore frontend contract

## Current UI behavior

The web and React Native clients expose Archive and Explore as the primary instrument views. Selecting an instrument opens its vector detail surface. The screen requests the latest persisted decision from the gateway:

`GET /api/v1/vector?symbol=<exact-engine-symbol>`

The response is the Rust Axum `EdgeDecision` persisted for that symbol. The current contract contains:

- `features`: six normalized input features keyed by the names in `QOIN_FEATURE_NAMES`
- `attended_features`: the six post-attention feature values, in `FEATURE_NAMES` order
- `attention`: the 6 × 6 attention-weight matrix, rendered as a labeled heatmap
- `decision_vector`: bullish and bearish components
- `magnitude`, `angle_radians`, `action`, `reason`, and `strategy_version`

The UI plots the actual persisted two-dimensional decision vector and exposes each raw feature, attended value, strongest attention link, and the complete 6 × 6 attention heatmap. A missing decision is shown as unavailable/awaiting data; it is not replaced by fabricated live values.

## Explicit integration gaps

The current backend does not expose a user-scoped instrument archive, a discovery/universe endpoint, or a news/macro feed endpoint. The instrument rows in this first UI pass are therefore illustrative seed entries, and archive changes currently live only in client state. They are not yet synchronized across devices or persisted to MongoDB.

Before calling Archive production-ready, add authenticated endpoints and MongoDB persistence for user instrument subscriptions. Before populating Explore news, add a sourced news/event ingestion contract with publication timestamps, attribution, deduplication, and stale-item handling.

The current Axum engine stores its own symbol state and decisions. UI symbol identifiers must exactly match the symbols used by event ingestion; no automatic symbol aliasing is assumed. A symbol with no persisted decision correctly returns the backend's WAIT/no-decision response.

## Safety boundary

This screen is research visualization only. It does not submit orders, size positions, or claim that the baseline has predictive or profitable performance. The existing `docs/RISK_POLICY.md` remains binding.
