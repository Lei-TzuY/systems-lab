//! NetFlow v9 & IPFIX Flow Telemetry (RFC 3954 / RFC 7011).
//!
//! Network traffic flow accounting, export, and telemetry collection over UDP ports 2055 and 4739.

use crate::ipv4::Ipv4Address;
use std::collections::HashMap;
use std::fmt;

pub const NETFLOW_V9_UDP_PORT: u16 = 2055;
pub const IPFIX_UDP_PORT: u16 = 4739;
pub const NETFLOW_V9_HEADER_LEN: usize = 20;

// NetFlow v9 Standard Field Types
pub const NF9_FIELD_IN_BYTES: u16 = 1;
pub const NF9_FIELD_IN_PKTS: u16 = 2;
pub const NF9_FIELD_PROTOCOL: u16 = 4;
pub const NF9_FIELD_TCP_FLAGS: u16 = 6;
pub const NF9_FIELD_L4_SRC_PORT: u16 = 7;
pub const NF9_FIELD_IPV4_SRC_ADDR: u16 = 8;
pub const NF9_FIELD_L4_DST_PORT: u16 = 11;
pub const NF9_FIELD_IPV4_DST_ADDR: u16 = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetflowHeader {
    pub version: u16,
    pub count: u16,
    pub sys_uptime_ms: u32,
    pub unix_secs: u32,
    pub sequence_number: u32,
    pub source_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetflowRecord {
    pub src_ip: Ipv4Address,
    pub dst_ip: Ipv4Address,
    pub src_port: u16,
    pub dst_port: u16,
    pub protocol: u8,
    pub packets: u32,
    pub bytes: u32,
    pub tcp_flags: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetflowPacket {
    pub header: NetflowHeader,
    pub template_id: u16,
    pub records: Vec<NetflowRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetflowError {
    PacketTooShort(usize),
    InvalidVersion(u16),
    LengthMismatch,
}

impl fmt::Display for NetflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetflowError::PacketTooShort(l) => {
                write!(f, "NetFlow packet too short ({} bytes, min 20)", l)
            }
            NetflowError::InvalidVersion(v) => {
                write!(f, "Invalid NetFlow version: expected 9, found {}", v)
            }
            NetflowError::LengthMismatch => write!(f, "NetFlow FlowSet length mismatch"),
        }
    }
}

impl std::error::Error for NetflowError {}

impl NetflowPacket {
    pub fn parse(data: &[u8]) -> Result<Self, NetflowError> {
        if data.len() < NETFLOW_V9_HEADER_LEN {
            return Err(NetflowError::PacketTooShort(data.len()));
        }

        let version = u16::from_be_bytes([data[0], data[1]]);
        if version != 9 {
            return Err(NetflowError::InvalidVersion(version));
        }

        let count = u16::from_be_bytes([data[2], data[3]]);
        let sys_uptime_ms = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let unix_secs = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let sequence_number = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
        let source_id = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);

        let header = NetflowHeader {
            version,
            count,
            sys_uptime_ms,
            unix_secs,
            sequence_number,
            source_id,
        };

        let mut records = Vec::new();
        let mut offset = NETFLOW_V9_HEADER_LEN;
        let mut template_id = 256;

        while offset + 4 <= data.len() {
            let flowset_id = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let flowset_len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;

            if offset + flowset_len > data.len() || flowset_len < 4 {
                break;
            }

            if flowset_id == 0 {
                // Template FlowSet
                if flowset_len >= 8 {
                    template_id = u16::from_be_bytes([data[offset + 4], data[offset + 5]]);
                }
            } else if flowset_id >= 256 {
                // Data FlowSet: Each record is 22 bytes in our standard 8-field template
                // (src_ip 4 + dst_ip 4 + src_p 2 + dst_p 2 + proto 1 + flags 1 + pkts 4 + bytes 4 = 22B)
                let rec_size = 22;
                let mut rec_offset = offset + 4;
                while rec_offset + rec_size <= offset + flowset_len {
                    let src_ip = Ipv4Address([
                        data[rec_offset],
                        data[rec_offset + 1],
                        data[rec_offset + 2],
                        data[rec_offset + 3],
                    ]);
                    let dst_ip = Ipv4Address([
                        data[rec_offset + 4],
                        data[rec_offset + 5],
                        data[rec_offset + 6],
                        data[rec_offset + 7],
                    ]);
                    let src_port = u16::from_be_bytes([data[rec_offset + 8], data[rec_offset + 9]]);
                    let dst_port =
                        u16::from_be_bytes([data[rec_offset + 10], data[rec_offset + 11]]);
                    let protocol = data[rec_offset + 12];
                    let tcp_flags = data[rec_offset + 13];
                    let packets = u32::from_be_bytes([
                        data[rec_offset + 14],
                        data[rec_offset + 15],
                        data[rec_offset + 16],
                        data[rec_offset + 17],
                    ]);
                    let bytes = u32::from_be_bytes([
                        data[rec_offset + 18],
                        data[rec_offset + 19],
                        data[rec_offset + 20],
                        data[rec_offset + 21],
                    ]);

                    records.push(NetflowRecord {
                        src_ip,
                        dst_ip,
                        src_port,
                        dst_port,
                        protocol,
                        packets,
                        bytes,
                        tcp_flags,
                    });
                    rec_offset += rec_size;
                }
            }
            offset += flowset_len;
        }

