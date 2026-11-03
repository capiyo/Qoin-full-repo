# Qoin investment platform direction

## Product purpose

Qoin is being developed to make disciplined access to investment markets easier for ordinary people. A customer should be able to state an investment intention, build a basket of instruments, understand the risks and follow the portfolio without becoming a retail trader or monitoring markets all day.

The product must not imply that investing guarantees a return. Contributions can lose value, and customers must be able to see the risks, fees, providers and custody arrangements.

## Product experience

- **Archive** is the instrument universe. Customers browse available instruments across supported asset classes and add instruments to a personal basket.
- **Feed** is the intelligence stream: verified market developments, research notes, portfolio context and explanations. It must not present synthetic fixtures as live news or market data.
- **Portfolio** contains the user's selected basket and their own investment intention, such as a goal, contribution amount, base currency and time horizon.
- Qoin's vector engine measures market conditions and portfolio relationships. It produces research observations and diagnostics; a signal is not an order.
- Customers do not receive an order ticket or need to decide short-term entry and exit points as the primary product workflow.

## User-authored investment intention

The first plan record stores only information the user supplies:

- goal label;
- base currency;
- intended monthly contribution in minor currency units;
- optional target amount in minor currency units;
- time horizon in months.

This record is a planning input, not an allocation, suitability assessment, product recommendation, promise of returns or instruction to transact. Qoin must not derive a recommended portfolio from these fields unless the relevant licensed advisory/management permissions and partner responsibilities are established.

The plan is stored under the Firebase-authenticated user's UID in the `user_investment_plans` collection. The API verifies the Firebase ID token server-side before reading or writing the record. The client never supplies the UID used for data ownership.

## Financial-services boundary

Qoin's intended partner model separates technology and research from regulated financial functions:

1. Qoin provides the interface, instrument discovery and research tooling.
2. Appropriately licensed providers supply financial products and any regulated advisory, management, dealing or execution services.
3. Licensed custodians hold client assets and provide authoritative ownership records.
4. Qoin must not receive or hold customer investment funds.

The Kenya Capital Markets Authority's public descriptions of licensed intermediary service platforms distinguish facilitation from execution, asset management and custody. The CMA's 2026 announcement on Rubiani Capital describes a non-custodial platform connecting users to products offered by regulated third parties, with execution, asset management and custody remaining with licensed institutions: https://www.cma.or.ke/cma-licenses-an-intermediary-service-platform-provider/

The 2023 Collective Investment Schemes Regulations include specific ISPP conduct restrictions, including prohibitions on holding client funds and offering investment advice. The CMA's current framework also lists the Capital Markets (Licensing Requirements) (General) Regulations 2025 and the Virtual Asset Service Providers Act 2025 and Regulations 2026. The exact licence perimeter for Qoin's planned product universe—including listed securities, online forex and virtual assets—must be confirmed with Kenyan capital-markets counsel and the CMA before public distribution or launch. References:
- https://www.parliament.go.ke/sites/default/files/2024-03/Capital%20markets%20regulations%2C%202023%20%28LN%20NO.%20170%20OF%202023%2C%20LN%20NO.%20171%20OF%202023%2C%20LN%20NO.%20172%20OF%202023%20AND%20LN%20NO.%20173%20OF%202023%29-compressed.pdf
- https://www.cma.or.ke/regulatory-framework/
- https://www.cma.or.ke/licensing/

Do not assume that describing the algorithm as research exempts Qoin from regulation if its actual conduct amounts to product distribution, investment advice, discretionary management or execution.

## Required controls before a live investment product

- Confirm the legal entity, licence category and permitted activities with the CMA and counsel.
- Sign written agreements with each licensed product provider and custodian, defining custody, execution, complaints, fees, outages, records and data responsibilities.
- Show the customer the exact product, issuer/manager, provider, custodian, fees, liquidity terms, risk disclosures and withdrawal/redemption route.
- Reconcile any displayed holdings and valuations against authoritative provider/custodian records.
- Enforce user ownership server-side and keep a durable audit trail of consent, disclosures and material account actions.
- Encrypt sensitive data in transit and at rest; restrict internal access; rate-limit authentication and portfolio endpoints; monitor security incidents.
- Do not share customer information with third parties outside a lawful, disclosed and contractually approved relationship.
- Keep live execution disconnected until licensing, partner integrations, risk controls, independent testing and explicit written approval are complete.

## Current implementation status

Implemented on the active feature branch:
- Firebase email/password identity on web and mobile;
- server-side Firebase ID-token verification;
- UID-scoped instrument-basket persistence in MongoDB;
- user-authored investment-plan persistence in MongoDB;
- web and mobile plan-entry interfaces.

Not yet implemented or verified:
- production market-data provider adapters and a complete instrument catalogue;
- instrument-level asset-class and venue metadata across all providers;
- verified live portfolio valuations and custody reconciliation;
- regulated provider onboarding, suitability/advice boundary and transaction workflows;
- production security review, privacy review, rate limiting and deployment hardening.

The product remains a research preview. No live investments or orders are placed by these features.
