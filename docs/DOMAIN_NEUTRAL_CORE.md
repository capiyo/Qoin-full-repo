# Qoin domain-neutral core

Qoin is a general-purpose event, vector and decision platform. Finance is its first application vertical, not the definition of the core.

The Rust workspace now includes `qoin-domain-core`, which defines shared contracts for:

- Domain identity and entity references.
- Source-attributed observations with event timestamps, measurements and quality.
- Versioned feature vectors.
- Algorithm decisions with confidence, reason codes and evidence references.

The core deliberately does not define instruments, orders, prices, portfolios, trades, crops, patients, vehicles or robots. Those belong in domain adapters and domain-specific application modules.

## Example verticals

| Vertical | Entity examples | Observation examples | Decision examples |
|---|---|---|---|
| Finance | Instrument, portfolio, account | Trade, quote, rate, filing | Allocate, rebalance, abstain |
| Agriculture | Field, crop, greenhouse | Soil moisture, rainfall, satellite observation | Irrigate, fertilize, inspect |
| Logistics | Vehicle, shipment, depot | GPS, traffic, capacity, demand | Route, dispatch, reassign |
| Energy | Meter, generator, grid node | Load, frequency, generation, weather | Dispatch, store, curtail |
| Manufacturing | Machine, line, batch | Vibration, temperature, quality result | Inspect, maintain, reschedule |
| Healthcare operations | Facility, ward, resource | Capacity, queue, equipment status | Allocate, escalate, schedule |
| Cybersecurity | Host, identity, service | Authentication, network and endpoint events | Alert, isolate, investigate |
| Robotics | Robot, sensor, workcell | Position, force, image-derived feature | Plan, coordinate, stop |

The same domain-neutral contract can be implemented by different adapters without forcing every domain into financial market terminology.

## Boundary rules

1. The core owns generic identity, observation, vector and decision contracts.
2. Each domain owns its schema, validation rules, algorithms, data connectors and safety constraints.
3. Each provider adapter preserves provenance and source time.
4. A domain decision is never automatically executable merely because it passed schema validation.
5. High-impact domains require their own human-approval, safety, regulatory and audit policies.
6. Finance execution remains disabled until the separate trading risk and operational gates pass.

The new crate is a foundation contract, not a claim that all listed verticals are implemented. The current production application remains finance-focused; other verticals still need their own adapters, algorithms, evaluation suites and user experiences.
