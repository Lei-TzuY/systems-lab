//! TCP Congestion Control (RFC 5681) and Jacobson's RTT Estimator (RFC 6298).
//!
//! Implements Slow Start, Congestion Avoidance, Fast Retransmit / Fast Recovery,
//! dynamic Sliding Window flow control, and adaptive Retransmission Timeout (RTO).

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CongestionState {
    SlowStart,
    CongestionAvoidance,
    FastRecovery,
}

impl fmt::Display for CongestionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CongestionState::SlowStart => write!(f, "SLOW_START"),
            CongestionState::CongestionAvoidance => write!(f, "CONGESTION_AVOIDANCE"),
            CongestionState::FastRecovery => write!(f, "FAST_RECOVERY"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CongestionControl {
    pub cwnd: u32,     // Congestion window in bytes
    pub ssthresh: u32, // Slow start threshold in bytes
    pub mss: u32,      // Maximum segment size in bytes
    pub dup_ack_count: u32,
    pub state: CongestionState,
    pub in_flight: u32, // Bytes currently in flight (unacknowledged)
}

impl CongestionControl {
    pub fn new(mss: u32) -> Self {
        let initial_cwnd = 2 * mss;
        CongestionControl {
            cwnd: initial_cwnd,
            ssthresh: 65535,
            mss,
            dup_ack_count: 0,
            state: CongestionState::SlowStart,
            in_flight: 0,
        }
    }

    /// Called when new in-order data is acknowledged.
    pub fn on_ack(&mut self, bytes_acked: u32) {
        if self.in_flight >= bytes_acked {
            self.in_flight -= bytes_acked;
        } else {
            self.in_flight = 0;
        }

        match self.state {
            CongestionState::SlowStart => {
                // Exponential growth: cwnd += min(bytes_acked, mss)
                self.cwnd = self.cwnd.saturating_add(bytes_acked.min(self.mss));
                if self.cwnd >= self.ssthresh {
                    self.state = CongestionState::CongestionAvoidance;
                }
                self.dup_ack_count = 0;
            }

            CongestionState::CongestionAvoidance => {
                // Linear growth: cwnd += (mss * mss) / cwnd per ACK
                if self.cwnd > 0 {
                    let increment =
                        ((self.mss as u64 * self.mss as u64) / (self.cwnd as u64)) as u32;
                    self.cwnd = self.cwnd.saturating_add(increment.max(1));
                }
                self.dup_ack_count = 0;
            }

            CongestionState::FastRecovery => {
                // On new ACK, exit Fast Recovery to Congestion Avoidance
                self.cwnd = self.ssthresh;
                self.dup_ack_count = 0;
                self.state = CongestionState::CongestionAvoidance;
            }
        }
    }

    /// Called when a duplicate ACK is received. Returns true if 3 duplicate ACKs are reached (Fast Retransmit).
    pub fn on_dup_ack(&mut self) -> bool {
        self.dup_ack_count += 1;

        if self.dup_ack_count == 3 {
            // Enter Fast Retransmit / Fast Recovery
            self.ssthresh = (self.cwnd / 2).max(2 * self.mss);
            self.cwnd = self.ssthresh + 3 * self.mss;
            self.state = CongestionState::FastRecovery;
            true
        } else if self.dup_ack_count > 3 && self.state == CongestionState::FastRecovery {
            // Inflate window for each additional duplicate ACK
            self.cwnd = self.cwnd.saturating_add(self.mss);
            false
        } else {
            false
        }
    }

    /// Called on packet retransmission timeout.
    pub fn on_timeout(&mut self) {
        self.ssthresh = (self.cwnd / 2).max(2 * self.mss);
        self.cwnd = self.mss; // Collapse cwnd back to 1 MSS
        self.dup_ack_count = 0;
        self.state = CongestionState::SlowStart;
    }

    /// Calculates how many bytes the sender is allowed to transmit now based on
    /// sliding window flow control: min(cwnd, peer_rwnd) - in_flight.
    pub fn send_window_available(&self, peer_rwnd: u16) -> u32 {
        let max_window = self.cwnd.min(peer_rwnd as u32);
        max_window.saturating_sub(self.in_flight)
    }

    pub fn record_sent(&mut self, bytes: u32) {
        self.in_flight += bytes;
    }
}

/// Jacobson's Algorithm for Adaptive Retransmission Timeout (RFC 6298).
#[derive(Debug, Clone)]
pub struct RttEstimator {
    pub srtt: Option<f64>,   // Smoothed Round-Trip Time in milliseconds
    pub rttvar: Option<f64>, // Round-Trip Time Variation in milliseconds
    pub rto: f64,            // Retransmission Timeout in milliseconds
    pub min_rto: f64,        // Lower bound (e.g. 200ms)
    pub max_rto: f64,        // Upper bound (e.g. 60000ms)
}

impl Default for RttEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl RttEstimator {
    pub fn new() -> Self {
        RttEstimator {
            srtt: None,
            rttvar: None,
            rto: 1000.0, // Initial RTO = 1.0s (RFC 6298)
            min_rto: 200.0,
            max_rto: 60000.0,
        }
    }

    /// Ingests a new RTT measurement sample (in milliseconds) and updates SRTT, RTTVAR, and RTO.
    pub fn update_sample(&mut self, rtt_sample: f64) {
        match (self.srtt, self.rttvar) {
            (None, _) => {
                // First RTT sample
                self.srtt = Some(rtt_sample);
                self.rttvar = Some(rtt_sample / 2.0);
                self.rto =
                    (rtt_sample + 4.0 * (rtt_sample / 2.0)).clamp(self.min_rto, self.max_rto);
            }
            (Some(srtt), Some(rttvar)) => {
                // Subsequent samples
                // RTTVAR = (1 - beta) * RTTVAR + beta * |SRTT - R| where beta = 1/4 (0.25)
                let new_rttvar = 0.75 * rttvar + 0.25 * (srtt - rtt_sample).abs();
                // SRTT = (1 - alpha) * SRTT + alpha * R where alpha = 1/8 (0.125)
                let new_srtt = 0.875 * srtt + 0.125 * rtt_sample;

                self.srtt = Some(new_srtt);
                self.rttvar = Some(new_rttvar);
                self.rto = (new_srtt + 4.0 * new_rttvar).clamp(self.min_rto, self.max_rto);
            }
            _ => {}
        }
    }

    /// Exponential backoff on retransmission timeout.
    pub fn backoff(&mut self) {
        self.rto = (self.rto * 2.0).min(self.max_rto);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slow_start_to_congestion_avoidance() {
        let mss = 1460;
        let mut cc = CongestionControl::new(mss);
        cc.ssthresh = 5840; // 4 * MSS

        assert_eq!(cc.cwnd, 2920); // 2 * MSS
        assert_eq!(cc.state, CongestionState::SlowStart);

        // ACK 1 MSS
        cc.on_ack(1460);
        assert_eq!(cc.cwnd, 2920 + 1460); // 4380

        // ACK another 1460 -> reaches ssthresh
        cc.on_ack(1460);
        assert_eq!(cc.cwnd, 5840);
        assert_eq!(cc.state, CongestionState::CongestionAvoidance);
    }

    #[test]
    fn test_fast_retransmit_on_three_duplicate_acks() {
        let mss = 1460;
        let mut cc = CongestionControl::new(mss);
        cc.cwnd = 10000;

        assert!(!cc.on_dup_ack()); // 1st dup ACK
        assert!(!cc.on_dup_ack()); // 2nd dup ACK
        assert!(cc.on_dup_ack()); // 3rd dup ACK -> Fast Retransmit!

        assert_eq!(cc.state, CongestionState::FastRecovery);
        assert_eq!(cc.ssthresh, 5000);
        assert_eq!(cc.cwnd, 5000 + 3 * 1460);
    }

    #[test]
    fn test_jacobson_rtt_estimator() {
        let mut rtt = RttEstimator::new();
        assert_eq!(rtt.rto, 1000.0);

        // Sample 1: 100ms
        rtt.update_sample(100.0);
        assert_eq!(rtt.srtt, Some(100.0));
        assert_eq!(rtt.rttvar, Some(50.0));
        assert_eq!(rtt.rto, 300.0); // 100 + 4*50 = 300ms

        // Sample 2: 120ms
        rtt.update_sample(120.0);
        assert!(rtt.srtt.unwrap() > 100.0 && rtt.srtt.unwrap() < 120.0);
    }
}
