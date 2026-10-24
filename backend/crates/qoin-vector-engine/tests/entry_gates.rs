use qoin_vector_engine::{DecisionContext, EdgeStrategy, PositionSide};

fn passing_context() -> DecisionContext {
    DecisionContext {
        expected_edge_bps: Some(50.0),
        fee_bps: Some(2.0),
        slippage_bps: Some(2.0),
        state_age_ms: Some(100),
        state_quality: Some(0.9),
        persistence_ticks: 3,
        feed_sequence_valid: true,
        is_entry_tick: true,
        regime_allowed: true,
        position_side: None,
    }
}

#[test]
fn non_market_event_cannot_trigger_entry_or_exit() {
    let strategy = EdgeStrategy::baseline();
    let mut context = passing_context();
    context.is_entry_tick = false;
    context.position_side = Some(PositionSide::Long);
    let result = strategy.decide_with_forces(
        "BTCUSDT", -0.9, -0.4, Some(0.001), Some(1.0), 0.001, 4.0,
        0.0, 0.01, Some(-0.0001), &context,
    ).unwrap();
    assert_eq!(result.action, "wait");
    assert!(result.reason.contains("trade/book ticks"));
}

#[test]
fn missing_sequence_continuity_vetoes_candidate() {
    let strategy = EdgeStrategy::baseline();
    let mut context = passing_context();
    context.feed_sequence_valid = false;
    let result = strategy.decide_with_forces(
        "BTCUSDT", 0.9, 0.4, Some(0.001), Some(1.0), 0.001, 4.0,
        0.0, 0.01, Some(0.0001), &context,
    ).unwrap();
    assert_eq!(result.action, "wait");
    assert!(result.reason.contains("sequence continuity"));
}

#[test]
fn low_state_quality_vetoes_candidate() {
    let strategy = EdgeStrategy::baseline();
    let mut context = passing_context();
    context.state_quality = Some(0.1);
    let result = strategy.decide_with_forces(
        "BTCUSDT", 0.9, 0.4, Some(0.001), Some(1.0), 0.001, 4.0,
        0.0, 0.01, Some(0.0001), &context,
    ).unwrap();
    assert_eq!(result.action, "wait");
    assert!(result.reason.contains("state quality"));
}
