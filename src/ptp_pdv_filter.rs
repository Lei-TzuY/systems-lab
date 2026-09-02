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
    pub asymmetry_compensation_ns: i64,
    pub smoothed_offset_ns: Option<f64>,
}

/// PTP Time Error Conformance Metrics (ITU-T G.8271 / G.8275.1 / G.8275.2)
#[derive(Debug, Clone, PartialEq)]
pub struct PtpTimeErrorMetrics {
    /// Constant Time Error (cTE): Average phase offset across the measurement window (ns)
    pub cte_ns: f64,
    /// Dynamic Time Error (dTE): Peak-to-peak phase fluctuation around the mean (ns)
    pub dte_peak_to_peak_ns: i64,
    /// Maximum Absolute Time Error: max(|TE(t)|) across the observation interval (ns)
    pub max_abs_te_ns: i64,
    /// Sample count evaluated
    pub sample_count: usize,
}

impl PtpPdvFloorFilter {
    pub fn new(window_size: usize, floor_threshold_percent: f64, max_cluster_spread_ns: i64) -> Self {
        Self {
            window_size: window_size.max(4),
            floor_threshold_percent: floor_threshold_percent.clamp(0.01, 50.0),
            max_cluster_spread_ns: max_cluster_spread_ns.max(10),
            samples: VecDeque::with_capacity(window_size),
            asymmetry_compensation_ns: 0,
            smoothed_offset_ns: None,
        }
    }

