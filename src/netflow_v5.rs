//! Cisco NetFlow Version 5 (NetFlow v5 - Flow Export Telemetry).
//!
//! Provides the classic enterprise & datacenter 24-byte header and 48-byte fixed record flow exporter.

use crate::ipv4::Ipv4Address;
use std::collections::HashMap;

pub const NETFLOW_V5_UDP_PORT: u16 = 2055;
pub const NETFLOW_V5_HEADER_LEN: usize = 24;
pub const NETFLOW_V5_RECORD_LEN: usize = 48;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetflowV5Header {
    pub version: u16,        // Always 5
    pub count: u16,          // Number of flow records in packet (1..30)
    pub sys_uptime_ms: u32,  // Router uptime in ms
    pub unix_secs: u32,      // Current seconds since epoch
    pub unix_nsecs: u32,     // Residual nanoseconds
    pub flow_sequence: u32,  // Sequence counter of total flows exported
    pub engine_type: u8,
    pub engine_id: u8,
    pub sampling_interval: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetflowV5Record {
    pub src_addr: Ipv4Address,
    pub dst_addr: Ipv4Address,
    pub next_hop: Ipv4Address,
    pub input_ifindex: u16,
    pub output_ifindex: u16,
    pub packet_count: u32,
    pub octet_count: u32,
    pub first_uptime_ms: u32,
    pub last_uptime_ms: u32,
    pub src_port: u16,
    pub dst_port: u16,
    pub tcp_flags: u8,
    pub protocol: u8,
    pub tos: u8,
    pub src_as: u16,
    pub dst_as: u16,
    pub src_mask: u8,
    pub dst_mask: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetflowV5Packet {
    pub header: NetflowV5Header,
    pub records: Vec<NetflowV5Record>,
}

impl NetflowV5Packet {
    pub fn new(header: NetflowV5Header, records: Vec<NetflowV5Record>) -> Self {
        NetflowV5Packet { header, records }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(NETFLOW_V5_HEADER_LEN + (self.records.len() * NETFLOW_V5_RECORD_LEN));

        // 24-byte Header
        buf.extend_from_slice(&self.header.version.to_be_bytes());
        buf.extend_from_slice(&(self.records.len() as u16).to_be_bytes());
        buf.extend_from_slice(&self.header.sys_uptime_ms.to_be_bytes());
        buf.extend_from_slice(&self.header.unix_secs.to_be_bytes());
        buf.extend_from_slice(&self.header.unix_nsecs.to_be_bytes());
        buf.extend_from_slice(&self.header.flow_sequence.to_be_bytes());
        buf.push(self.header.engine_type);
        buf.push(self.header.engine_id);
        buf.extend_from_slice(&self.header.sampling_interval.to_be_bytes());

        // 48-byte Records
        for rec in &self.records {
            buf.extend_from_slice(&rec.src_addr.0);
            buf.extend_from_slice(&rec.dst_addr.0);
            buf.extend_from_slice(&rec.next_hop.0);
            buf.extend_from_slice(&rec.input_ifindex.to_be_bytes());
            buf.extend_from_slice(&rec.output_ifindex.to_be_bytes());
            buf.extend_from_slice(&rec.packet_count.to_be_bytes());
            buf.extend_from_slice(&rec.octet_count.to_be_bytes());
            buf.extend_from_slice(&rec.first_uptime_ms.to_be_bytes());
            buf.extend_from_slice(&rec.last_uptime_ms.to_be_bytes());
            buf.extend_from_slice(&rec.src_port.to_be_bytes());
            buf.extend_from_slice(&rec.dst_port.to_be_bytes());
            buf.push(0); // Pad 1
            buf.push(rec.tcp_flags);
            buf.push(rec.protocol);
            buf.push(rec.tos);
            buf.extend_from_slice(&rec.src_as.to_be_bytes());
            buf.extend_from_slice(&rec.dst_as.to_be_bytes());
            buf.push(rec.src_mask);
            buf.push(rec.dst_mask);
            buf.extend_from_slice(&[0, 0]); // Pad 2
        }

        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < NETFLOW_V5_HEADER_LEN {
            return None;
        }

        let version = u16::from_be_bytes([data[0], data[1]]);
        if version != 5 {
            return None;
        }

        let count = u16::from_be_bytes([data[2], data[3]]) as usize;
        let sys_uptime_ms = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let unix_secs = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let unix_nsecs = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
        let flow_sequence = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let engine_type = data[20];
        let engine_id = data[21];
        let sampling_interval = u16::from_be_bytes([data[22], data[23]]);

        let header = NetflowV5Header {
            version,
            count: count as u16,
            sys_uptime_ms,
            unix_secs,
            unix_nsecs,
            flow_sequence,
            engine_type,
            engine_id,
            sampling_interval,
        };

        if data.len() < NETFLOW_V5_HEADER_LEN + (count * NETFLOW_V5_RECORD_LEN) {
            return None;
        }

        let mut records = Vec::with_capacity(count);
        let mut offset = NETFLOW_V5_HEADER_LEN;

        for _ in 0..count {
            let src_addr = Ipv4Address::new(data[offset], data[offset + 1], data[offset + 2], data[offset + 3]);
            let dst_addr = Ipv4Address::new(data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]);
            let next_hop = Ipv4Address::new(data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11]);
            let input_ifindex = u16::from_be_bytes([data[offset + 12], data[offset + 13]]);
            let output_ifindex = u16::from_be_bytes([data[offset + 14], data[offset + 15]]);
            let packet_count = u32::from_be_bytes([data[offset + 16], data[offset + 17], data[offset + 18], data[offset + 19]]);
            let octet_count = u32::from_be_bytes([data[offset + 20], data[offset + 21], data[offset + 22], data[offset + 23]]);
            let first_uptime_ms = u32::from_be_bytes([data[offset + 24], data[offset + 25], data[offset + 26], data[offset + 27]]);
            let last_uptime_ms = u32::from_be_bytes([data[offset + 28], data[offset + 29], data[offset + 30], data[offset + 31]]);
            let src_port = u16::from_be_bytes([data[offset + 32], data[offset + 33]]);
            let dst_port = u16::from_be_bytes([data[offset + 34], data[offset + 35]]);
            let tcp_flags = data[offset + 37];
            let protocol = data[offset + 38];
            let tos = data[offset + 39];
            let src_as = u16::from_be_bytes([data[offset + 40], data[offset + 41]]);
            let dst_as = u16::from_be_bytes([data[offset + 42], data[offset + 43]]);
            let src_mask = data[offset + 44];
            let dst_mask = data[offset + 45];

            records.push(NetflowV5Record {
                src_addr,
                dst_addr,
                next_hop,
                input_ifindex,
                output_ifindex,
                packet_count,
                octet_count,
                first_uptime_ms,
                last_uptime_ms,
                src_port,
                dst_port,
                tcp_flags,
                protocol,
                tos,
                src_as,
                dst_as,
                src_mask,
                dst_mask,
            });

            offset += NETFLOW_V5_RECORD_LEN;
        }

        Some(NetflowV5Packet { header, records })
    }
}

