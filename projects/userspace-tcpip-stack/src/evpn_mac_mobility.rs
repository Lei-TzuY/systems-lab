//! EVPN MAC Mobility (RFC 7432 Section 15 / RFC 7024).
//!
//! When a host moves from one PE to another in a BGP EVPN fabric, the new
//! PE must advertise the MAC with an incremented **MAC Mobility Extended
//! Community** sequence number so that all remote PEs converge to the new
//! location and withdraw the stale entry.
//!
//! This module implements:
//! * MAC Mobility Extended Community codec (Type 0x06, Sub-Type 0x00).
//! * Per-MAC sequence number tracking with monotonic increment.
//! * **Sticky MAC** (static flag) — a MAC that should never move.
//! * Duplicate-detection: configurable move-count threshold within a
//!   time window to suppress MAC flapping.

/// MAC Mobility Extended Community (RFC 7432 Section 7.7).
///
/// ```text
///  0                   1                   2                   3
///  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |  Type (0x06)  |Sub-Type(0x00) |  Flags  |   Reserved          |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                     Sequence Number                           |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// ```
///
/// Flags bit 0 = Sticky/Static flag.

pub const EXT_COMM_TYPE_MAC_MOBILITY: u8 = 0x06;
pub const EXT_COMM_SUBTYPE_MAC_MOBILITY: u8 = 0x00;

/// Parsed MAC Mobility Extended Community.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacMobilityExtComm {
    pub sticky: bool,
    pub sequence_number: u32,
}

impl MacMobilityExtComm {
    /// Serializes to the 8-byte BGP Extended Community wire format.
    pub fn serialize(&self) -> [u8; 8] {
        let mut buf = [0u8; 8];
        buf[0] = EXT_COMM_TYPE_MAC_MOBILITY;
        buf[1] = EXT_COMM_SUBTYPE_MAC_MOBILITY;
        if self.sticky {
            buf[2] = 0x01;
        }
        // buf[3] reserved
        let seq_bytes = self.sequence_number.to_be_bytes();
        buf[4..8].copy_from_slice(&seq_bytes);
        buf
    }

    /// Parses from the 8-byte BGP Extended Community wire format.
    pub fn parse(data: &[u8; 8]) -> Option<Self> {
        if data[0] != EXT_COMM_TYPE_MAC_MOBILITY || data[1] != EXT_COMM_SUBTYPE_MAC_MOBILITY {
            return None;
        }
        let sticky = (data[2] & 0x01) != 0;
        let sequence_number = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        Some(MacMobilityExtComm {
            sticky,
            sequence_number,
        })
    }
}

// ── Per-MAC entry ────────────────────────────────────────────────────────

/// Represents one MAC address entry in the EVPN MAC/IP table with mobility
/// tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacEntry {
    pub mac: [u8; 6],
    pub vtep_ip: [u8; 4],
    pub vni: u32,
    pub sequence_number: u32,
    pub sticky: bool,
    /// Number of moves detected within the current detection window.
    pub move_count: u32,
    /// Whether this MAC has been flagged as duplicate (flapping).
    pub duplicate_detected: bool,
}

// ── MAC Mobility Engine ──────────────────────────────────────────────────

/// EVPN MAC Mobility engine managing per-VNI MAC tables.
#[derive(Debug, Clone)]
pub struct EvpnMacMobilityEngine {
    pub entries: Vec<MacEntry>,
    /// Maximum moves within a detection window before duplicate flagging.
    pub move_threshold: u32,
}

impl EvpnMacMobilityEngine {
    pub fn new(move_threshold: u32) -> Self {
        EvpnMacMobilityEngine {
            entries: Vec::new(),
            move_threshold,
        }
    }

