# Qoin branch policy

## Authoritative branch

`master` is the sole authoritative development, integration, verification, and release branch for Qoin.

## Required workflow

- Make all new changes directly on `master`.
- Do not create feature, fix, integration, release, or temporary branches.
- Do not use pull requests as a parallel development workflow.
- Port any valid work from historical branches into `master` by reviewing its actual diff against the current `master`, then applying only changes that are not already present and do not regress current architecture or safety.
- Never merge a historical branch wholesale solely because it contains commits absent from `master`; historical branches may be stale, divergent, or superseded.
- Keep `master` buildable and run the relevant tests after each coherent integration.
- Update documentation and CI in the same `master` change when implementation or repository policy changes.

## Architecture and safety constraints

- Rust/Axum remains the authoritative vector/research engine and decision/risk boundary.
- C++/Drogon remains the HTTP/WebSocket gateway, not a second decision engine.
- MongoDB remains the persistence layer.
- Web, mobile, and desktop clients consume shared contracts and do not implement authoritative decisions.
- Version 1 remains the deterministic vector algorithm. Version 2 is the set-theoretic attention and relationship-ledger direction.
- JEPA is retired and must not be reintroduced.
- Qoin remains research/paper-only. Live order dispatch must remain disabled unless separately authorized and protected by the repository's safety policy.
- Broker credentials must remain server-side; user identity must be verified through the configured authentication boundary.

## Historical branch handling

Historical branches are reference material only. Before porting any change, verify whether it is already present in `master`, whether a later implementation supersedes it, and whether its tests and security assumptions remain valid. Preserve useful history in Git; do not delete or rewrite historical refs as part of routine consolidation.

## Completion standard

A change is considered integrated only when its implementation, tests, documentation, and required CI checks are present on `master`. A branch name, open pull request, draft description, or unverified CI run is not evidence of completion.
