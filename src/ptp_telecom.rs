//! PTP Telecom Profile ITU-T G.8275.1 / G.8275.2 (T-GM, T-BC, T-TSC & Telecom BMCA).
//!
//! Implements Telecom Profiles for Phase and Time synchronization in 5G cellular
//! mobile fronthaul / backhaul with Alternate Best Master Clock Algorithm (BMCA).

use std::cmp::Ordering;

pub const ETHERTYPE_PTP_TELECOM: u16 = 0x88F7;
pub const PTP_TELECOM_DEFAULT_LOCAL_PRIORITY: u8 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelecomClockType {
    TelecomGrandmaster,    // T-GM (PRTC / ePRTC Reference)
    TelecomBoundaryClock,  // T-BC (Fronthaul / Midhaul Switch Clock)
    TelecomTimeSlaveClock, // T-TSC (gNodeB Baseband / Radio Unit Clock)
}

/// Telecom BMCA (Best Master Clock Algorithm) Dataset Attributes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelecomBmcaAttributes {
    pub clock_class: u8,    // 6 = PRTC Traceable, 7, 140, 150, 160 = Holdover
    pub clock_accuracy: u8, // 0x20 = Within 25ns, 0x21 = 100ns
    pub offset_scaled_log_variance: u16,
    pub priority1: u8,           // Static Override Priority 1
    pub priority2: u8,           // Static Override Priority 2
    pub local_priority: u8,      // Telecom Profile Specific Priority (1..255)
    pub clock_identity: [u8; 8], // EUI-64 Clock Identity
    pub steps_removed: u16,      // Number of boundary hops from GM
}

impl TelecomBmcaAttributes {
    pub fn new_prtc_grandmaster(clock_id: [u8; 8]) -> Self {
        TelecomBmcaAttributes {
            clock_class: 6,       // PRTC locked
            clock_accuracy: 0x20, // <25ns accuracy
            offset_scaled_log_variance: 0x4000,
            priority1: 128,
            priority2: 128,
            local_priority: 100,
            clock_identity: clock_id,
            steps_removed: 0,
        }
    }

    pub fn new_slave_clock(clock_id: [u8; 8]) -> Self {
        TelecomBmcaAttributes {
            clock_class: 248,     // Slave default
            clock_accuracy: 0xFE, // Unknown
            offset_scaled_log_variance: 0xFFFF,
            priority1: 128,
            priority2: 128,
            local_priority: PTP_TELECOM_DEFAULT_LOCAL_PRIORITY,
            clock_identity: clock_id,
            steps_removed: 0,
        }
    }

    /// Compares two clock datasets according to ITU-T G.8275.1 Modified BMCA rules
    pub fn compare_telecom_bmca(&self, other: &Self) -> Ordering {
        // 1. Compare clockClass (lower is better)
        if self.clock_class != other.clock_class {
            return self.clock_class.cmp(&other.clock_class);
        }

        // 2. Compare clockAccuracy (lower is better)
        if self.clock_accuracy != other.clock_accuracy {
            return self.clock_accuracy.cmp(&other.clock_accuracy);
        }

        // 3. Compare offsetScaledLogVariance (lower is better)
        if self.offset_scaled_log_variance != other.offset_scaled_log_variance {
            return self
                .offset_scaled_log_variance
                .cmp(&other.offset_scaled_log_variance);
        }

        // 4. Compare priority2 (lower is better)
        if self.priority2 != other.priority2 {
            return self.priority2.cmp(&other.priority2);
        }

        // 5. Compare localPriority (lower is better, Telecom Profile specific)
        if self.local_priority != other.local_priority {
            return self.local_priority.cmp(&other.local_priority);
        }

        // 6. Compare stepsRemoved (lower is better)
        if self.steps_removed != other.steps_removed {
            return self.steps_removed.cmp(&other.steps_removed);
        }

        // 7. Tie-breaker: clockIdentity
        self.clock_identity.cmp(&other.clock_identity)
    }
}

/// Telecom Profile State Engine (T-GM / T-BC / T-TSC)
#[derive(Debug, Clone)]
pub struct TelecomProfileEngine {
    pub clock_type: TelecomClockType,
    pub own_attributes: TelecomBmcaAttributes,
    pub best_master: Option<TelecomBmcaAttributes>,
    pub phase_offset_ns: i64,
}

impl TelecomProfileEngine {
    pub fn new(clock_type: TelecomClockType, own_attributes: TelecomBmcaAttributes) -> Self {
        TelecomProfileEngine {
            clock_type,
            own_attributes,
            best_master: None,
            phase_offset_ns: 0,
        }
    }

    /// Processes an incoming Announce message dataset and updates BMCA Master selection
    pub fn process_announce(&mut self, master_candidate: TelecomBmcaAttributes) -> bool {
        let is_better = match &self.best_master {
            Some(current_best) => {
                master_candidate.compare_telecom_bmca(current_best) == Ordering::Less
            }
            None => master_candidate.compare_telecom_bmca(&self.own_attributes) == Ordering::Less,
        };

        if is_better {
            self.best_master = Some(master_candidate);
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
    fn test_telecom_bmca_grandmaster_selection() {
        let gm1 = TelecomBmcaAttributes {
            clock_class: 6, // PRTC locked
            clock_accuracy: 0x20,
            offset_scaled_log_variance: 0x4000,
            priority1: 128,
            priority2: 128,
            local_priority: 100,
            clock_identity: [1, 2, 3, 4, 5, 6, 7, 1],
            steps_removed: 0,
        };

        let gm2 = TelecomBmcaAttributes {
            clock_class: 7, // Holdover mode (degraded)
            clock_accuracy: 0x21,
            offset_scaled_log_variance: 0x4800,
            priority1: 128,
            priority2: 128,
            local_priority: 100,
            clock_identity: [1, 2, 3, 4, 5, 6, 7, 2],
            steps_removed: 0,
        };

        assert_eq!(gm1.compare_telecom_bmca(&gm2), Ordering::Less); // gm1 wins
    }

    #[test]
    fn test_telecom_profile_engine_state() {
        let slave_attr = TelecomBmcaAttributes::new_slave_clock([0xAA; 8]);
        let mut engine =
            TelecomProfileEngine::new(TelecomClockType::TelecomTimeSlaveClock, slave_attr);

        let gm = TelecomBmcaAttributes::new_prtc_grandmaster([0x11; 8]);
        let updated = engine.process_announce(gm.clone());

        assert!(updated);
        assert_eq!(engine.best_master, Some(gm));
    }
}