    /// Looks up an existing entry by (VNI, MAC).
    fn find_entry(&self, vni: u32, mac: &[u8; 6]) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.vni == vni && e.mac == *mac)
    }

    /// Learns or updates a MAC. Returns the MAC Mobility Extended Community
    /// that should be attached to the BGP UPDATE, and whether the MAC moved.
    ///
    /// Semantics per RFC 7432 Section 15:
    /// - If MAC is new → install with seq 0, no move.
    /// - If MAC exists at same VTEP → no-op (refresh).
    /// - If MAC exists at different VTEP → move detected:
    ///   - If remote entry is sticky and we are not → reject (sticky wins).
    ///   - Otherwise, increment seq and update.
    pub fn learn_mac(
        &mut self,
        vni: u32,
        mac: [u8; 6],
        vtep_ip: [u8; 4],
        sticky: bool,
    ) -> (MacMobilityExtComm, bool /* moved */) {
        if let Some(idx) = self.find_entry(vni, &mac) {
            let existing = &self.entries[idx];

            if existing.vtep_ip == vtep_ip {
                // Same location — no move, return current seq.
                return (
                    MacMobilityExtComm {
                        sticky: existing.sticky,
                        sequence_number: existing.sequence_number,
                    },
                    false,
                );
            }

            // Sticky remote MAC cannot be overridden by non-sticky local.
            if existing.sticky && !sticky {
                return (
                    MacMobilityExtComm {
                        sticky: existing.sticky,
                        sequence_number: existing.sequence_number,
                    },
                    false,
                );
            }

            // MAC has moved.
            let new_seq = existing.sequence_number + 1;
            let new_move_count = existing.move_count + 1;
            let dup = new_move_count >= self.move_threshold;

            self.entries[idx] = MacEntry {
                mac,
                vtep_ip,
                vni,
                sequence_number: new_seq,
                sticky,
                move_count: new_move_count,
                duplicate_detected: dup,
            };

            (
                MacMobilityExtComm {
                    sticky,
                    sequence_number: new_seq,
                },
                true,
            )
        } else {
            // New MAC — first seen.
            self.entries.push(MacEntry {
                mac,
                vtep_ip,
                vni,
                sequence_number: 0,
                sticky,
                move_count: 0,
                duplicate_detected: false,
            });

            (
                MacMobilityExtComm {
                    sticky,
                    sequence_number: 0,
                },
                false,
            )
        }
    }

    /// Processes an incoming remote BGP EVPN Route Type 2 advertisement
    /// carrying a MAC Mobility Extended Community.  Returns `true` if the
    /// local table was updated (remote wins).
    pub fn process_remote_advertisement(
        &mut self,
        vni: u32,
        mac: [u8; 6],
        vtep_ip: [u8; 4],
        remote_comm: &MacMobilityExtComm,
    ) -> bool {
        if let Some(idx) = self.find_entry(vni, &mac) {
            let existing = &self.entries[idx];

            // Higher sequence wins.
            if remote_comm.sequence_number > existing.sequence_number {
                let new_move_count = existing.move_count + 1;
                let dup = new_move_count >= self.move_threshold;
                self.entries[idx] = MacEntry {
                    mac,
                    vtep_ip,
                    vni,
                    sequence_number: remote_comm.sequence_number,
                    sticky: remote_comm.sticky,
                    move_count: new_move_count,
                    duplicate_detected: dup,
                };
                return true;
            }
            // Equal sequence, sticky wins.
            if remote_comm.sequence_number == existing.sequence_number
                && remote_comm.sticky
                && !existing.sticky
            {
                self.entries[idx] = MacEntry {
                    mac,
                    vtep_ip,
                    vni,
                    sequence_number: remote_comm.sequence_number,
                    sticky: true,
                    move_count: existing.move_count + 1,
                    duplicate_detected: existing.move_count + 1 >= self.move_threshold,
                };
                return true;
            }
            false
        } else {
            self.entries.push(MacEntry {
                mac,
                vtep_ip,
                vni,
                sequence_number: remote_comm.sequence_number,
                sticky: remote_comm.sticky,
                move_count: 0,
                duplicate_detected: false,
            });
            true
        }
    }

    /// Returns the count of duplicate-flagged MACs across all VNIs.
    pub fn duplicate_count(&self) -> usize {
        self.entries.iter().filter(|e| e.duplicate_detected).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mac_mobility_ext_comm_codec() {
        let comm = MacMobilityExtComm {
            sticky: true,
            sequence_number: 42,
        };
        let wire = comm.serialize();
        assert_eq!(wire[0], EXT_COMM_TYPE_MAC_MOBILITY);
        assert_eq!(wire[1], EXT_COMM_SUBTYPE_MAC_MOBILITY);
        assert_eq!(wire[2] & 0x01, 1); // sticky flag

        let parsed = MacMobilityExtComm::parse(&wire).unwrap();
        assert_eq!(parsed, comm);
    }

    #[test]
    fn test_mac_move_increments_seq() {
        let mut engine = EvpnMacMobilityEngine::new(5);
        let mac = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
        let vtep1 = [10, 0, 0, 1];
        let vtep2 = [10, 0, 0, 2];

        // First learn
        let (comm, moved) = engine.learn_mac(1000, mac, vtep1, false);
        assert!(!moved);
        assert_eq!(comm.sequence_number, 0);

        // Move to vtep2
        let (comm, moved) = engine.learn_mac(1000, mac, vtep2, false);
        assert!(moved);
        assert_eq!(comm.sequence_number, 1);

        // Move back to vtep1
        let (comm, moved) = engine.learn_mac(1000, mac, vtep1, false);
        assert!(moved);
        assert_eq!(comm.sequence_number, 2);
    }

    #[test]
    fn test_sticky_mac_wins() {
        let mut engine = EvpnMacMobilityEngine::new(5);
        let mac = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let vtep1 = [10, 0, 0, 1];
        let vtep2 = [10, 0, 0, 2];

        // Learn as sticky on vtep1
        engine.learn_mac(2000, mac, vtep1, true);

        // Non-sticky attempt to move → rejected
        let (_, moved) = engine.learn_mac(2000, mac, vtep2, false);
        assert!(!moved);
        let entry = engine.entries.iter().find(|e| e.mac == mac).unwrap();
        assert_eq!(entry.vtep_ip, vtep1); // stayed at vtep1
    }

    #[test]
    fn test_duplicate_detection_threshold() {
        let mut engine = EvpnMacMobilityEngine::new(3);
        let mac = [0x00, 0x00, 0x00, 0x00, 0x00, 0x01];
        let vtep_a = [10, 0, 0, 1];
        let vtep_b = [10, 0, 0, 2];

        engine.learn_mac(100, mac, vtep_a, false);
        engine.learn_mac(100, mac, vtep_b, false); // move 1
        engine.learn_mac(100, mac, vtep_a, false); // move 2
        engine.learn_mac(100, mac, vtep_b, false); // move 3 → threshold

        assert_eq!(engine.duplicate_count(), 1);
    }

    #[test]
    fn test_remote_higher_seq_wins() {
        let mut engine = EvpnMacMobilityEngine::new(5);
        let mac = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01];
        let vtep_local = [10, 0, 0, 1];
        let vtep_remote = [10, 0, 0, 99];

        engine.learn_mac(500, mac, vtep_local, false); // seq 0

        let remote_comm = MacMobilityExtComm {
            sticky: false,
            sequence_number: 5,
        };
        let updated = engine.process_remote_advertisement(500, mac, vtep_remote, &remote_comm);
        assert!(updated);

        let entry = engine.entries.iter().find(|e| e.mac == mac).unwrap();
        assert_eq!(entry.vtep_ip, vtep_remote);
        assert_eq!(entry.sequence_number, 5);
    }
}
