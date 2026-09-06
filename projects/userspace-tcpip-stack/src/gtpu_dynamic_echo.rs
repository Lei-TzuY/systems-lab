//! 3GPP TS 29.281 — 5G GTP-U Adaptive Heartbeat & Loss-Triggered Fast Probing Engine.
//!
//! In high-reliability 5G / 4G user-plane topologies (URLLC), GTP-U Echo Request intervals
//! are dynamically accelerated when loss or jitter degradation is detected to enable
//! sub-second fast failover.
//!
//! This module implements:
//! * Dual-mode heartbeat interval: `Normal` (e.g. 5000 ms) vs `FastProbing` (e.g. 500 ms).
//! * Loss detection state machine: Consecutive missing echo responses trigger fast probing.
//! * Automatic recovery to normal interval after sustained successful echoes.

/// Operational path health state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GtpuPathHealth {
    Healthy,
    DegradedFastProbing,
    Dead,
}

/// Adaptive GTP-U Heartbeat Engine.
#[derive(Debug, Clone)]
pub struct GtpuDynamicEchoEngine {
    pub normal_interval_ms: u64,
    pub fast_interval_ms: u64,
    pub loss_threshold_for_fast_probe: u32,
    pub max_consecutive_losses_for_dead: u32,
    pub recovery_success_threshold: u32,
    pub state: GtpuPathHealth,
    pub consecutive_losses: u32,
    pub consecutive_successes: u32,
    pub last_echo_sent_ms: u64,
    pub total_echo_requests_sent: u64,
}

impl GtpuDynamicEchoEngine {
    pub fn new(normal_interval_ms: u64, fast_interval_ms: u64) -> Self {
        GtpuDynamicEchoEngine {
            normal_interval_ms,
            fast_interval_ms,
            loss_threshold_for_fast_probe: 1,
            max_consecutive_losses_for_dead: 3,
            recovery_success_threshold: 3,
            state: GtpuPathHealth::Healthy,
            consecutive_losses: 0,
            consecutive_successes: 0,
            last_echo_sent_ms: 0,
            total_echo_requests_sent: 0,
        }
    }

    /// Returns the active heartbeat interval depending on path health state.
    pub fn current_interval_ms(&self) -> u64 {
        match self.state {
            GtpuPathHealth::Healthy => self.normal_interval_ms,
            GtpuPathHealth::DegradedFastProbing | GtpuPathHealth::Dead => self.fast_interval_ms,
        }
    }

    /// Checks if a new Echo Request should be dispatched.
    pub fn should_send_echo(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.last_echo_sent_ms) >= self.current_interval_ms()
    }

    /// Dispatches an Echo Request and updates timestamp.
    pub fn notify_echo_sent(&mut self, now_ms: u64) {
        self.last_echo_sent_ms = now_ms;
        self.total_echo_requests_sent += 1;
    }

    /// Ingests probe outcome (success or timeout).
    pub fn record_probe_result(&mut self, success: bool) {
        if success {
            self.consecutive_losses = 0;
            self.consecutive_successes += 1;
            if self.consecutive_successes >= self.recovery_success_threshold {
                self.state = GtpuPathHealth::Healthy;
            }
        } else {
            self.consecutive_successes = 0;
            self.consecutive_losses += 1;
            if self.consecutive_losses >= self.max_consecutive_losses_for_dead {
                self.state = GtpuPathHealth::Dead;
            } else if self.consecutive_losses >= self.loss_threshold_for_fast_probe {
                self.state = GtpuPathHealth::DegradedFastProbing;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gtpu_dynamic_echo_adaptation() {
        let mut engine = GtpuDynamicEchoEngine::new(5000, 500);

        // 1. Initially Healthy -> 5000 ms
        assert_eq!(engine.state, GtpuPathHealth::Healthy);
        assert_eq!(engine.current_interval_ms(), 5000);

        // 2. 1 loss -> DegradedFastProbing (500 ms)
        engine.record_probe_result(false);
        assert_eq!(engine.state, GtpuPathHealth::DegradedFastProbing);
        assert_eq!(engine.current_interval_ms(), 500);

        // 3. 2 more losses -> Dead
        engine.record_probe_result(false);
        engine.record_probe_result(false);
        assert_eq!(engine.state, GtpuPathHealth::Dead);

        // 4. 3 consecutive successes -> Recovers to Healthy
        engine.record_probe_result(true);
        engine.record_probe_result(true);
        engine.record_probe_result(true);
        assert_eq!(engine.state, GtpuPathHealth::Healthy);
        assert_eq!(engine.current_interval_ms(), 5000);
    }
}
