# Qoin open stability risks

## Drogon `std::length_error` — root cause unproven

- **Historical failure:** workflow run [42514628818](https://github.com/capiyo/Qoin-full-repo/actions/runs/42514628818), attempt 1, job [128834833613](https://github.com/capiyo/Qoin-full-repo/actions/runs/42514628818/job/128834833613), on commit `eb650cf58fbf397f22887be8ce938c21af807273`.
- **Observed output:** the gateway returned `curl: (52) Empty reply from server`, exited before its health probe became ready, and terminated with `std::length_error` / `basic_string::_M_create`. The failure was intermittent; attempt 2 on the same commit passed. The first attempt did not produce a native stack trace.
- **Current investigation changes:** request/response/frame/query size limits; upstream response-size logging; a bounded response-copy `std::length_error` catch; a terminate handler that prints a Linux backtrace; an ASan/UBSan debug build with symbols; an oversized-request assertion; and 20 repeated ingestion/vector/backtest gateway cycles.
- **Status:** IN PROGRESS. These mitigations and diagnostics are not evidence that the historical crash is fixed. The current candidate has not yet produced a completed sanitizer/integration run or a new stack trace.
- **Next evidence required:** run the sanitized integration job on the current PR head; if the exception recurs, capture the new terminate backtrace and identify the exact throwing frame. If it does not recur, complete at least 20 successful cycles and retain the result while still describing the original root cause as unproven.
- **Do not classify this as an external blocker.** It is unfinished engineering and verification work.
