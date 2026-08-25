//! PTP Telecom Profile Boundary Clock (T-BC - ITU-T G.8275.1 / G.8273.2).
//!
//! Implements the ITU-T G.8275.1 Alternate Best Master Clock Algorithm (BMCA),
//! local priority arbitration (localPriority 1..255), Steps-Removed override,
//! multi-port Boundary Clock state machine (Master, Slave, Passive), and phase error
//! accumulation filtering for 5G fronthaul and packet-based phase/time synchronization.

use std::collections::HashMap;

/// Telecom Profile Clock Port Role / State.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TelecomPortState {
    Master,
    Slave,
    Passive,
    Disabled,
}

/// G.8275.1 Telecom Profile Clock Quality Attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TelecomClockQuality {
    pub clock_class: u8, // 6 = PRTC (Primary Reference Time Clock), 7, 135, 140, 248
    pub clock_accuracy: u8, // 0x20 = Within 25ns, 0x21 = Within 100ns
    pub offset_scaled_log_variance: u16,
}

/// Port configuration and received grandmaster Announce dataset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelecomPortConfig {
    pub port_id: u32,
    pub local_priority: u8, // 1..255 (lower = preferred in G.8275.1)
    pub not_slave: bool,    // If true, port can only be Master or Passive
    pub rx_clock_quality: Option<TelecomClockQuality>,
    pub rx_steps_removed: u16,
    pub rx_grandmaster_priority: u8,
}

/// Telecom Boundary Clock (T-BC) Protocol Engine (ITU-T G.8275.1 / G.8273.2).
#[derive(Debug, Clone, Default)]
pub struct TelecomBoundaryClockEngine {
    pub ports: HashMap<u32, TelecomPortConfig>,
    pub port_states: HashMap<u32, TelecomPortState>,
    pub slave_port: Option<u32>,
    pub accumulated_phase_offset_ns: i64,
    pub bmca_cycles_run: usize,
}

impl TelecomBoundaryClockEngine {
    pub fn new() -> Self {
        TelecomBoundaryClockEngine {
            ports: HashMap::new(),
            port_states: HashMap::new(),
            slave_port: None,
            accumulated_phase_offset_ns: 0,
            bmca_cycles_run: 0,
        }
    }

    /// Configures a physical T-BC port.
    pub fn add_port(&mut self, port_id: u32, local_priority: u8, not_slave: bool) {
        self.ports.insert(
            port_id,
            TelecomPortConfig {
                port_id,
                local_priority,
                not_slave,
                rx_clock_quality: None,
                rx_steps_removed: 0,
                rx_grandmaster_priority: 128,
            },
        );
        self.port_states.insert(port_id, TelecomPortState::Master);
    }

    /// Ingests received Announce timing data on a port.
    pub fn update_rx_announce(
        &mut self,
        port_id: u32,
        quality: TelecomClockQuality,
        steps_removed: u16,
        gm_priority: u8,
    ) {
        if let Some(port) = self.ports.get_mut(&port_id) {
            port.rx_clock_quality = Some(quality);
            port.rx_steps_removed = steps_removed;
            port.rx_grandmaster_priority = gm_priority;
        }
    }

    /// Runs ITU-T G.8275.1 Alternate BMCA across all configured Boundary Clock ports.
    pub fn run_alternate_bmca(&mut self) -> Option<u32> {
        self.bmca_cycles_run += 1;

        // Candidate ports eligible to become Slave:
        // (port_id, clock_class, local_priority, steps_removed)
        let mut best: Option<(u32, u8, u8, u16)> = None;

        for port in self.ports.values() {
            if port.not_slave {
                continue;
            }
            let quality = match port.rx_clock_quality {
                Some(q) => q,
                None => continue,
            };

            // ITU-T G.8275.1 BMCA Comparison:
            // 1. Clock Class (lower = superior)
            // 2. Local Priority (lower = preferred)
            // 3. Steps Removed (lower = shorter distance to GM)
            let cand = (
                port.port_id,
                quality.clock_class,
                port.local_priority,
                port.rx_steps_removed,
            );

            match best {
                None => best = Some(cand),
                Some((_, b_class, b_prio, b_steps)) => {
                    if cand.1 < b_class
                        || (cand.1 == b_class && cand.2 < b_prio)
                        || (cand.1 == b_class && cand.2 == b_prio && cand.3 < b_steps)
                    {
                        best = Some(cand);
                    }
                }
            }
        }

        if let Some((best_slave_port, _, _, _)) = best {
            self.slave_port = Some(best_slave_port);
            for (pid, state) in self.port_states.iter_mut() {
                if *pid == best_slave_port {
                    *state = TelecomPortState::Slave;
                } else if self.ports.get(pid).map(|p| p.not_slave).unwrap_or(false) {
                    *state = TelecomPortState::Master;
                } else {
                    *state = TelecomPortState::Passive;
                }
            }
            Some(best_slave_port)
        } else {
            self.slave_port = None;
            for state in self.port_states.values_mut() {
                *state = TelecomPortState::Master;
            }
            None
        }
    }

    /// Filters and adjusts local phase offset error (in nanoseconds).
    pub fn adjust_phase_offset(&mut self, phase_error_ns: i64) -> i64 {
        // Apply damping filter
        let correction = phase_error_ns / 2;
        self.accumulated_phase_offset_ns += correction;
        correction
    }
}
