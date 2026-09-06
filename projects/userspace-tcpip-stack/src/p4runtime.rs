//! P4Runtime SDN Data Plane Programming API (P4.org / ONF Standard Port 9559).
//!
//! Provides protocol-independent Match-Action Table programming, Packet-In/Packet-Out
//! streaming, and pipeline configuration for P4 programmable switches.

use std::collections::HashMap;

pub const P4RUNTIME_PORT: u16 = 9559;
pub const P4RUNTIME_VERSION: &str = "v1.3.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum P4MatchKind {
    Exact(Vec<u8>),
    Lpm { value: Vec<u8>, prefix_len: u32 },
    Ternary { value: Vec<u8>, mask: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P4MatchField {
    pub field_name: String,
    pub match_value: P4MatchKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P4TableEntry {
    pub table_name: String,
    pub matches: Vec<P4MatchField>,
    pub action_name: String,
    pub action_params: Vec<(String, Vec<u8>)>,
    pub priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P4PacketOut {
    pub egress_port: u32,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P4PacketIn {
    pub ingress_port: u32,
    pub payload: Vec<u8>,
}

/// P4Runtime Switch Server & Match-Action Pipeline Store
#[derive(Debug, Clone, Default)]
pub struct P4RuntimeServer {
    pub device_id: u64,
    pub election_id: u64,
    pub pipeline_loaded: bool,
    pub table_entries: HashMap<String, Vec<P4TableEntry>>,
    pub packet_in_count: u64,
    pub packet_out_count: u64,
}

impl P4RuntimeServer {
    pub fn new(device_id: u64) -> Self {
        P4RuntimeServer {
            device_id,
            election_id: 1,
            pipeline_loaded: false,
            table_entries: HashMap::new(),
            packet_in_count: 0,
            packet_out_count: 0,
        }
    }

    /// Loads P4 forward pipeline configuration (P4Info + Device binary)
    pub fn set_forwarding_pipeline_config(&mut self, _p4info_name: &str) -> bool {
        self.pipeline_loaded = true;
        true
    }

    /// Writes (inserts or updates) a match-action table entry
    pub fn write_table_entry(&mut self, entry: P4TableEntry) {
        let entries = self
            .table_entries
            .entry(entry.table_name.clone())
            .or_default();
        entries.retain(|e| e.matches != entry.matches);
        entries.push(entry);
    }

    /// Simulates controller packet transmission (Packet-Out) to switch datapath
    pub fn handle_packet_out(&mut self, pkt: P4PacketOut) -> usize {
        self.packet_out_count += 1;
        pkt.payload.len()
    }

    /// Simulates switch datapath sending punted packet (Packet-In) to controller
    pub fn emit_packet_in(&mut self, ingress_port: u32, payload: &[u8]) -> P4PacketIn {
        self.packet_in_count += 1;
        P4PacketIn {
            ingress_port,
            payload: payload.to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p4runtime_table_programming_and_packet_io() {
        let mut server = P4RuntimeServer::new(1);
        server.set_forwarding_pipeline_config("fabric.p4info.txt");
        assert!(server.pipeline_loaded);

        // Add Table Entry to Ingress IPv4 LPM table
        let entry = P4TableEntry {
            table_name: "IngressPipeImpl.ipv4_lpm".to_string(),
            matches: vec![P4MatchField {
                field_name: "hdr.ipv4.dst_addr".to_string(),
                match_value: P4MatchKind::Lpm {
                    value: vec![10, 0, 0, 0],
                    prefix_len: 16,
                },
            }],
            action_name: "IngressPipeImpl.set_next_hop".to_string(),
            action_params: vec![("port".to_string(), vec![0, 0, 0, 3])],
            priority: 10,
        };

        server.write_table_entry(entry);
        assert_eq!(server.table_entries["IngressPipeImpl.ipv4_lpm"].len(), 1);

        // Test Packet-Out
        let bytes_sent = server.handle_packet_out(P4PacketOut {
            egress_port: 3,
            payload: b"P4 Injected Probe".to_vec(),
        });
        assert_eq!(bytes_sent, 17);
        assert_eq!(server.packet_out_count, 1);

        // Test Packet-In
        let pkt_in = server.emit_packet_in(1, b"Punted ARP Frame");
        assert_eq!(pkt_in.ingress_port, 1);
        assert_eq!(server.packet_in_count, 1);
    }
}
