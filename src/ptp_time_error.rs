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
}
