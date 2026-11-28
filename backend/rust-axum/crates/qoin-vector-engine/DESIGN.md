# Deterministic force engine design

## Direction, interaction, and gates

The engine validates finite inputs and configuration, clips caller-normalized inputs, applies a top-k **magnitude gate**, then runs the seven-force attention pipeline to produce one effective direction vector. The OFI × open-interest interaction is clamped to [-0.25, 0.25] and converted to an OFI-equivalent input by dividing by the OFI coefficient (0.30). Both the score dot product and cosine alignment/angle are computed from this same vector, so interaction cannot improve score while bypassing alignment.

The cosine and angle checks are intentionally both retained: at max_angle_radians = acos(min_alignment), their boundaries agree; elsewhere a configuration can make either check stricter.

## Coefficients

| Coefficient | Role | Provenance / bound |
|---|---|---|
| +0.30 | OFI directional weight | Placeholder; calibrate offline |
| +0.18 | Liquidity directional weight | Placeholder; calibrate offline |
| 0.0 | Volatility directional weight | Context-only; volatility instead dampens score |
| -0.10 | Funding directional weight | Placeholder; calibrate offline |
| +0.12 | Open-interest-change directional weight | Placeholder; calibrate offline |
| +0.12 | Liquidation-pressure directional weight | Placeholder; calibrate offline |
| -0.08 | Basis directional weight | Placeholder; calibrate offline |
| +0.025 | OFI × OI interaction default | Effective interaction clamped to [-0.25, 0.25]; calibrate offline |
| +0.15 | Volatility dampener default | Nonnegative config value; placeholder, calibrate offline |

## Audit identity and rejection

The config fingerprint is stable FNV-1a over all `EngineConfig` fields and is serialized as a 16-character lowercase hexadecimal string. It detects accidental configuration mismatch but is not cryptographic authentication.

`strategy_version` is a release/provenance label attached by the runtime after the decision. It is deliberately separate from the fingerprint: changing a label cannot change or impersonate the actual configuration identity.

`RejectReason` is a machine-readable enum for missing/wide spread, stale spread, low magnitude, misalignment, simultaneous magnitude/alignment failure, and a flat vector. Human-readable rationale is derived from this enum; downstream systems must not parse prose.

## Causality invariant

For any time index t, if caller observation construction at t reads only data at indices <= t, changing rows after t must not change the Signal at t. Re-evaluating decide with the same observation, configuration, and position flag must produce an equal Signal and byte-identical JSON serialization. The integration tests in `tests/causality.rs` enforce both invariants with a prefix-only mock history builder.

The crate deliberately does not define market sampling, imputation, normalization, or smoothing policy. The multi-horizon layer builds separate trailing windows and retains venue/symbol identity at the API boundary.

## Limitations and production additions

Weights and thresholds are research placeholders, not evidence of predictive edge. A production research system still needs offline chronological calibration/validation, explicit regime detection, replayable timestamped feature provenance, and a structured logging sink. The crate remains signal-only: no sizing, execution, PnL, or live-trading behavior.
