//! IEEE 802.1Qbv Time-Aware Shaper (TAS) Gate Control List (GCL) Schedule Engine.
//!
//! Implements deterministic TSN cyclic traffic scheduling across 8 Traffic Classes (TC 0..7),
//! sub-microsecond epoch alignment, cyclic GCL entry transitions, and guard-band
//! calculation to prevent lower-priority frames from interfering with scheduled critical flows.

/// Gate Control List (GCL) Entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GclEntry {
    pub gate_states: [bool; 8], // True = Open, False = Closed for TC 0..7
    pub time_interval_ns: u64,  // Duration of this state in nanoseconds
}

impl GclEntry {
    pub fn new(gate_states: [bool; 8], interval_ns: u64) -> Self {
        GclEntry {
            gate_states,
            time_interval_ns: interval_ns,
        }
    }
}

/// IEEE 802.1Qbv TAS GCL Schedule Engine.
#[derive(Debug, Clone)]
pub struct TsnQbvGclEngine {
    pub base_time_ns: u64,
    pub cycle_time_ns: u64,
    pub entries: Vec<GclEntry>,
    pub line_rate_mbps: u64,
    pub scheduled_frames_tx: usize,
    pub guard_band_blocked_tx: usize,
}

impl TsnQbvGclEngine {
    pub fn new(base_time_ns: u64, line_rate_mbps: u64) -> Self {
        TsnQbvGclEngine {
            base_time_ns,
            cycle_time_ns: 0,
            entries: Vec::new(),
            line_rate_mbps,
            scheduled_frames_tx: 0,
            guard_band_blocked_tx: 0,
        }
    }

    /// Adds a GCL entry to the cyclic schedule.
    pub fn add_entry(&mut self, entry: GclEntry) {
        self.cycle_time_ns += entry.time_interval_ns;
        self.entries.push(entry);
    }

    /// Calculates transmission duration in nanoseconds for a given frame size.
    pub fn frame_tx_time_ns(&self, frame_bytes: usize) -> u64 {
        if self.line_rate_mbps == 0 {
            return 0;
        }
        (frame_bytes as u64 * 8 * 1000) / self.line_rate_mbps
    }

    /// Determines the active GCL entry and the remaining nanoseconds in that entry at timestamp `t`.
    pub fn get_active_state_at(&self, current_time_ns: u64) -> Option<([bool; 8], u64)> {
        if self.entries.is_empty() || self.cycle_time_ns == 0 {
            return None;
        }

        let elapsed = if current_time_ns >= self.base_time_ns {
            current_time_ns - self.base_time_ns
        } else {
            0
        };

        let cycle_offset = elapsed % self.cycle_time_ns;
        let mut accum = 0u64;

        for e in &self.entries {
            if cycle_offset < accum + e.time_interval_ns {
                let remaining = (accum + e.time_interval_ns) - cycle_offset;
                return Some((e.gate_states, remaining));
            }
            accum += e.time_interval_ns;
        }

        None
    }

    /// Evaluates if a frame of given priority (TC 0..7) and size can begin transmission.
    pub fn evaluate_transmission(
        &mut self,
        priority: u8,
        frame_bytes: usize,
        current_time_ns: u64,
    ) -> bool {
        let tc = (priority as usize) & 0x07;
        if let Some((gates, remaining_ns)) = self.get_active_state_at(current_time_ns) {
            if !gates[tc] {
                // Gate is currently closed for this traffic class
                return false;
            }

            let tx_time = self.frame_tx_time_ns(frame_bytes);
            if tx_time <= remaining_ns {
                // Frame fits in the open window without overrunning into next slot
                self.scheduled_frames_tx += 1;
                true
            } else {
                // Blocked by Guard Band to protect next scheduled slot
                self.guard_band_blocked_tx += 1;
                false
            }
        } else {
            false
        }
    }
}
