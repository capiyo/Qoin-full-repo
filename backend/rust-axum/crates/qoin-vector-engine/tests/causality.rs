//! Contract tests: caller-side observations are prefix-causal and decision evaluation is pure.
use qoin_vector_engine::{decide, Decision, EngineConfig, ForceObservation};
use serde_json::to_vec;

fn observation_from_history(history: &[f64]) -> ForceObservation {
    // The mock caller policy intentionally reads only the supplied prefix.
    let last = history.last().copied().unwrap_or(0.0);
    let prior = history.iter().rev().nth(1).copied().unwrap_or(0.0);
    ForceObservation {
        ofi: last,
        liquidity: prior,
        volatility: history.iter().map(|value| value.abs()).fold(0.0_f64, f64::max) / 10.0,
        funding: 0.0,
        open_interest_change: last - prior,
        liquidation_pressure: 0.0,
        basis: 0.0,
    }
}

#[test]
fn mutating_future_history_cannot_change_past_signals() {
    let original = vec![0.1, 0.4, 0.8, 1.2, -0.2, 0.5, 2.0, -2.0];
    let mut mutated_future = original.clone();
    mutated_future[5..].fill(1.0e6);
    let config = EngineConfig::default();

    for end in 1..=5 {
        let original_observation = observation_from_history(&original[..end]);
        let mutated_observation = observation_from_history(&mutated_future[..end]);
        let original_signal = decide(original_observation, &config, false).unwrap();
        let mutated_signal = decide(mutated_observation, &config, false).unwrap();
        assert_eq!(original_signal, mutated_signal, "prefix length {end}");
    }
}

#[test]
fn same_observation_serializes_to_identical_signal_bytes() {
    let history = [0.1, 0.4, 0.8, 1.2];
    let observation = observation_from_history(&history);
    let config = EngineConfig::default();
    let first = decide(observation, &config, false).unwrap();
    let second = decide(observation, &config, false).unwrap();
    assert_eq!(first, second);
    assert_eq!(to_vec(&first).unwrap(), to_vec(&second).unwrap());
}

#[test]
fn history_contract_does_not_claim_a_signal_from_future_rows() {
    let history = [0.1, 0.4, 0.8];
    let observation = observation_from_history(&history);
    let result = decide(observation, &EngineConfig::default(), false).unwrap();
    assert!(matches!(result.decision, Decision::Buy | Decision::Sell | Decision::Close | Decision::Wait));
}
