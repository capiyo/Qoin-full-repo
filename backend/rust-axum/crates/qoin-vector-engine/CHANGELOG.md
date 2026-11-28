# Changelog

## Unreleased — deterministic engine auditability

### Breaking changes

- Removed the caller-supplied `config_version` argument from `decide`. A caller-controlled label could disagree with the actual engine configuration.
- Added a stable `config_fingerprint` computed from every `EngineConfig` field. The runtime attaches its separate `strategy_version` label after evaluation; it is not used as configuration identity.
- Added `reject_reason: Option<RejectReason>` alongside derived human-readable rationale. Consumers should aggregate by the enum rather than parse rationale text.
- Made `ForceObservation::values()` public and exposed stable OFI/OI indices for the interaction contract.
- Changed OFI × OI interaction to contribute to one effective direction vector used by both score and alignment. Some edge-case decisions therefore differ from the former score-only interaction.
- The legacy `QOIN_CONFIG_VERSION` environment variable is accepted only as a fallback strategy label; it never determines the config fingerprint.
- The top-k operation is a magnitude gate, not a Pareto frontier.

### Added

- Stable config fingerprint, structured rejection reasons, index constants, causal replay tests, interaction/alignment tests, and design documentation.
