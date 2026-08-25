//! PTP Telecom Profile Peer-to-Peer Transparent Clock (T-TC) Engine (ITU-T G.8275.2 / IEEE 1588 Section 11.4).
//!
//! Implements P2P Peer Delay calculation (Pdelay_Req/Pdelay_Resp timestamps t1, t2, t3, t4),
//! ingress-to-egress residence time computation, and cumulative sub-nanosecond correctionField
//! adjustments across telecom fronthaul and packet networks.

use std::collections::HashMap;

/// PTP Peer-to-Peer Transparent Clock (P2P T-TC) Engine.
#[derive(Debug, Clone, Default)]
pub struct TelecomPeerTransparentClockEngine {
    pub peer_delays_ns: HashMap<u32, i64>, // Port ID -> Link Peer Mean Delay in nanoseconds
    pub corrections_performed: usize,
    pub accumulated_correction_ns: i64,
}

impl TelecomPeerTransparentClockEngine {
    pub fn new() -> Self {
        TelecomPeerTransparentClockEngine {
            peer_delays_ns: HashMap::new(),
            corrections_performed: 0,
            accumulated_correction_ns: 0,
        }
    }

    /// Computes the peer mean path delay using IEEE 1588 P2P formula:
    /// Delay = ((t4 - t1) - (t3 - t2)) / 2
    pub fn compute_peer_delay(&self, t1_ns: i64, t2_ns: i64, t3_ns: i64, t4_ns: i64) -> i64 {
        let round_trip = t4_ns - t1_ns;
        let peer_turnaround = t3_ns - t2_ns;
        (round_trip - peer_turnaround) / 2
    }

    /// Updates the measured peer delay for a specific port.
    pub fn set_port_peer_delay(&mut self, port_id: u32, delay_ns: i64) {
        self.peer_delays_ns.insert(port_id, delay_ns);
    }

    /// Computes and applies residence time + ingress link peer delay correction to a PTP packet.
    ///
    /// Correction = OldCorrection + ResidenceTime + IngressPeerDelay
    pub fn correct_event_packet(
        &mut self,
        ingress_port: u32,
        ingress_time_ns: i64,
        egress_time_ns: i64,
        initial_correction_ns: i64,
    ) -> i64 {
        self.corrections_performed += 1;
        let residence_time = egress_time_ns.saturating_sub(ingress_time_ns);
        let peer_delay = self.peer_delays_ns.get(&ingress_port).copied().unwrap_or(0);

        let delta = residence_time + peer_delay;
        self.accumulated_correction_ns += delta;
        initial_correction_ns + delta
    }
}
