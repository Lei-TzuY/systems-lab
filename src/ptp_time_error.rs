//! PTP Telecom Time Error & Constant Time Error (cTE / dTE) Modeling (ITU-T G.8273.2 / G.8271.1).
//!
//! Implements Time Error TE(t) sampling, Constant Time Error (cTE) moving window estimation,
//! Dynamic Time Error (dTE) peak-to-peak calculation, and compliance mask checking for
//! 5G fronthaul eCPRI/O-RAN Telecom Boundary Clock Classes (Class A, B, C, and D).

/// ITU-T G.8273.2 Telecom Boundary Clock (T-BC) Performance Classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TelecomClockClass {
    /// Class A: Max |cTE| <= 100ns, Max |TE| <= 1100ns (Macro BTS).
    ClassA,
    /// Class B: Max |cTE| <= 70ns, Max |TE| <= 200ns.
    ClassB,
    /// Class C: Max |cTE| <= 30ns, Max |TE| <= 55ns (5G Fronthaul / Small Cells).
    ClassC,
    /// Class D: Max |cTE| <= 15ns, Max |TE| <= 30ns (Enhanced 5G URLLC / eCPRI).
    ClassD,
}

impl TelecomClockClass {
    /// Maximum allowable Constant Time Error (|cTE|) in nanoseconds.
    pub fn max_cte_ns(&self) -> f64 {
        match self {
            TelecomClockClass::ClassA => 100.0,
            TelecomClockClass::ClassB => 70.0,
            TelecomClockClass::ClassC => 30.0,
            TelecomClockClass::ClassD => 15.0,
        }
    }

    /// Maximum allowable absolute Time Error (|TE|) in nanoseconds.
    pub fn max_te_ns(&self) -> i64 {
        match self {
            TelecomClockClass::ClassA => 1100,
            TelecomClockClass::ClassB => 200,
            TelecomClockClass::ClassC => 55,
            TelecomClockClass::ClassD => 30,
        }
    }
}

/// Real-Time PTP Time Error & cTE/dTE Measurement Engine.
#[derive(Debug, Clone)]
pub struct PtpTimeErrorEngine {
    pub samples: Vec<i64>, // Ring buffer of TE(t) in nanoseconds
    pub max_capacity: usize,
    pub total_samples_collected: usize,
}

impl PtpTimeErrorEngine {
    pub fn new(max_capacity: usize) -> Self {
        PtpTimeErrorEngine {
            samples: Vec::with_capacity(max_capacity),
            max_capacity,
            total_samples_collected: 0,
        }
    }

    /// Records a new Time Error TE(t) sample in nanoseconds.
    pub fn add_sample(&mut self, te_ns: i64) {
        if self.samples.len() >= self.max_capacity {
            self.samples.remove(0);
        }
        self.samples.push(te_ns);
        self.total_samples_collected += 1;
    }

    /// Calculates Constant Time Error (cTE) as the arithmetic mean of TE(t) over the window.
    pub fn calculate_cte(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: i64 = self.samples.iter().sum();
        sum as f64 / self.samples.len() as f64
    }

    /// Calculates Dynamic Time Error (dTE) peak-to-peak amplitude (max - min) in nanoseconds.
    pub fn calculate_peak_to_peak_te(&self) -> i64 {
        if self.samples.is_empty() {
            return 0;
        }
        let max_val = *self.samples.iter().max().unwrap_or(&0);
        let min_val = *self.samples.iter().min().unwrap_or(&0);
        max_val - min_val
    }

    /// Verifies if current timing performance meets the ITU-T G.8273.2 target clock class.
    pub fn verify_compliance(&self, class: TelecomClockClass) -> bool {
        if self.samples.is_empty() {
            return false;
        }

        let cte = self.calculate_cte().abs();
        if cte > class.max_cte_ns() {
            return false;
        }

        let max_abs_te = self.samples.iter().map(|&s| s.abs()).max().unwrap_or(0);
        if max_abs_te > class.max_te_ns() {
            return false;
        }

        true
    }

