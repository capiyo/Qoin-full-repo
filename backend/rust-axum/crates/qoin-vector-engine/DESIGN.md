# Deterministic force engine design

## Direction, interaction, and gates

The engine clips caller-normalized inputs, applies a top-k **magnitude gate**, and builds one effective direction vector. The OFI × open-interest interaction is clamped to [-0.25, 0.25] and converted to an OFI-equivalent input by dividing by the OFI coefficient (0.30). Both the score dot product and cosine alignment/angle are computed from this same vector, so interaction cannot improve score while bypassing alignment. The cosine and angle checks are intentionally both retained: at max_angle_radians = acos(min_alignment), their boundaries agree; elsewhere a caller can configure either check to be stricter.

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

The config fingerprint is stable FNV-1a over all fields. It detects accidental config mismatch but is not cryptographic authentication.

## Causality invariant

For any time index t, if caller observation construction at t reads only data at indices <= t, then changing rows after t must not change the Signal at t. Re-evaluating decide with the same observation, configuration, and position flag must produce an equal Signal and byte-identical JSON serialization. The integration tests in tests/causality.rs enforce both invariants with a prefix-only mock history builder. The crate itself deliberately does not define market sampling, imputation, normalization, or smoothing policy.

## Limitations and production additions

Weights and thresholds are research placeholders, not evidence of predictive edge. A production research system would add an offline chronological calibration/validation pipeline, explicit regime detection, replayable timestamped feature provenance, and a structured logging sink. The crate remains signal-only: no sizing, execution, PnL, or live-trading behavior.
