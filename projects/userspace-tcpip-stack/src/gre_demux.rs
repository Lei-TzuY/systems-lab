//! GRE RFC 2890 Key Demultiplexing, VRF Binding & Anti-Replay Engine.
//!
//! Provides multi-tenant virtual tunnel isolation and sequence-based replay protection for Generic Routing Encapsulation (GRE).

use crate::ipv4::Ipv4Address;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GreVirtualTunnel {
    pub if_name: String,
    pub vrf_id: u32,
    pub local_ip: Ipv4Address,
    pub remote_ip: Ipv4Address,
    pub key: u32,
    pub strict_sequence: bool,
}

#[derive(Debug, Clone, Default)]
pub struct GreSessionTracker {
    pub highest_seq: u32,
    pub packets_received: u64,
    pub packets_dropped: u64,
}

impl GreSessionTracker {
    pub fn validate_and_update(&mut self, seq: Option<u32>, strict: bool) -> bool {
        self.packets_received += 1;

        if let Some(s) = seq {
            if strict && s <= self.highest_seq && self.highest_seq != 0 {
                self.packets_dropped += 1;
                return false; // Out-of-order or duplicate replay packet dropped
            }
            if s > self.highest_seq {
                self.highest_seq = s;
            }
        }
        true
    }
}

#[derive(Debug, Clone, Default)]
pub struct GreDemuxTable {
    pub tunnels: HashMap<(Ipv4Address, u32), (GreVirtualTunnel, GreSessionTracker)>,
}

impl GreDemuxTable {
    pub fn new() -> Self {
        GreDemuxTable {
            tunnels: HashMap::new(),
        }
    }

    pub fn register_tunnel(&mut self, tunnel: GreVirtualTunnel) {
        let key = (tunnel.remote_ip, tunnel.key);
        self.tunnels
            .insert(key, (tunnel, GreSessionTracker::default()));
    }

    pub fn demux_packet(
        &mut self,
        remote_ip: Ipv4Address,
        key: Option<u32>,
        seq: Option<u32>,
        payload: &[u8],
    ) -> Option<(String, u32, Vec<u8>)> {
        let k = key.unwrap_or(0);
        let lookup_key = (remote_ip, k);

        if let Some((tunnel, tracker)) = self.tunnels.get_mut(&lookup_key) {
            let valid = tracker.validate_and_update(seq, tunnel.strict_sequence);
            if valid {
                return Some((tunnel.if_name.clone(), tunnel.vrf_id, payload.to_vec()));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gre_demux_key_routing_and_anti_replay() {
        let mut demux = GreDemuxTable::new();

        let peer_ip = Ipv4Address::new(198, 51, 100, 1);

        // Tenant A: Key 1001 -> gre1 (VRF 10)
        demux.register_tunnel(GreVirtualTunnel {
            if_name: "gre1".to_string(),
            vrf_id: 10,
            local_ip: Ipv4Address::new(192, 168, 1, 1),
            remote_ip: peer_ip,
            key: 1001,
            strict_sequence: true,
        });

        // Tenant B: Key 1002 -> gre2 (VRF 20)
        demux.register_tunnel(GreVirtualTunnel {
            if_name: "gre2".to_string(),
            vrf_id: 20,
            local_ip: Ipv4Address::new(192, 168, 1, 1),
            remote_ip: peer_ip,
            key: 1002,
            strict_sequence: false,
        });

        // Packet 1 for Tenant A (Seq 1) -> Passed to gre1
        let res1 = demux.demux_packet(peer_ip, Some(1001), Some(1), b"Tenant A Payload 1");
        assert!(res1.is_some());
        let (iface, vrf, data) = res1.unwrap();
        assert_eq!(iface, "gre1");
        assert_eq!(vrf, 10);
        assert_eq!(data, b"Tenant A Payload 1");

        // Packet 2 for Tenant A (Duplicate Seq 1 replay attack) -> DROPPED
        let res2 = demux.demux_packet(peer_ip, Some(1001), Some(1), b"Tenant A Replay Payload");
        assert_eq!(res2, None);

        // Packet 3 for Tenant B (Key 1002) -> Passed to gre2
        let res3 = demux.demux_packet(peer_ip, Some(1002), Some(1), b"Tenant B Payload 1");
        assert!(res3.is_some());
        let (iface_b, vrf_b, _) = res3.unwrap();
        assert_eq!(iface_b, "gre2");
        assert_eq!(vrf_b, 20);
    }
}
