# Qoin derivatives research suite

Status: deterministic V1 research primitives, not a dealer pricing system, valuation control, clearing member, or execution service. All outputs are model estimates and must carry input provenance and model/version metadata in the calling application.

## V1 scope

- Listed and OTC product taxonomy: spot-underlying references, forwards, futures, vanilla European options, and fixed-for-floating interest-rate swaps.
- Black–Scholes–Merton European call/put price and first-/second-order Greeks (delta, gamma, vega, theta, rho); carry-adjusted forward/futures fair value; fixed-leg vs par-reset floating-leg swap NPV.
- Strict finite/range validation and explicit contract multiplier. Volatility and rates are decimals (0.20 = 20%; 0.05 = 5%); vega/rho are per one percentage point; theta is per calendar day.
- Unit tests cover a textbook at-the-money option, put-call parity, Greeks, invalid inputs and forward carry.

## V1 boundaries and required next stages

The first implementation intentionally does not pretend to price every derivative. American/Bermudan exercise, discrete dividends, stochastic/local volatility, volatility-surface calibration, SABR/Heston, yield-curve bootstrapping, collateral/discounting, cross-currency basis, convexity adjustments, XVA, credit valuation, funding, market impact, exchange margin, clearing, lifecycle events, corporate actions, and collateral haircuts remain later work.

Before using outputs for decisions, add independent reference-price tests, market-data freshness and provenance, curve/surface versioning, calendars/day-count conventions, contract-specific settlement/expiry rules, independent valuation, stress scenarios, and walk-forward backtests. Do not infer a tradable edge from model price vs market price without executable quotes, fees, spread, liquidity, and out-of-sample validation.

## Risk and safety

- Research/paper-only. No broker adapter, order endpoint, wallet, custody, or exchange credential is introduced by this crate.
- Missing, stale, non-finite or inconsistent market inputs must result in an error/abstention, never a fabricated price.
- Options can have nonlinear/gap risk; short-option exposure can be theoretically unbounded. Report scenario loss and margin separately from Greeks.
- A model price is not a quote, a fair-value opinion is not an executable price, and Greeks are local sensitivities rather than worst-case risk.
