# Version 2 ledger operations runbook

Status: research/operations design guidance. The current master implementation persists V2 relationship events and exposes bounded internal read/discovery handlers; resumable backfill, correction approval, and complete integrity-audit workflows described below are not all wired end-to-end. This document does not grant trading authority or authorize production changes.

## Safety invariants

- V1 remains the only authoritative decision path. V2 records and projections must never alter V1 vectors, strategy configuration, risk decisions, or orders.
- Treat relation edges as observed temporal precedence, never causation.
- Preserve source observations. Corrections are proposals that supersede prior interpretations; do not edit or delete source evidence to make an audit pass.
- Scope every operation to one canonical instrument symbol and venue-aware source identity. Never combine different symbols in a relation universe.
- Backfill and integrity scans must be bounded and resumable. A partial page or truncated scan is not a complete audit.
- Do not expose operator-only maintenance operations to browser/mobile clients.

## Record model and integrity

Each observation record is append-only and carries a deterministic record ID, canonical source-event key, source event payload, SHA-256 fingerprint, event timestamp, ingestion timestamp, set memberships, and explicit trade-size observed/missing indicators.

Each relation-edge record carries its deterministic relation ID, source and target event keys, source/target event IDs and venues, event timestamps, positive lag, relation window, and a non-causal interpretation. The design target caps persisted relation edges at 100 per relation type for an ingested event and records candidate/persisted counts plus a truncation flag. Verify the exact endpoint and schema on the deployed commit before treating this as operational behavior.

The read projection is bounded by the requested raw-event limit. It reports universe count, requested limit, truncation, skipped invalid events, time bounds, exact counts and volume over the returned universe, and at most 100 example edges per relation. A response with truncation is a partial view, not a complete history.

## Routine maintenance

1. Confirm the service is in research/paper mode and that the maintenance identity is an authorized operator. Never use a client Firebase UID as operator proof unless the backend has independently verified its ID token and role.
2. Record the target symbol, venue scope, event-time range, requested batch limit, operator, start time, and the current database backup/recovery point.
3. Run a bounded backfill batch using the repository's operator-only backfill path. Persist the returned continuation cursor exactly; resume from that cursor rather than restarting an unbounded scan.
4. Record processed, skipped, duplicate, and remaining counts. Idempotent retries should retain the same deterministic record IDs; a duplicate is not evidence that the source event was revalidated.
5. Run the bounded integrity audit for the same symbol and scope. Preserve the full audit result, including missing observations, invalid rows, hash mismatches, unresolved endpoints, and any truncation/continuation indicator.
6. Continue audit pages until the service explicitly reports completion. If the scan is truncated or the continuation cursor is missing unexpectedly, stop and investigate rather than recording a clean result.
7. Compare the observation source hash with the canonical raw event. Check both source and target endpoint identity before accepting a relation edge.
8. Store the maintenance report and the exact code revision alongside the audit result.

## Corrections

- Prefer a correction proposal that includes the original source identity and expected source hash.
- Reject a proposal if the current source hash differs from the expected hash; re-fetch and investigate the changed evidence.
- Preserve the original observation and prior proposals. A correction must be represented as a new superseding record with an operator, reason, timestamp, and source-hash reference.
- Never rewrite a historical relation to imply causation or to conceal missing source data.
- Require a second operator review for corrections that change source interpretation or membership. If two-person review is not supported by the current endpoint, keep the proposal pending and do not apply it.

## Recovery and incident handling

1. Stop the affected maintenance worker; do not stop market ingestion or V1 decision processing unless the incident independently requires it.
2. Capture service logs, request identifiers, symbol/venue scope, cursor, batch bounds, and the failed record IDs.
3. Take or verify a MongoDB backup before any repair attempt. Do not run ad-hoc deletes or bulk updates against append-only collections.
4. Resume from the last durable cursor after confirming deterministic IDs and idempotent write behavior. If cursor integrity is uncertain, start a new bounded audit to establish actual coverage before backfill resumes.
5. For hash mismatches or unresolved relation endpoints, preserve the evidence and create a correction proposal. Do not silently regenerate source hashes or relation records.
6. After recovery, rerun the bounded audit over the affected range and retain both the pre-recovery and post-recovery reports.
7. Escalate repeated mismatch, cursor loss, or database restore events for independent review before declaring the ledger healthy.

## Operational limitations

- Current relation construction is a bounded 60-second temporal-precedence model, not a general causal graph.
- The read projection is derived from the raw events returned for a request; durable observation/relation records are stored separately.
- The current UI/API contract exposes truncation metadata, but user-facing pagination/continuation for ledger reconstruction and relation browsing remains an implementation gap.
- This runbook describes safe operating practice; it does not claim that automated secret rotation, backup/restore drills, two-person correction approval, or all pagination paths are already implemented.
- V2 remains descriptive and research-only. No V2 output may authorize or submit an order.


## Master implementation boundary

- Implemented in master: a standalone V2 set-theoretic attention crate; a MongoDB-backed relationship-event collection with event-ID uniqueness and chronological indexes; internal relationship append, bounded list, and discovery handlers; and a direction-neutral forward-window evaluation module.
- Not yet verified as an end-to-end operator workflow: source-observation backfill, continuation cursors, two-person correction approval, complete integrity scanning, and durable maintenance-report storage.
- V2 remains descriptive/research-only. It cannot modify V1 decisions, select an execution broker, or submit orders.