        Ok(NetflowPacket {
            header,
            template_id,
            records,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // 1. Header (20 bytes)
        buf.extend_from_slice(&self.header.version.to_be_bytes());
        buf.extend_from_slice(&((self.records.len() as u16) + 1).to_be_bytes()); // Count (Template + Records)
        buf.extend_from_slice(&self.header.sys_uptime_ms.to_be_bytes());
        buf.extend_from_slice(&self.header.unix_secs.to_be_bytes());
        buf.extend_from_slice(&self.header.sequence_number.to_be_bytes());
        buf.extend_from_slice(&self.header.source_id.to_be_bytes());

        // 2. Template FlowSet (ID = 0, 8 fields = 4 + 4 + 8*4 = 40 bytes)
        let template_flowset_len: u16 = 40;
        buf.extend_from_slice(&0u16.to_be_bytes()); // FlowSet ID = 0
        buf.extend_from_slice(&template_flowset_len.to_be_bytes());
        buf.extend_from_slice(&self.template_id.to_be_bytes());
        buf.extend_from_slice(&8u16.to_be_bytes()); // Field Count = 8

        let fields: [(u16, u16); 8] = [
            (NF9_FIELD_IPV4_SRC_ADDR, 4),
            (NF9_FIELD_IPV4_DST_ADDR, 4),
            (NF9_FIELD_L4_SRC_PORT, 2),
            (NF9_FIELD_L4_DST_PORT, 2),
            (NF9_FIELD_PROTOCOL, 1),
            (NF9_FIELD_TCP_FLAGS, 1),
            (NF9_FIELD_IN_PKTS, 4),
            (NF9_FIELD_IN_BYTES, 4),
        ];
        for (f_type, f_len) in fields {
            buf.extend_from_slice(&f_type.to_be_bytes());
            buf.extend_from_slice(&f_len.to_be_bytes());
        }

        // 3. Data FlowSet (ID = template_id)
        let data_rec_len = self.records.len() * 22;
        let data_flowset_len = (4 + data_rec_len) as u16;
        buf.extend_from_slice(&self.template_id.to_be_bytes());
        buf.extend_from_slice(&data_flowset_len.to_be_bytes());

        for r in &self.records {
            buf.extend_from_slice(&r.src_ip.0);
            buf.extend_from_slice(&r.dst_ip.0);
            buf.extend_from_slice(&r.src_port.to_be_bytes());
            buf.extend_from_slice(&r.dst_port.to_be_bytes());
            buf.push(r.protocol);
            buf.push(r.tcp_flags);
            buf.extend_from_slice(&r.packets.to_be_bytes());
            buf.extend_from_slice(&r.bytes.to_be_bytes());
        }

        buf
    }

    pub fn build_export(seq: u32, records: Vec<NetflowRecord>) -> Self {
        NetflowPacket {
            header: NetflowHeader {
                version: 9,
                count: (records.len() as u16) + 1,
                sys_uptime_ms: 125000,
                unix_secs: 1700000000,
                sequence_number: seq,
                source_id: 1,
            },
            template_id: 256,
            records,
        }
    }
}

/// In-Memory NetFlow Flow Cache Table
#[derive(Debug, Default)]
pub struct NetflowFlowTable {
    // 5-Tuple Key -> (Packets, Bytes, Flags)
    pub flows: HashMap<(Ipv4Address, Ipv4Address, u16, u16, u8), (u32, u32, u8)>,
}

impl NetflowFlowTable {
    pub fn new() -> Self {
        let mut table = NetflowFlowTable::default();
        table.record_traffic(
            Ipv4Address::new(192, 168, 1, 100),
            Ipv4Address::new(192, 168, 1, 10),
            55000,
            80,
            6, // TCP
            15,
            9420,
            0x18, // PSH+ACK
        );
        table.record_traffic(
            Ipv4Address::new(192, 168, 1, 100),
            Ipv4Address::new(192, 168, 1, 10),
            53535,
            53,
            17, // UDP
            2,
            140,
            0x00,
        );
        table
    }

    pub fn record_traffic(
        &mut self,
        src_ip: Ipv4Address,
        dst_ip: Ipv4Address,
        src_p: u16,
        dst_p: u16,
        proto: u8,
        pkts: u32,
        bytes: u32,
        flags: u8,
    ) {
        let entry = self
            .flows
            .entry((src_ip, dst_ip, src_p, dst_p, proto))
            .or_insert((0, 0, 0));
        entry.0 += pkts;
        entry.1 += bytes;
        entry.2 |= flags;
    }

    pub fn export_records(&self) -> Vec<NetflowRecord> {
        let mut list = Vec::new();
        for (&(src_ip, dst_ip, src_port, dst_port, protocol), &(packets, bytes, tcp_flags)) in
            &self.flows
        {
            list.push(NetflowRecord {
                src_ip,
                dst_ip,
                src_port,
                dst_port,
                protocol,
                packets,
                bytes,
                tcp_flags,
            });
        }
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_netflow_v9_packet_roundtrip() {
        let record = NetflowRecord {
            src_ip: Ipv4Address::new(10, 0, 0, 1),
            dst_ip: Ipv4Address::new(10, 0, 0, 2),
            src_port: 12345,
            dst_port: 80,
            protocol: 6,
            packets: 42,
            bytes: 5600,
            tcp_flags: 0x18,
        };

        let pkt = NetflowPacket::build_export(1, vec![record.clone()]);
        let raw = pkt.serialize();

        assert!(raw.len() >= NETFLOW_V9_HEADER_LEN);
        let parsed = NetflowPacket::parse(&raw).unwrap();

        assert_eq!(parsed.header.version, 9);
        assert_eq!(parsed.records.len(), 1);
        assert_eq!(parsed.records[0], record);
    }
}
