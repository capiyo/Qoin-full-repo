use std::time::{SystemTime, UNIX_EPOCH};

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64).unwrap_or(0)
}
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CycleConfig {
    pub observation_minutes: u64,
    pub weight_evaluation_minutes: u64,
    pub session_minutes: u64,
    pub mode: String,
}

impl Default for CycleConfig {
    fn default() -> Self {
        Self {
            observation_minutes: 5,
            weight_evaluation_minutes: 60,
            session_minutes: 480,
            mode: "paper".to_owned(),
        }
    }
}

impl CycleConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.observation_minutes == 0 || self.observation_minutes > 120 {
            return Err("observation_minutes must be between 1 and 120".into());
        }
        if self.weight_evaluation_minutes == 0 || self.weight_evaluation_minutes > 720 {
            return Err("weight_evaluation_minutes must be between 1 and 720".into());
        }
        if self.session_minutes == 0 || self.session_minutes > 1440 {
            return Err("session_minutes must be between 1 and 1440".into());
        }
        if self.mode != "research" && self.mode != "paper" {
            return Err("mode must be research or paper; live mode is not supported".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CycleSnapshot {
    pub config: CycleConfig,
    pub phase: &'static str,
    pub active: bool,
    pub started_at_ms: Option<i64>,
    pub elapsed_seconds: u64,
    pub phase_seconds_remaining: u64,
    pub observations: u64,
    pub signal_evaluations: u64,
    pub live_execution_enabled: bool,
    pub weight_policy: &'static str,
    pub note: &'static str,
}

#[derive(Debug, Clone)]
pub struct OperatingCycle {
    config: CycleConfig,
    started_at_ms: Option<i64>,
    active: bool,
    observations: u64,
    signal_evaluations: u64,
}

impl Default for OperatingCycle {
    fn default() -> Self {
        Self {
            config: CycleConfig::default(),
            started_at_ms: None,
            active: false,
            observations: 0,
            signal_evaluations: 0,
        }
    }
}

impl OperatingCycle {
    pub fn configure(&mut self, config: CycleConfig) -> Result<CycleSnapshot, String> {
        config.validate()?;
        if self.snapshot().active {
            return Err("stop the current cycle before changing its configuration".into());
        }
        self.config = config;
        Ok(self.snapshot())
    }

    pub fn start(&mut self) -> CycleSnapshot {
        self.started_at_ms = Some(now_ms());
        self.active = true;
        self.observations = 0;
        self.signal_evaluations = 0;
        self.snapshot()
    }

    pub fn stop(&mut self) -> CycleSnapshot {
        self.active = false;
        self.snapshot()
    }

    pub fn record_observation(&mut self) {
        if self.active {
            self.observations = self.observations.saturating_add(1);
        }
    }

    pub fn record_signal_evaluation(&mut self) {
        if self.active {
            self.signal_evaluations = self.signal_evaluations.saturating_add(1);
        }
    }

    pub fn snapshot(&self) -> CycleSnapshot {
        let now = now_ms();
        let elapsed_seconds = self.started_at_ms
            .map(|started| ((now - started).max(0) / 1000) as u64)
            .unwrap_or(0);
        let observation_seconds = self.config.observation_minutes * 60;
        let validation_seconds = self.config.weight_evaluation_minutes * 60;
        let session_seconds = self.config.session_minutes * 60;
        let (phase, phase_seconds_remaining) = if self.started_at_ms.is_none() {
            ("idle", 0)
        } else if !self.active {
            ("stopped", 0)
        } else if elapsed_seconds < observation_seconds {
            ("observation", observation_seconds - elapsed_seconds)
        } else if elapsed_seconds < observation_seconds + validation_seconds {
            ("weight_evaluation", observation_seconds + validation_seconds - elapsed_seconds)
        } else if elapsed_seconds < observation_seconds + validation_seconds + session_seconds {
            ("paper_session", observation_seconds + validation_seconds + session_seconds - elapsed_seconds)
        } else {
            ("completed", 0)
        };
        CycleSnapshot {
            config: self.config.clone(),
            phase,
            active: self.active && phase != "completed",
            started_at_ms: self.started_at_ms,
            elapsed_seconds,
            phase_seconds_remaining,
            observations: self.observations,
            signal_evaluations: self.signal_evaluations,
            live_execution_enabled: false,
            weight_policy: "baseline_locked_during_session; validation_is_observational_not_online_training",
            note: "The five-minute window is an initial observation period, not a substitute for historical lookback. The one-hour phase evaluates signal and weight diagnostics; it does not silently retune coefficients. Session operation is research/paper only.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_requested_cycle_and_keep_live_disabled() {
        let cycle = OperatingCycle::default();
        let snapshot = cycle.snapshot();
        assert_eq!(snapshot.config.observation_minutes, 5);
        assert_eq!(snapshot.config.weight_evaluation_minutes, 60);
        assert_eq!(snapshot.config.mode, "paper");
        assert!(!snapshot.live_execution_enabled);
    }

    #[test]
    fn live_mode_is_rejected() {
        let mut cycle = OperatingCycle::default();
        let mut config = CycleConfig::default();
        config.mode = "live".into();
        assert!(cycle.configure(config).is_err());
    }

    #[test]
    fn active_cycle_cannot_be_reconfigured() {
        let mut cycle = OperatingCycle::default();
        cycle.start();
        assert!(cycle.configure(CycleConfig::default()).is_err());
    }
}