    pub fn with_asymmetry_compensation(mut self, asym_ns: i64) -> Self {
        self.asymmetry_compensation_ns = asym_ns;
        self
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

    /// Computes Constant Time Error (cTE) and Dynamic Time Error (dTE) metrics
    /// across the current sample window.
    pub fn compute_time_error_metrics(&self) -> Option<PtpTimeErrorMetrics> {
        let n = self.samples.len();
        if n < 4 {
            return None;
        }

        let offsets: Vec<i64> = self
            .samples
            .iter()
            .map(|s| (s.forward_delay() - s.reverse_delay()) / 2)
            .collect();

        let sum: i64 = offsets.iter().sum();
        let cte = sum as f64 / n as f64;

        let min_offset = *offsets.iter().min().unwrap();
        let max_offset = *offsets.iter().max().unwrap();
        let dte_peak_to_peak = max_offset - min_offset;

        let max_abs = offsets.iter().map(|&o| o.abs()).max().unwrap();

        Some(PtpTimeErrorMetrics {
            cte_ns: cte,
            dte_peak_to_peak_ns: dte_peak_to_peak,
            max_abs_te_ns: max_abs,
            sample_count: n,
        })
    }

    /// Filters extreme delay spikes using Interquartile Range (IQR) outlier detection.
    pub fn filter_iqr_outliers(delays: &[i64]) -> Vec<i64> {
        if delays.len() < 4 {
            return delays.to_vec();
        }
        let mut sorted = delays.to_vec();
        sorted.sort();

        let n = sorted.len();
        let q1 = sorted[n / 4];
        let q3 = sorted[(3 * n) / 4];
        let iqr = q3 - q1;
        let upper_cutoff = q3.saturating_add(iqr.saturating_mul(3));

        sorted.into_iter().filter(|&d| d <= upper_cutoff).collect()
    }

    /// Partitions sliding window into subwindows, selects the minimum-delay "lucky packet"
    /// per subwindow, and aggregates to prevent sample bunching under bursty queuing.
    pub fn compute_subwindow_lucky_estimate(&self, subwindow_count: usize) -> Option<PtpFilteredEstimate> {
        let n = self.samples.len();
        let k = subwindow_count.max(2);
        if n < k * 2 {
            return self.compute_estimate();
        }

        let chunk_size = n / k;
        let sample_vec: Vec<&PtpTimestampSample> = self.samples.iter().collect();
        let mut fwd_floors = Vec::with_capacity(k);
        let mut rev_floors = Vec::with_capacity(k);

        for chunk in sample_vec.chunks(chunk_size) {
            if chunk.is_empty() {
                continue;
            }
            let min_fwd = chunk.iter().map(|s| s.forward_delay()).min().unwrap();
            let min_rev = chunk.iter().map(|s| s.reverse_delay()).min().unwrap();
            fwd_floors.push(min_fwd);
            rev_floors.push(min_rev);
        }

        if fwd_floors.is_empty() {
            return None;
        }

        let fwd_avg = fwd_floors.iter().sum::<i64>() / fwd_floors.len() as i64;
        let rev_avg = rev_floors.iter().sum::<i64>() / rev_floors.len() as i64;

        let estimated_offset = (fwd_avg - rev_avg) / 2;
        let mean_path_delay = (fwd_avg + rev_avg) / 2;

        let fwd_spread = fwd_floors.iter().max().unwrap() - fwd_floors.iter().min().unwrap();
        let rev_spread = rev_floors.iter().max().unwrap() - rev_floors.iter().min().unwrap();

        Some(PtpFilteredEstimate {
            forward_delay_floor_ns: fwd_avg,
            reverse_delay_floor_ns: rev_avg,
            mean_path_delay_ns: mean_path_delay,
            estimated_offset_ns: estimated_offset,
            forward_pdv_spread_ns: fwd_spread,
            reverse_pdv_spread_ns: rev_spread,
            floor_population_ratio: fwd_floors.len() as f64 / n as f64,
            valid_samples_in_window: n,
        })
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

        // Standard two-way offset calculation on filtered delay floors with asymmetry compensation:
        // offset = ((d_fwd - d_rev) - asymmetry) / 2
        let estimated_offset = ((fwd_floor_avg - rev_floor_avg) - self.asymmetry_compensation_ns) / 2;
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

    /// Computes percentage of samples in the window within `floor_width_ns` of the minimum delay
    /// for forward and reverse directions: `(fwd_percent, rev_percent)` (ITU-T G.8275.2 Section 6.2).
    pub fn floor_packet_percentage(&self, floor_width_ns: i64) -> (f64, f64) {
        let n = self.samples.len();
        if n == 0 {
            return (0.0, 0.0);
        }

        let min_fwd = self.samples.iter().map(|s| s.forward_delay()).min().unwrap();
        let min_rev = self.samples.iter().map(|s| s.reverse_delay()).min().unwrap();

        let fwd_floor_count = self
            .samples
            .iter()
            .filter(|s| s.forward_delay() <= min_fwd + floor_width_ns)
            .count();
        let rev_floor_count = self
            .samples
            .iter()
            .filter(|s| s.reverse_delay() <= min_rev + floor_width_ns)
            .count();

        (
            (fwd_floor_count as f64 / n as f64) * 100.0,
            (rev_floor_count as f64 / n as f64) * 100.0,
        )
    }

    /// Evaluates whether the floor packet rate in both forward and reverse paths meets the minimum
    /// operational threshold required to maintain phase synchronization lock (ITU-T G.8275.2).
    pub fn is_floor_rate_adequate(&self, min_rate_percent: f64, floor_width_ns: i64) -> bool {
        let (fwd_rate, rev_rate) = self.floor_packet_percentage(floor_width_ns);
        fwd_rate >= min_rate_percent && rev_rate >= min_rate_percent
    }

    /// Updates and returns the exponential moving average (EMA) of the estimated phase offset
    /// with smoothing factor `alpha` in range (0.0, 1.0].
    pub fn update_smoothed_offset(&mut self, alpha: f64) -> Option<f64> {
        let current_estimate = self.compute_estimate()?;
        let raw_offset = current_estimate.estimated_offset_ns as f64;
        let clamped_alpha = alpha.clamp(0.001, 1.0);

        let new_smoothed = match self.smoothed_offset_ns {
            Some(prev) => prev * (1.0 - clamped_alpha) + raw_offset * clamped_alpha,
            None => raw_offset,
        };
        self.smoothed_offset_ns = Some(new_smoothed);
        Some(new_smoothed)
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