    /// Calculates Maximum Time Interval Error (MTIE) over an observation interval of `n` samples (ITU-T G.810 / G.8271.1).
    ///
    /// MTIE(n) = max_{1 <= k <= N - n + 1} [ max_{k <= i < k + n} x[i] - min_{k <= i < k + n} x[i] ]
    pub fn calculate_mtie(&self, tau_samples: usize) -> Option<f64> {
        let n = tau_samples;
        let total = self.samples.len();
        if n == 0 || n > total {
            return None;
        }

        let mut max_diff: i64 = 0;
        for k in 0..=(total - n) {
            let window = &self.samples[k..k + n];
            let max_val = *window.iter().max().unwrap();
            let min_val = *window.iter().min().unwrap();
            let diff = max_val - min_val;
            if diff > max_diff {
                max_diff = diff;
            }
        }

        Some(max_diff as f64)
    }

    /// Calculates Time Deviation (TDEV) over an observation interval of `n` samples (ITU-T G.810).
    ///
    /// TDEV(n) = sqrt( 1 / (6 * n^2 * (N - 3n + 1)) * sum_{j=0}^{N - 3n} ( sum_{i=j}^{j + n - 1} (x[i+2n] - 2*x[i+n] + x[i]) )^2 )
    pub fn calculate_tdev(&self, tau_samples: usize) -> Option<f64> {
        let n = tau_samples;
        let total = self.samples.len();
        if n == 0 || 3 * n > total {
            return None; // Requires at least 3*n samples
        }

        let outer_limit = total - 3 * n + 1;
        let mut sum_sq: f64 = 0.0;

        for j in 0..outer_limit {
            let mut inner_sum: f64 = 0.0;
            for i in j..(j + n) {
                let x_i = self.samples[i] as f64;
                let x_in = self.samples[i + n] as f64;
                let x_i2n = self.samples[i + 2 * n] as f64;
                let second_diff = x_i2n - 2.0 * x_in + x_i;
                inner_sum += second_diff;
            }
            sum_sq += inner_sum * inner_sum;
        }

        let denom = 6.0 * (n as f64) * (n as f64) * (outer_limit as f64);
        let variance = sum_sq / denom;
        Some(variance.sqrt())
    }

    /// Calculates Time Variance (TVAR) over an observation interval of `n` samples (ITU-T G.810).
    ///
    /// TVAR(tau) = TDEV^2(tau)
    pub fn calculate_tvar(&self, tau_samples: usize) -> Option<f64> {
        self.calculate_tdev(tau_samples).map(|tdev| tdev * tdev)
    }

    /// Computes MTIE values across multiple observation intervals (tau steps).
    pub fn compute_mtie_curve(&self, tau_steps: &[usize]) -> Vec<MtiePoint> {
        let mut results = Vec::new();
        for &tau in tau_steps {
            if let Some(val) = self.calculate_mtie(tau) {
                results.push(MtiePoint {
                    tau_samples: tau,
                    mtie_ns: val,
                });
            }
        }
        results
    }

    /// Computes TDEV values across multiple observation intervals (tau steps).
    pub fn compute_tdev_curve(&self, tau_steps: &[usize]) -> Vec<TdevPoint> {
        let mut results = Vec::new();
        for &tau in tau_steps {
            if let Some(val) = self.calculate_tdev(tau) {
                results.push(TdevPoint {
                    tau_samples: tau,
                    tdev_ns: val,
                });
            }
        }
        results
    }

    /// Verifies whether the calculated MTIE curve satisfies a standard telecom mask limit.
    pub fn verify_mtie_mask(
        &self,
        mask: &TelecomSyncMask,
        tau_steps: &[usize],
        sample_interval_sec: f64,
    ) -> bool {
        for &tau in tau_steps {
            if let Some(mtie) = self.calculate_mtie(tau) {
                let tau_sec = tau as f64 * sample_interval_sec;
                let limit = mask.max_allowed_mtie_ns(tau_sec);
                if mtie > limit {
                    return false;
                }
            }
        }
        true
    }

