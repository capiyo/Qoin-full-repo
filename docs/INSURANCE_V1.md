# Qoin insurance and actuarial research suite

Status: deterministic V1 actuarial primitives for research and decision support. These outputs are not filed rates, binding quotes, actuarial opinions, reserving opinions, or regulatory capital calculations.

## V1 capabilities

- Frequency × severity expected loss and pure premium per exposure.
- Gross indicated premium with explicit expense, commission, risk-margin and profit loadings using a transparent denominator method.
- Compound Poisson aggregate expected loss and variance using severity mean and standard deviation.
- Per-claim excess-of-loss recovery with an optional per-claim layer limit, followed by quota-share cession.
- Simple discounted reserve estimate from reported case reserves, expected IBNR and future claim expenses.
- Input finiteness/range validation and deterministic unit tests for calculations and edge cases.

## V1 assumptions and limits

The frequency-severity aggregate model assumes a Poisson claim count and independent identically distributed severities; variance is lambda × E[X²]. Real portfolios can exhibit frequency over-dispersion, catastrophe dependence, inflation, policyholder heterogeneity, changing exposure, development patterns and parameter uncertainty. The current reserve function is a transparent present-value calculator, not a chain-ladder, Bornhuetter–Ferguson, frequency-severity reserving or stochastic reserving implementation.

The reinsurance function models a single per-claim excess-of-loss layer and then quota share on the post-layer amount. It does not model reinstatements, annual aggregate deductibles/limits, hours clauses, inuring order, facultative placements, collateral, counterparty default, recoverable disputes or treaty wording.

## Required follow-on stages

1. Policy/coverage and exposure schemas with effective dates, endorsements, limits, deductibles, exclusions and earned/unearned premium.
2. Triangle ingestion and audited chain-ladder, Bornhuetter–Ferguson and Cape Cod reserving with tail factors and diagnostics.
3. GLM frequency/severity pricing with credibility, rating territories, inflation and exposure offsets.
4. Catastrophe and accumulation modelling, dependence/copulas, scenario stress and reinsurance programme optimization.
5. Solvency/capital modules only after jurisdiction-specific legal basis, prescribed stresses, validation and independent actuarial review are established.
6. Data governance: source provenance, versioned assumptions, rate filing controls, explainability, bias/fairness review and human sign-off.

## Safety and governance

- No policy issuance, binding, premium collection, claims adjudication or automated denial is provided.
- No personal data, health data or protected-class feature is required or modelled by this crate.
- Human actuarial/underwriting review is mandatory before any operational use.
- Do not use the illustrative premium as an individual quote or represent this module as compliant with any regulator's capital or reserving framework.
