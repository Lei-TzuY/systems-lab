//! PTP Packet Delay Variation (PDV) Floor Filter & Min-Delay Estimator (ITU-T G.8275.2 / IEEE 1588 Annex C)
//!
//! Provides sliding-window min-delay floor selection, queuing jitter rejection,
//! outlier filtering, and stable phase offset estimation across packet-switched networks
//! lacking full on-path timing support.
//!
//! # Standard References
//! - ITU-T Recommendation G.8275.2: Precision time protocol telecom profile for phase/time synchronization with partial timing support
//! - IEEE Std 1588-2019: Standard for a Precision Clock Synchronization Protocol (Annex C: Network Impairments)

use std::collections::VecDeque;

/// A PTP Four-Timestamp Measurement Sample (t1, t2, t3, t4 in nanoseconds)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtpTimestampSample {
    pub seq_id: u16,
    pub t1_master_tx: i64,
    pub t2_slave_rx: i64,
    pub t3_slave_tx: i64,
    pub t4_master_rx: i64,
}

impl PtpTimestampSample {
    pub fn new(seq_id: u16, t1: i64, t2: i64, t3: i64, t4: i64) -> Self {
        Self {
            seq_id,
            t1_master_tx: t1,
            t2_slave_rx: t2,
            t3_slave_tx: t3,
            t4_master_rx: t4,
        }
    }

    /// Raw forward delay: (t2 - t1)
    pub fn forward_delay(&self) -> i64 {
        self.t2_slave_rx - self.t1_master_tx
    }

    /// Raw reverse delay: (t4 - t3)
    pub fn reverse_delay(&self) -> i64 {
        self.t4_master_rx - self.t3_slave_tx
    }
}

/// Filtered PTP Synchronization Estimate
#[derive(Debug, Clone, PartialEq)]
pub struct PtpFilteredEstimate {
    pub forward_delay_floor_ns: i64,
    pub reverse_delay_floor_ns: i64,
    pub mean_path_delay_ns: i64,
    pub estimated_offset_ns: i64,
    pub forward_pdv_spread_ns: i64,
    pub reverse_pdv_spread_ns: i64,
    pub floor_population_ratio: f64,
    pub valid_samples_in_window: usize,
}

/// PTP PDV Floor Filter & Min-Delay Estimator
#[derive(Debug, Clone)]
pub struct PtpPdvFloorFilter {
    pub window_size: usize,
    pub floor_threshold_percent: f64, // e.g. 5.0% lowest delay
    pub max_cluster_spread_ns: i64,    // Max allowable spread within floor cluster
    pub samples: VecDeque<PtpTimestampSample>,
}

impl PtpPdvFloorFilter {
    pub fn new(window_size: usize, floor_threshold_percent: f64, max_cluster_spread_ns: i64) -> Self {
        Self {
            window_size: window_size.max(4),
            floor_threshold_percent: floor_threshold_percent.clamp(0.01, 50.0),
            max_cluster_spread_ns: max_cluster_spread_ns.max(10),
            samples: VecDeque::with_capacity(window_size),
        }
    }

    /// Ingest a new PTP timestamp measurement sample into the sliding window
    pub fn push_sample(&mut self, sample: PtpTimestampSample) {
        if self.samples.len() >= self.window_size {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    /// Reset filter history
    pub fn clear(&mut self) {
        self.samples.clear();
    }

    /// Compute filtered phase offset and mean path delay using the window floor estimator
    pub fn compute_estimate(&self) -> Option<PtpFilteredEstimate> {
        let n = self.samples.len();
        if n < 4 {
            return None;
        }

        let mut fwd_delays: Vec<i64> = self.samples.iter().map(|s| s.forward_delay()).collect();
        let mut rev_delays: Vec<i64> = self.samples.iter().map(|s| s.reverse_delay()).collect();

        fwd_delays.sort();
        rev_delays.sort();

        let floor_count = ((n as f64 * (self.floor_threshold_percent / 100.0)).ceil() as usize).max(1);

        // Compute average of the lowest `floor_count` delays to reduce discretization noise
        let fwd_floor_slice = &fwd_delays[0..floor_count];
        let rev_floor_slice = &rev_delays[0..floor_count];

        let fwd_floor_avg = fwd_floor_slice.iter().sum::<i64>() / floor_count as i64;
        let rev_floor_avg = rev_floor_slice.iter().sum::<i64>() / floor_count as i64;

        let fwd_spread = fwd_delays[n - 1] - fwd_delays[0];
        let rev_spread = rev_delays[n - 1] - rev_delays[0];

        // Standard two-way offset calculation on filtered delay floors:
        // offset = (d_fwd - d_rev) / 2
        let estimated_offset = (fwd_floor_avg - rev_floor_avg) / 2;
        // meanPathDelay = (d_fwd + d_rev) / 2
        let mean_path_delay = (fwd_floor_avg + rev_floor_avg) / 2;

        // Ratio of samples within cluster spread of the floor
        let fwd_cluster_count = fwd_delays
            .iter()
            .filter(|&&d| (d - fwd_delays[0]) <= self.max_cluster_spread_ns)
            .count();
        let floor_population_ratio = fwd_cluster_count as f64 / n as f64;

        Some(PtpFilteredEstimate {
            forward_delay_floor_ns: fwd_floor_avg,
            reverse_delay_floor_ns: rev_floor_avg,
            mean_path_delay_ns: mean_path_delay,
            estimated_offset_ns: estimated_offset,
            forward_pdv_spread_ns: fwd_spread,
            reverse_pdv_spread_ns: rev_spread,
            floor_population_ratio,
            valid_samples_in_window: n,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ptp_pdv_floor_filter_rejection_and_convergence() {
        let mut filter = PtpPdvFloorFilter::new(20, 10.0, 100);

        // True one-way delay = 10,000 ns (10 µs)
        // True clock offset = +500 ns
        // So base t2 - t1 = 10,000 + 500 = 10,500 ns
        // base t4 - t3 = 10,000 - 500 = 9,500 ns

        // Inject 20 samples with occasional queuing spikes (+50,000 ns PDV jitter)
        for i in 0..20 {
            let jitter_fwd = if i % 4 == 0 { 0 } else { i as i64 * 3_000 }; // Spikes up to +57 µs
            let jitter_rev = if i % 4 == 0 { 0 } else { i as i64 * 4_000 };

            let t1 = (i as i64) * 1_000_000;
            let t2 = t1 + 10_500 + jitter_fwd;
            let t3 = t2 + 100_000;
            let t4 = t3 + 9_500 + jitter_rev;

            filter.push_sample(PtpTimestampSample::new(i as u16, t1, t2, t3, t4));
        }

        let estimate = filter.compute_estimate().expect("Expected valid estimate");

        // Delay floors should lock onto the true minimum delay of 10,500 ns and 9,500 ns
        assert_eq!(estimate.forward_delay_floor_ns, 10_500);
        assert_eq!(estimate.reverse_delay_floor_ns, 9_500);
        assert_eq!(estimate.mean_path_delay_ns, 10_000);
        assert_eq!(estimate.estimated_offset_ns, 500);
        assert!(estimate.forward_pdv_spread_ns > 0);
    }
}
