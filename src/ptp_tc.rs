//! PTP Transparent Clock (IEEE 1588v2 & ITU-T G.8275.1 Residence Time Correction).
//!
//! Implements End-to-End (E2E) and Peer-to-Peer (P2P) Transparent Clocks (TC),
//! updating the 64-bit PTP Correction Field with switch residence time and link delays.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransparentClockMode {
    EndToEnd,   // E2E TC (Residence Time only)
    PeerToPeer, // P2P TC (Residence Time + Peer Link Delay)
}

/// Hop Timestamping Measurement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HopMeasurement {
    pub ingress_timestamp_ns: u64,
    pub egress_timestamp_ns: u64,
}

/// Transparent Clock Engine
#[derive(Debug, Clone)]
pub struct TransparentClockEngine {
    pub mode: TransparentClockMode,
    pub peer_delay_ns: u64,
    pub total_residence_time_ns: u64,
    pub corrected_packets_count: u64,
}

impl TransparentClockEngine {
    pub fn new(mode: TransparentClockMode) -> Self {
        TransparentClockEngine {
            mode,
            peer_delay_ns: 0,
            total_residence_time_ns: 0,
            corrected_packets_count: 0,
        }
    }

    /// Calculates node transit residence time: T_residence = T_egress - T_ingress
    pub fn calculate_residence_time(&self, hop: &HopMeasurement) -> u64 {
        if hop.egress_timestamp_ns >= hop.ingress_timestamp_ns {
            hop.egress_timestamp_ns - hop.ingress_timestamp_ns
        } else {
            0
        }
    }

    /// Calculates Peer Link Delay: PDelay = ((t4 - t1) - (t3 - t2)) / 2
    pub fn calculate_peer_delay(&mut self, t1_ns: u64, t2_ns: u64, t3_ns: u64, t4_ns: u64) -> u64 {
        let round_trip = t4_ns.saturating_sub(t1_ns);
        let peer_turnaround = t3_ns.saturating_sub(t2_ns);
        let link_delay = round_trip.saturating_sub(peer_turnaround) / 2;
        self.peer_delay_ns = link_delay;
        link_delay
    }

    /// Updates the incoming PTP frame Correction Field in nanoseconds
    pub fn update_correction_field(&mut self, initial_correction_ns: u64, hop: &HopMeasurement) -> u64 {
        let residence = self.calculate_residence_time(hop);
        self.total_residence_time_ns += residence;
        self.corrected_packets_count += 1;

        let additional_delay = match self.mode {
            TransparentClockMode::EndToEnd => residence,
            TransparentClockMode::PeerToPeer => residence + self.peer_delay_ns,
        };

        initial_correction_ns + additional_delay
    }

    /// Encodes nanoseconds to IEEE 1588v2 scaledNanoseconds (48-bit integer ns + 16-bit fractional ns)
    pub fn to_scaled_nanoseconds(ns: u64) -> u64 {
        ns << 16
    }

    /// Decodes IEEE 1588v2 scaledNanoseconds to integer nanoseconds
    pub fn from_scaled_nanoseconds(scaled: u64) -> u64 {
        scaled >> 16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_e2e_transparent_clock_residence_correction() {
        let mut tc = TransparentClockEngine::new(TransparentClockMode::EndToEnd);
        let hop = HopMeasurement {
            ingress_timestamp_ns: 1_000_000_000,
            egress_timestamp_ns: 1_000_000_350, // 350ns residence time
        };

        assert_eq!(tc.calculate_residence_time(&hop), 350);

        let new_corr = tc.update_correction_field(100, &hop);
        assert_eq!(new_corr, 450); // 100 + 350 = 450ns
        assert_eq!(tc.corrected_packets_count, 1);
    }

    #[test]
    fn test_p2p_transparent_clock_peer_delay_correction() {
        let mut tc = TransparentClockEngine::new(TransparentClockMode::PeerToPeer);
        // t1 = 0, t2 = 100, t3 = 150, t4 = 250 -> RoundTrip=250, Turnaround=50 -> PDelay=100ns
        let pdelay = tc.calculate_peer_delay(0, 100, 150, 250);
        assert_eq!(pdelay, 100);

        let hop = HopMeasurement {
            ingress_timestamp_ns: 10_000,
            egress_timestamp_ns: 10_200, // 200ns residence
        };

        let new_corr = tc.update_correction_field(50, &hop);
        assert_eq!(new_corr, 50 + 200 + 100); // 350ns
    }

    #[test]
    fn test_scaled_nanoseconds_conversions() {
        let ns = 12345;
        let scaled = TransparentClockEngine::to_scaled_nanoseconds(ns);
        assert_eq!(scaled, 12345 << 16);
        assert_eq!(TransparentClockEngine::from_scaled_nanoseconds(scaled), ns);
    }
}