#[derive(Debug, Clone, Default)]
pub struct NetflowV5Table {
    pub flows: HashMap<(Ipv4Address, Ipv4Address, u16, u16, u8), NetflowV5Record>,
    pub exported_flows: u32,
}

impl NetflowV5Table {
    pub fn new() -> Self {
        NetflowV5Table {
            flows: HashMap::new(),
            exported_flows: 0,
        }
    }

    pub fn record_flow(
        &mut self,
        src: Ipv4Address,
        dst: Ipv4Address,
        next_hop: Ipv4Address,
        src_port: u16,
        dst_port: u16,
        protocol: u8,
        bytes: u32,
        now_ms: u32,
    ) {
        let key = (src, dst, src_port, dst_port, protocol);
        let entry = self.flows.entry(key).or_insert_with(|| NetflowV5Record {
            src_addr: src,
            dst_addr: dst,
            next_hop,
            input_ifindex: 1,
            output_ifindex: 2,
            packet_count: 0,
            octet_count: 0,
            first_uptime_ms: now_ms,
            last_uptime_ms: now_ms,
            src_port,
            dst_port,
            tcp_flags: 0x18, // PSH + ACK
            protocol,
            tos: 0,
            src_as: 65001,
            dst_as: 65002,
            src_mask: 24,
            dst_mask: 24,
        });

        entry.packet_count += 1;
        entry.octet_count += bytes;
        entry.last_uptime_ms = now_ms;
    }

    pub fn export_packet(&mut self, uptime_ms: u32, unix_secs: u32) -> NetflowV5Packet {
        let records: Vec<NetflowV5Record> = self.flows.drain().map(|(_, r)| r).collect();
        let count = records.len() as u16;
        self.exported_flows += count as u32;

        let header = NetflowV5Header {
            version: 5,
            count,
            sys_uptime_ms: uptime_ms,
            unix_secs,
            unix_nsecs: 0,
            flow_sequence: self.exported_flows,
            engine_type: 0,
            engine_id: 0,
            sampling_interval: 0,
        };

        NetflowV5Packet { header, records }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_netflow_v5_packet_roundtrip() {
        let mut table = NetflowV5Table::new();
        let src = Ipv4Address::new(10, 0, 1, 50);
        let dst = Ipv4Address::new(172, 16, 2, 80);
        let next_hop = Ipv4Address::new(10, 0, 1, 1);

        table.record_flow(src, dst, next_hop, 45123, 80, 6, 1460, 1000);
        table.record_flow(src, dst, next_hop, 45123, 80, 6, 1460, 1050);

        let pkt = table.export_packet(1050, 1700000000);
        assert_eq!(pkt.header.count, 1);
        assert_eq!(pkt.records[0].packet_count, 2);
        assert_eq!(pkt.records[0].octet_count, 2920);

        let raw = pkt.serialize();
        assert_eq!(raw.len(), NETFLOW_V5_HEADER_LEN + NETFLOW_V5_RECORD_LEN);

        let parsed = NetflowV5Packet::parse(&raw).unwrap();
        assert_eq!(parsed.header.version, 5);
        assert_eq!(parsed.records[0].src_addr, src);
        assert_eq!(parsed.records[0].dst_addr, dst);
        assert_eq!(parsed.records[0].src_port, 45123);
        assert_eq!(parsed.records[0].dst_port, 80);
    }
}
