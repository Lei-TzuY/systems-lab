//! IEEE 802.1Qav Credit-Based Shaper (CBS / TSN Audio Video Bridging).
//!
//! Implements the Credit-Based Shaper algorithm for deterministic AVB Class A / Class B
//! bandwidth reservation, credit accumulation (`idleSlope`), and depletion (`sendSlope`).

/// TSN AVB / CBS Class Shaper
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditBasedShaper {
    pub class_name: String,
    pub idle_slope_bps: i64, // Rate credit increases when waiting (bps)
    pub send_slope_bps: i64, // Rate credit decreases when transmitting (bps)
    pub port_transmit_rate_bps: i64, // Physical port speed (e.g. 1 Gbps)
    pub max_credit_bits: i64, // Upper credit bound (prevents excessive burst)
    pub min_credit_bits: i64, // Lower credit bound
    pub current_credit_bits: i64, // Current credit in bits (signed)
    pub last_update_time_us: u64, // Timestamp of last update (microseconds)
    pub is_transmitting: bool,
    pub has_queued_frames: bool,
}

impl CreditBasedShaper {
    pub fn new(
        class_name: &str,
        idle_slope_bps: i64,
        port_rate_bps: i64,
        max_interference_frame_bytes: usize,
    ) -> Self {
        let send_slope_bps = idle_slope_bps - port_rate_bps; // Negative
        let max_credit_bits =
            ((max_interference_frame_bytes as i64) * 8 * idle_slope_bps) / port_rate_bps;
        let min_credit_bits =
            ((max_interference_frame_bytes as i64) * 8 * send_slope_bps) / port_rate_bps;

        CreditBasedShaper {
            class_name: class_name.to_string(),
            idle_slope_bps,
            send_slope_bps,
            port_transmit_rate_bps: port_rate_bps,
            max_credit_bits,
            min_credit_bits,
            current_credit_bits: 0,
            last_update_time_us: 0,
            is_transmitting: false,
            has_queued_frames: false,
        }
    }

    /// Updates credit based on time elapsed and active transmission state
    pub fn advance_time(&mut self, current_time_us: u64) {
        if self.last_update_time_us == 0 {
            self.last_update_time_us = current_time_us;
            return;
        }

        let dt_us = current_time_us.saturating_sub(self.last_update_time_us);
        if dt_us == 0 {
            return;
        }

        if self.is_transmitting {
            // Credit decreases at sendSlope
            // delta_bits = (send_slope_bps * dt_us) / 1_000_000
            let delta_bits = (self.send_slope_bps * (dt_us as i64)) / 1_000_000;
            self.current_credit_bits += delta_bits; // send_slope is negative, so decreases
            if self.current_credit_bits < self.min_credit_bits {
                self.current_credit_bits = self.min_credit_bits;
            }
        } else if self.has_queued_frames {
            // Credit increases at idleSlope while waiting
            let delta_bits = (self.idle_slope_bps * (dt_us as i64)) / 1_000_000;
            self.current_credit_bits += delta_bits;
            if self.current_credit_bits > self.max_credit_bits {
                self.current_credit_bits = self.max_credit_bits;
            }
        } else {
            // No queued frames
            if self.current_credit_bits > 0 {
                // If credit was positive and queue is empty, reset to 0
                self.current_credit_bits = 0;
            } else if self.current_credit_bits < 0 {
                // If credit was negative, replenish at idleSlope until 0
                let delta_bits = (self.idle_slope_bps * (dt_us as i64)) / 1_000_000;
                self.current_credit_bits += delta_bits;
                if self.current_credit_bits > 0 {
                    self.current_credit_bits = 0;
                }
            }
        }

        self.last_update_time_us = current_time_us;
    }

    /// Determines if the class queue is permitted to transmit
    pub fn can_transmit(&self) -> bool {
        self.has_queued_frames && self.current_credit_bits >= 0
    }

    /// Signals start of frame transmission
    pub fn start_transmitting(&mut self, now_us: u64) {
        self.advance_time(now_us);
        self.is_transmitting = true;
    }

    /// Signals end of frame transmission
    pub fn finish_transmitting(&mut self, now_us: u64, still_has_queued_frames: bool) {
        self.advance_time(now_us);
        self.is_transmitting = false;
        self.has_queued_frames = still_has_queued_frames;
        if !still_has_queued_frames && self.current_credit_bits > 0 {
            self.current_credit_bits = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cbs_initialization_and_slopes() {
        // Class A: 75 Mbps on 1000 Mbps port
        let cbs = CreditBasedShaper::new("Class-A", 75_000_000, 1_000_000_000, 1500);
        assert_eq!(cbs.idle_slope_bps, 75_000_000);
        assert_eq!(cbs.send_slope_bps, -925_000_000);
        assert!(cbs.max_credit_bits > 0);
        assert!(cbs.min_credit_bits < 0);
    }

    #[test]
    fn test_cbs_credit_accumulation_and_transmission_cycle() {
        let mut cbs = CreditBasedShaper::new("Class-A", 100_000_000, 1_000_000_000, 1500);

        // 1. Frame arrives at t=100µs, queue waiting
        cbs.advance_time(100);
        cbs.has_queued_frames = true;

        // Advance 100µs while waiting -> credit accumulates: 100_000_000 * 100 / 1_000_000 = 10,000 bits
        cbs.advance_time(200);
        assert!(cbs.current_credit_bits > 0);
        assert!(cbs.can_transmit());

        // 2. Start transmitting at t=200µs
        cbs.start_transmitting(200);
        // Transmit for 50µs -> credit drops: sendSlope = -900 Mbps
        // delta = -900 * 50 = -45,000 bits -> credit = 10,000 - 45,000 = -35,000 bits
        cbs.finish_transmitting(250, true);
        assert!(cbs.current_credit_bits < 0);
        assert!(!cbs.can_transmit()); // Blocked because credit < 0
    }
}
