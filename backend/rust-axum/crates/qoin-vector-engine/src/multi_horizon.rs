//! Independent, deterministic signal evaluation across trading horizons.
//!
//! Each horizon receives its own causal observation, configuration and open-position
//! state. The module deliberately reports consensus as descriptive metadata only:
//! it never combines signals into an order, position size, or portfolio allocation.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{decide, Decision, EngineConfig, EngineError, ForceObservation, Signal};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Horizon {
    OneMinute,
    FiveMinutes,
    FifteenMinutes,
    ThirtyMinutes,
    OneHour,
    FourHours,
    OneDay,
    OneWeek,
}

impl Horizon {
    pub const fn duration_seconds(self) -> u64 {
        match self {
            Self::OneMinute => 60,
            Self::FiveMinutes => 5 * 60,
            Self::FifteenMinutes => 15 * 60,
            Self::ThirtyMinutes => 30 * 60,
            Self::OneHour => 60 * 60,
            Self::FourHours => 4 * 60 * 60,
            Self::OneDay => 24 * 60 * 60,
            Self::OneWeek => 7 * 24 * 60 * 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HorizonInput {
    pub horizon: Horizon,
    /// Features must be computed causally for this horizon; do not reuse another
    /// horizon's aggregate observation as if it were independent data.
    pub observation: ForceObservation,
    pub config: EngineConfig,
    pub config_version: String,
    pub has_open_position: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HorizonSignal {
    pub horizon: Horizon,
    pub duration_seconds: u64,
    pub signal: Signal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HorizonRelationship {
    AlignedBuy,
    AlignedSell,
    Divergent,
    NoDirectionalConsensus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HorizonConsensus {
    pub relationship: HorizonRelationship,
    pub buy_count: usize,
    pub sell_count: usize,
    pub wait_count: usize,
    pub close_count: usize,
    pub directional_count: usize,
    /// The dominant side is populated only when every directional signal agrees.
    pub unanimous_direction: Option<Decision>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MultiHorizonSignal {
    pub horizons: Vec<HorizonSignal>,
    pub consensus: HorizonConsensus,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum MultiHorizonError {
    #[error("at least one horizon is required")]
    Empty,
    #[error("each horizon may appear only once")]
    DuplicateHorizon,
    #[error(transparent)]
    Engine(#[from] EngineError),
}

pub fn evaluate_multi_horizon(
    inputs: &[HorizonInput],
) -> Result<MultiHorizonSignal, MultiHorizonError> {
    if inputs.is_empty() {
        return Err(MultiHorizonError::Empty);
    }

    let mut seen = BTreeSet::new();
    let mut horizons = Vec::with_capacity(inputs.len());
    let mut buy_count = 0;
    let mut sell_count = 0;
    let mut wait_count = 0;
    let mut close_count = 0;

    for input in inputs {
        if !seen.insert(input.horizon) {
            return Err(MultiHorizonError::DuplicateHorizon);
        }

        let signal = decide(
            input.observation,
            &input.config,
            &input.config_version,
            input.has_open_position,
        )?;

        match signal.decision {
            Decision::Buy => buy_count += 1,
            Decision::Sell => sell_count += 1,
            Decision::Wait => wait_count += 1,
            Decision::Close => close_count += 1,
        }

        horizons.push(HorizonSignal {
            horizon: input.horizon,
            duration_seconds: input.horizon.duration_seconds(),
            signal,
        });
    }

    // Keep output ordering stable regardless of caller input order.
    horizons.sort_by_key(|item| item.horizon);
    let directional_count = buy_count + sell_count;
    let (relationship, unanimous_direction) = match (buy_count, sell_count) {
        (buy, 0) if buy > 0 => (HorizonRelationship::AlignedBuy, Some(Decision::Buy)),
        (0, sell) if sell > 0 => (HorizonRelationship::AlignedSell, Some(Decision::Sell)),
        (buy, sell) if buy > 0 && sell > 0 => (HorizonRelationship::Divergent, None),
        _ => (HorizonRelationship::NoDirectionalConsensus, None),
    };

    Ok(MultiHorizonSignal {
        horizons,
        consensus: HorizonConsensus {
            relationship,
            buy_count,
            sell_count,
            wait_count,
            close_count,
            directional_count,
            unanimous_direction,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(horizon: Horizon, observation: ForceObservation) -> HorizonInput {
        HorizonInput {
            horizon,
            observation,
            config: EngineConfig::default(),
            config_version: "research-baseline-v1".to_owned(),
            has_open_position: false,
        }
    }

    fn bullish() -> ForceObservation {
        ForceObservation {
            ofi: 2.0,
            liquidity: 1.0,
            volatility: 0.2,
            funding: -0.5,
            open_interest_change: 1.0,
            liquidation_pressure: 0.5,
            basis: 0.0,
        }
    }

    fn bearish() -> ForceObservation {
        ForceObservation {
            ofi: -2.0,
            liquidity: -1.0,
            volatility: 0.2,
            funding: 0.5,
            open_interest_change: -1.0,
            liquidation_pressure: -0.5,
            basis: 0.0,
        }
    }

    #[test]
    fn evaluates_horizons_independently_and_reports_divergence() {
        let result = evaluate_multi_horizon(&[
            input(Horizon::OneDay, bearish()),
            input(Horizon::FiveMinutes, bullish()),
        ])
        .unwrap();

        assert_eq!(result.consensus.relationship, HorizonRelationship::Divergent);
        assert_eq!(result.consensus.buy_count, 1);
        assert_eq!(result.consensus.sell_count, 1);
        assert_eq!(result.horizons[0].horizon, Horizon::FiveMinutes);
        assert_eq!(result.horizons[1].horizon, Horizon::OneDay);
    }

    #[test]
    fn unanimous_direction_is_descriptive_not_an_order() {
        let result = evaluate_multi_horizon(&[
            input(Horizon::OneHour, bullish()),
            input(Horizon::FourHours, bullish()),
        ])
        .unwrap();

        assert_eq!(result.consensus.relationship, HorizonRelationship::AlignedBuy);
        assert_eq!(result.consensus.unanimous_direction, Some(Decision::Buy));
    }

    #[test]
    fn rejects_empty_and_duplicate_horizon_sets() {
        assert_eq!(evaluate_multi_horizon(&[]).unwrap_err(), MultiHorizonError::Empty);
        let duplicate = [
            input(Horizon::FiveMinutes, bullish()),
            input(Horizon::FiveMinutes, bearish()),
        ];
        assert_eq!(
            evaluate_multi_horizon(&duplicate).unwrap_err(),
            MultiHorizonError::DuplicateHorizon
        );
    }

    #[test]
    fn supports_one_minute_through_one_week_durations() {
        assert_eq!(Horizon::OneMinute.duration_seconds(), 60);
        assert_eq!(Horizon::FiveMinutes.duration_seconds(), 300);
        assert_eq!(Horizon::OneHour.duration_seconds(), 3_600);
        assert_eq!(Horizon::OneDay.duration_seconds(), 86_400);
        assert_eq!(Horizon::OneWeek.duration_seconds(), 604_800);
    }
}
