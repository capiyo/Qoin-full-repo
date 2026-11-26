# Repository Engineering Instructions

## Mandatory branch policy

- `master` is the sole working and integration branch for all future Qoin development.
- Before making any change, check out or target the existing `master` branch.
- Commit every implementation, fix, test, documentation update, and integration change directly to `master`.
- Do not create feature, task, fix, staging, or temporary branches.
- Do not open pull requests for routine repository development; work directly on `master`.
- Do not push implementation commits to `main` or any other branch.
- If a tool or workflow cannot write directly to `master`, stop and report the exact blocker rather than silently creating another branch or using a different target.
- Preserve existing work. Inspect the current tree and history before editing; never overwrite or discard unrelated changes.
- Complete work sequentially: implement, run relevant tests/builds, fix failures, and continue to the next incomplete production-critical area. Do not replace implementation with a backlog or claim completion without evidence.

## Architecture and safety boundaries

- Rust/Axum remains the authoritative vector engine and decision/risk/state layer.
- C++/Drogon remains the HTTP/WebSocket gateway; it must not duplicate authoritative engine decisions.
- MongoDB remains the backend persistence layer.
- Web and React Native clients share the established `@qoin/core` contracts where applicable.
- Qoin authentication and user data must remain Qoin-owned. Never depend on Funspot authentication, backend services, or data.
- Keep live order execution disabled unless broker adapters, identity, persistence, risk controls, reconciliation, and release checks are implemented and independently verified.
- Do not represent preview-only, research-only, or specification-only functionality as production-complete.

## Verification and reporting

- Run the narrowest relevant tests during implementation and the applicable full validation suite before declaring a subsystem complete.
- Report the exact commit SHA, files changed, commands/checks run, and any checks that could not be run.
- Distinguish implemented, tested, configured, and deployed states. Never claim deployment or external service configuration without direct evidence.
