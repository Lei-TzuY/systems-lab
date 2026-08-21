//! IEEE 802.1Qcc TSN Centralized Network Configuration (CNC) & Stream Reservation.
//!
//! Implements Centralized Network Configuration (CNC), Centralized User Configuration (CUC),
//! Talker/Listener stream registration, Traffic Specification (TSpec), and latency bound computation.

use crate::ethernet::MacAddress;
use std::collections::HashMap;

/// 64-bit IEEE 802.1Qcc Stream ID (Source MAC Address + 16-bit Unique ID)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StreamId(pub [u8; 8]);

impl StreamId {
    pub fn new(mac: MacAddress, unique_id: u16) -> Self {
        let mut bytes = [0u8; 8];
        bytes[0..6].copy_from_slice(&mac.0);
        bytes[6..8].copy_from_slice(&unique_id.to_be_bytes());
        StreamId(bytes)
    }
}

/// TSN Traffic Specification (TSpec)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrafficSpecification {
    pub max_frame_size: u16,     // Maximum frame size in bytes
    pub max_interval_frames: u16, // Max frames per transmission interval
    pub interval_us: u32,        // Transmission interval in microseconds
}

/// User-to-Network Requirements
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserToNetworkRequirements {
    pub max_latency_us: u32,    // Maximum tolerated end-to-end latency
    pub num_seamless_trees: u8, // Redundant paths for FRER reliability (e.g. 1 or 2)
}

/// TSN Stream Talker Profile
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsnTalker {
    pub stream_id: StreamId,
    pub talker_mac: MacAddress,
    pub vlan_id: u16,
    pub priority: u8,
    pub tspec: TrafficSpecification,
}

/// TSN Stream Listener Profile
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsnListener {
    pub stream_id: StreamId,
    pub listener_mac: MacAddress,
    pub reqs: UserToNetworkRequirements,
}

/// Centralized Network Configuration (CNC) Engine
#[derive(Debug, Clone, Default)]
pub struct CentralizedNetworkConfigurator {
    pub talkers: HashMap<StreamId, TsnTalker>,
    pub listeners: HashMap<StreamId, Vec<TsnListener>>,
    pub total_reserved_bandwidth_bps: u64,
}

impl CentralizedNetworkConfigurator {
    pub fn new() -> Self {
        CentralizedNetworkConfigurator {
            talkers: HashMap::new(),
            listeners: HashMap::new(),
            total_reserved_bandwidth_bps: 0,
        }
    }

    /// Computes the required reserved bandwidth in bits per second for a given TSpec
    pub fn compute_stream_bandwidth(tspec: &TrafficSpecification) -> u64 {
        if tspec.interval_us == 0 {
            return 0;
        }
        let bits_per_interval = (tspec.max_frame_size as u64) * (tspec.max_interval_frames as u64) * 8;
        // bps = (bits / interval_us) * 1,000,000
        (bits_per_interval * 1_000_000) / (tspec.interval_us as u64)
    }

    /// Registers a TSN Talker stream with CNC
    pub fn register_talker(&mut self, talker: TsnTalker) -> Result<u64, &'static str> {
        let bw = Self::compute_stream_bandwidth(&talker.tspec);
        self.total_reserved_bandwidth_bps += bw;
        let sid = talker.stream_id;
        self.talkers.insert(sid, talker);
        Ok(bw)
    }

    /// Registers a TSN Listener subscribing to an existing Talker stream
    /// Returns the calculated worst-case bounded latency in microseconds
    pub fn register_listener(&mut self, listener: TsnListener) -> Result<u32, &'static str> {
        let talker = self.talkers.get(&listener.stream_id).ok_or("Talker stream not found")?;

        // Calculate bounded hop latency (approximate TSN deterministic delay calculation: 2 * interval + bridge queuing)
        let calculated_latency_us = (talker.tspec.interval_us * 2) + 20; // 2 cycles + 20µs switch transit

        if calculated_latency_us > listener.reqs.max_latency_us {
            return Err("Calculated TSN latency exceeds listener maximum tolerance");
        }

        let sid = listener.stream_id;
        self.listeners.entry(sid).or_default().push(listener);
        Ok(calculated_latency_us)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tsn_cnc_talker_listener_registration() {
        let mut cnc = CentralizedNetworkConfigurator::new();
        let talker_mac = MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let stream_id = StreamId::new(talker_mac, 1);

        let talker = TsnTalker {
            stream_id,
            talker_mac,
            vlan_id: 100,
            priority: 6,
            tspec: TrafficSpecification {
                max_frame_size: 500,     // 500 bytes
                max_interval_frames: 2, // 2 frames per interval
                interval_us: 1000,      // every 1000µs (1ms)
            },
        };

        // Bandwidth: (500 * 2 * 8) bits / 0.001s = 8,000,000 bps = 8 Mbps
        let bw = cnc.register_talker(talker).unwrap();
        assert_eq!(bw, 8_000_000);
        assert_eq!(cnc.total_reserved_bandwidth_bps, 8_000_000);

        let listener = TsnListener {
            stream_id,
            listener_mac: MacAddress([0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE]),
            reqs: UserToNetworkRequirements {
                max_latency_us: 5000, // 5ms tolerance
                num_seamless_trees: 1,
            },
        };

        let latency = cnc.register_listener(listener).unwrap();
        assert_eq!(latency, 2020); // 2000µs + 20µs
    }

    #[test]
    fn test_tsn_cnc_latency_violation() {
        let mut cnc = CentralizedNetworkConfigurator::new();
        let talker_mac = MacAddress([0x00, 0x01, 0x02, 0x03, 0x04, 0x05]);
        let stream_id = StreamId::new(talker_mac, 2);

        cnc.register_talker(TsnTalker {
            stream_id,
            talker_mac,
            vlan_id: 200,
            priority: 7,
            tspec: TrafficSpecification {
                max_frame_size: 1000,
                max_interval_frames: 1,
                interval_us: 5000, // 5ms interval -> ~10ms calculated latency
            },
        }).unwrap();

        let listener = TsnListener {
            stream_id,
            listener_mac: MacAddress([0x00, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E]),
            reqs: UserToNetworkRequirements {
                max_latency_us: 2000, // Demands 2ms, but stream needs >10ms
                num_seamless_trees: 1,
            },
        };

        assert!(cnc.register_listener(listener).is_err());
    }
}
