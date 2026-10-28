# Changelog

## Unreleased — deterministic engine auditability refactor

### Breaking changes

- Removed the caller-supplied `config_version: &str` argument from `decide`. The old string could disagree with the actual config; Signal now exposes a computed `config_fingerprint`.
- Removed the API's `QOIN_CONFIG_VERSION` label from signal generation; configuration identity is derived from the actual field values instead of a free-text environment label.
- Replaced the old free-text-only rationale contract with `reject_reason: Option<RejectReason>` plus derived human-readable rationale. Consumers should aggregate by the enum rather than parse text.
- Made `ForceObservation::values()` public to document and test the stable field/index contract used by the interaction.
- Added `config_fingerprint`, `reject_reason`, and updated rationale semantics to Signal. Downstream struct literals and serialized schemas must account for these fields.
- Changed the OFI × OI interaction to contribute to a single effective direction vector used by both score and alignment. Some edge-case decisions can therefore differ from the previous score-only interaction behavior.
- Renamed the top-k concept in documentation from “Pareto-style” selection to a magnitude gate; it does not compute a Pareto frontier.

### Added

- Stable config fingerprint, structured rejection reasons, index constants, causal replay tests, edge-case tests, and design documentation.
