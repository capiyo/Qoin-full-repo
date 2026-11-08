# Qoin engineering blockers

## Intermittent Axum–Drogon integration process abort (2026-11-08)
- Evidence: the first attempt for workflow run 42514628818 failed in the integration step. The gateway returned an empty reply after the mixed-venue backtest assertion; the Drogon process logged `terminate called after throwing an instance of 'std::length_error'`.
- Retry: rerunning the failed job on the same code commit `eb650cf58fbf397f22887be8ce938c21af807273` completed successfully. Axum build, Drogon build, and the full integration step were green on attempt 2.
- Status: intermittent failure, root cause unknown; not considered fixed.
- Question to resolve during the next gateway investigation: why can the Drogon process abort with `std::length_error` on the first attempt, while the same integration sequence completes on retry? Add request-scoped gateway diagnostics and a regression/repetition test before closing this item.
- This is not an architecture decision blocker. Continue with independent research/backtest work while preserving this item as an open verification risk.
