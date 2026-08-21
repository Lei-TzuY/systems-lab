//! IEEE 802.1Qbv Time-Aware Shaper (TAS / TSN - Scheduled Traffic & Gate Control Lists).
//!
//! Provides deterministic transmission scheduling across 8 traffic classes (queues 0..7)
//! using repeating Gate Control Lists (GCL) and Guard Bands to eliminate latency jitter
//! and prevent frame collision during critical time windows.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GclEntry {
    pub gate_states: u8, // Bitmask: bit 0 = queue 0, bit 7 = queue 7 (1 = Open, 0 = Closed)
    pub duration_us: u32, // Duration of this time slot in microseconds
}

/// IEEE 802.1Qbv Time-Aware Shaper Engine
#[derive(Debug, Clone, Default)]
pub struct TimeAwareShaper {
    pub gcl: Vec<GclEntry>,
    pub cycle_time_us: u32,
    pub guard_band_us: u32,
    pub transmitted_frames: u64,
    pub guard_band_drops: u64,
    pub gate_closed_drops: u64,
}

impl TimeAwareShaper {
    pub fn new() -> Self {
        TimeAwareShaper {
            gcl: Vec::new(),
            cycle_time_us: 0,
            guard_band_us: 10, // Default 10µs guard band for 1Gbps MTU transmission
            transmitted_frames: 0,
            guard_band_drops: 0,
            gate_closed_drops: 0,
        }
    }

    /// Adds a scheduled slot entry to the Gate Control List (GCL)
    pub fn add_entry(&mut self, gate_states: u8, duration_us: u32) {
        self.gcl.push(GclEntry { gate_states, duration_us });
        self.cycle_time_us += duration_us;
    }

    /// Checks if a specific queue (0..7) gate is OPEN at a given absolute timestamp (in µs)
    pub fn is_queue_open(&self, queue_id: u8, time_us: u64) -> bool {
        if self.cycle_time_us == 0 || self.gcl.is_empty() || queue_id > 7 {
            return false;
        }

        let time_in_cycle = (time_us % self.cycle_time_us as u64) as u32;
        let mut accumulated_time = 0u32;

        for entry in &self.gcl {
            accumulated_time += entry.duration_us;
            if time_in_cycle < accumulated_time {
                let mask = 1u8 << queue_id;
                return (entry.gate_states & mask) != 0;
            }
        }

        false
    }

    /// Determines if a frame of `frame_len` bytes can be transmitted on `queue_id`
    /// without overrunning the open gate interval or invading the Guard Band.
    pub fn can_transmit(
        &mut self,
        queue_id: u8,
        frame_len: usize,
        link_speed_mbps: u32,
        time_us: u64,
    ) -> bool {
        if self.cycle_time_us == 0 || self.gcl.is_empty() || queue_id > 7 {
            return false;
        }

        let time_in_cycle = (time_us % self.cycle_time_us as u64) as u32;
        let mut accumulated_time = 0u32;
        let mut current_slot_opt: Option<(&GclEntry, u32)> = None;

        for entry in &self.gcl {
            let next_time = accumulated_time + entry.duration_us;
            if time_in_cycle >= accumulated_time && time_in_cycle < next_time {
                let remaining_in_slot = next_time - time_in_cycle;
                current_slot_opt = Some((entry, remaining_in_slot));
                break;
            }
            accumulated_time = next_time;
        }

        if let Some((entry, remaining_in_slot)) = current_slot_opt {
            let mask = 1u8 << queue_id;
            let is_open = (entry.gate_states & mask) != 0;

            if !is_open {
                self.gate_closed_drops += 1;
                return false;
            }

            // Calculate transmission duration in microseconds: (bits / (Mbps * 1e6)) * 1e6 = bits / Mbps
            let bits = (frame_len * 8) as u32;
            let tx_duration_us = (bits + link_speed_mbps - 1) / link_speed_mbps.max(1);

            // If non-critical queue (queue < 7), ensure frame finishes before slot ends (respect guard band)
            if queue_id < 7 {
                if tx_duration_us + self.guard_band_us > remaining_in_slot {
                    self.guard_band_drops += 1;
                    return false;
                }
            } else if tx_duration_us > remaining_in_slot {
                self.gate_closed_drops += 1;
                return false;
            }

            self.transmitted_frames += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tas_gcl_open_closed_cycles() {
        let mut tas = TimeAwareShaper::new();
        // Slot 0 (0..100µs): Only Queue 7 (Time-Critical Control: 0b10000000 = 0x80)
        tas.add_entry(0x80, 100);
        // Slot 1 (100..500µs): Queues 0..6 (Best-Effort & Bulk: 0b01111111 = 0x7F)
        tas.add_entry(0x7F, 400);

        assert_eq!(tas.cycle_time_us, 500);

        // At t=50µs (in Slot 0):
        assert!(tas.is_queue_open(7, 50));
        assert!(!tas.is_queue_open(0, 50));

        // At t=200µs (in Slot 1):
        assert!(!tas.is_queue_open(7, 200));
        assert!(tas.is_queue_open(0, 200));
        assert!(tas.is_queue_open(6, 200));
    }

    #[test]
    fn test_tas_guard_band_protection() {
        let mut tas = TimeAwareShaper::new();
        tas.guard_band_us = 15;
        // Slot 0 (0..200µs): Best Effort (Queue 0: 0x01)
        tas.add_entry(0x01, 200);
        // Slot 1 (200..300µs): Scheduled Control (Queue 7: 0x80)
        tas.add_entry(0x80, 100);

        // At t=100µs: plenty of time remaining in slot (100µs remaining > 12µs tx + 15µs guard)
        assert!(tas.can_transmit(0, 1500, 1000, 100)); // 1500 bytes at 1000Mbps ~ 12µs

        // At t=190µs: only 10µs remaining in slot. Tx + guard (12 + 15 = 27µs) > 10µs remaining -> Guard band drops
        assert!(!tas.can_transmit(0, 1500, 1000, 190));
        assert_eq!(tas.guard_band_drops, 1);
    }
}