    /// Verifies whether the calculated TDEV curve satisfies a standard telecom mask limit (ITU-T G.8262 / G.8273.2).
    pub fn verify_tdev_mask(
        &self,
        mask: &TelecomTdevMask,
        tau_steps: &[usize],
        sample_interval_sec: f64,
    ) -> bool {
        for &tau in tau_steps {
            if let Some(tdev) = self.calculate_tdev(tau) {
                let tau_sec = tau as f64 * sample_interval_sec;
                let limit = mask.max_allowed_tdev_ns(tau_sec);
                if tdev > limit {
                    return false;
                }
            }
        }
        true
    }
}

/// Point on an MTIE curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MtiePoint {
    pub tau_samples: usize,
    pub mtie_ns: f64,
}

/// Point on a TDEV curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TdevPoint {
    pub tau_samples: usize,
    pub tdev_ns: f64,
}

/// Standard Telecom Synchronization Mask for MTIE verification (ITU-T G.8273.2 / G.8262).
#[derive(Debug, Clone, PartialEq)]
pub enum TelecomSyncMask {
    /// ITU-T G.8273.2 Class C Dynamic Time Error (dTE) mask (22 ns for tau <= 100s).
    G8273_2ClassC,
    /// ITU-T G.8273.2 Class D Dynamic Time Error (dTE) mask (10 ns for tau <= 100s).
    G8273_2ClassD,
    /// Custom threshold with constant ceiling (ns).
    ConstantLimitNs(f64),
    /// Piecewise linear limit: (tau_threshold_sec, limit_below_ns, limit_above_ns).
    PiecewiseTwoStage {
        threshold_sec: f64,
        limit_below_ns: f64,
        limit_above_ns: f64,
    },
}

impl TelecomSyncMask {
    /// Computes the maximum allowable MTIE in nanoseconds for a given observation interval in seconds.
    pub fn max_allowed_mtie_ns(&self, tau_sec: f64) -> f64 {
        match self {
            TelecomSyncMask::G8273_2ClassC => {
                if tau_sec <= 100.0 {
                    22.0
                } else {
                    22.0 + 0.05 * (tau_sec - 100.0)
                }
            }
            TelecomSyncMask::G8273_2ClassD => {
                if tau_sec <= 100.0 {
                    10.0
                } else {
                    10.0 + 0.02 * (tau_sec - 100.0)
                }
            }
            TelecomSyncMask::ConstantLimitNs(limit) => *limit,
            TelecomSyncMask::PiecewiseTwoStage {
                threshold_sec,
                limit_below_ns,
                limit_above_ns,
            } => {
                if tau_sec <= *threshold_sec {
                    *limit_below_ns
                } else {
                    *limit_above_ns
                }
            }
        }
    }
}

/// Standard Telecom Synchronization Mask for TDEV verification (ITU-T G.8262 / G.8273.2).
#[derive(Debug, Clone, PartialEq)]
pub enum TelecomTdevMask {
    /// ITU-T G.8262 EEC Option 1 (SyncE wander generation limit)
    /// tau <= 0.1s: 0.25 ns
    /// 0.1s < tau <= 100s: 0.25 * sqrt(tau) ns
    /// 100s < tau <= 1000s: 2.5 ns
    G8262Option1,
    /// Custom threshold with constant ceiling (ns).
    ConstantLimitNs(f64),
    /// Piecewise linear limit: (tau_threshold_sec, limit_below_ns, limit_above_ns).
    PiecewiseTwoStage {
        threshold_sec: f64,
        limit_below_ns: f64,
        limit_above_ns: f64,
    },
}

impl TelecomTdevMask {
    /// Computes the maximum allowable TDEV in nanoseconds for a given observation interval in seconds.
    pub fn max_allowed_tdev_ns(&self, tau_sec: f64) -> f64 {
        match self {
            TelecomTdevMask::G8262Option1 => {
                if tau_sec <= 0.1 {
                    0.25
                } else if tau_sec <= 100.0 {
                    0.25 * tau_sec.sqrt()
                } else {
                    2.5
                }
            }
            TelecomTdevMask::ConstantLimitNs(limit) => *limit,
            TelecomTdevMask::PiecewiseTwoStage {
                threshold_sec,
                limit_below_ns,
                limit_above_ns,
            } => {
                if tau_sec <= *threshold_sec {
                    *limit_below_ns
                } else {
                    *limit_above_ns
                }
            }
        }
    }
}
