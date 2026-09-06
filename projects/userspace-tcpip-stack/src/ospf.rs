//! Open Shortest Path First Version 2 (OSPFv2 - RFC 2328).
//!
//! Link-State dynamic routing protocol over IP Protocol 89 (Multicast 224.0.0.5 AllSPFRouters).
//! Features 24-byte OSPF framing, Hello packets, LSDB graph topology, and Dijkstra SPF calculation.

use crate::checksum::compute_checksum;
use crate::ipv4::Ipv4Address;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::fmt;

pub const IP_PROTO_OSPF: u8 = 89;
pub const OSPF_ALL_SPF_ROUTERS: Ipv4Address = Ipv4Address([224, 0, 0, 5]);
pub const OSPF_ALL_D_ROUTERS: Ipv4Address = Ipv4Address([224, 0, 0, 6]);

pub const OSPF_VERSION_2: u8 = 2;
pub const OSPF_HEADER_LEN: usize = 24;

// OSPF Packet Types
pub const OSPF_TYPE_HELLO: u8 = 1;
pub const OSPF_TYPE_DB_DESC: u8 = 2;
pub const OSPF_TYPE_LS_REQ: u8 = 3;
pub const OSPF_TYPE_LS_UPDATE: u8 = 4;
pub const OSPF_TYPE_LS_ACK: u8 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OspfHeader {
    pub version: u8,
    pub msg_type: u8,
    pub length: u16,
    pub router_id: Ipv4Address,
    pub area_id: Ipv4Address,
    pub checksum: u16,
    pub autype: u16,
    pub auth: [u8; 8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OspfHelloPacket {
    pub header: OspfHeader,
    pub network_mask: Ipv4Address,
    pub hello_interval: u16,
    pub options: u8,
    pub priority: u8,
    pub dead_interval: u32,
    pub designated_router: Ipv4Address,
    pub backup_designated_router: Ipv4Address,
    pub neighbors: Vec<Ipv4Address>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OspfError {
    PacketTooShort(usize),
    InvalidVersion(u8),
    InvalidChecksum,
}

impl fmt::Display for OspfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OspfError::PacketTooShort(l) => {
                write!(f, "OSPF packet too short ({} bytes, min 24)", l)
            }
            OspfError::InvalidVersion(v) => {
                write!(f, "Invalid OSPF version: expected 2, found {}", v)
            }
            OspfError::InvalidChecksum => write!(f, "OSPF checksum verification failed"),
        }
    }
}

impl std::error::Error for OspfError {}

impl OspfHeader {
    pub fn parse(data: &[u8]) -> Result<Self, OspfError> {
        if data.len() < OSPF_HEADER_LEN {
            return Err(OspfError::PacketTooShort(data.len()));
        }

        let version = data[0];
        if version != OSPF_VERSION_2 {
            return Err(OspfError::InvalidVersion(version));
        }

        let msg_type = data[1];
        let length = u16::from_be_bytes([data[2], data[3]]);
        let router_id = Ipv4Address([data[4], data[5], data[6], data[7]]);
        let area_id = Ipv4Address([data[8], data[9], data[10], data[11]]);
        let checksum = u16::from_be_bytes([data[12], data[13]]);
        let autype = u16::from_be_bytes([data[14], data[15]]);

        let mut auth = [0u8; 8];
        auth.copy_from_slice(&data[16..24]);

        Ok(OspfHeader {
            version,
            msg_type,
            length,
            router_id,
            area_id,
            checksum,
            autype,
            auth,
        })
    }

    pub fn serialize_into(&self, buf: &mut [u8]) {
        buf[0] = self.version;
        buf[1] = self.msg_type;
        buf[2..4].copy_from_slice(&self.length.to_be_bytes());
        buf[4..8].copy_from_slice(&self.router_id.0);
        buf[8..12].copy_from_slice(&self.area_id.0);
        buf[12..14].copy_from_slice(&self.checksum.to_be_bytes());
        buf[14..16].copy_from_slice(&self.autype.to_be_bytes());
        buf[16..24].copy_from_slice(&self.auth);
    }
}

impl OspfHelloPacket {
    pub fn parse(data: &[u8], verify_csum: bool) -> Result<Self, OspfError> {
        if data.len() < OSPF_HEADER_LEN + 20 {
            return Err(OspfError::PacketTooShort(data.len()));
        }

        let header = OspfHeader::parse(&data[..OSPF_HEADER_LEN])?;
        if verify_csum && compute_checksum(data) != 0 {
            return Err(OspfError::InvalidChecksum);
        }

        let body = &data[OSPF_HEADER_LEN..];
        let network_mask = Ipv4Address([body[0], body[1], body[2], body[3]]);
        let hello_interval = u16::from_be_bytes([body[4], body[5]]);
        let options = body[6];
        let priority = body[7];
        let dead_interval = u32::from_be_bytes([body[8], body[9], body[10], body[11]]);
        let designated_router = Ipv4Address([body[12], body[13], body[14], body[15]]);
        let backup_designated_router = Ipv4Address([body[16], body[17], body[18], body[19]]);

        let mut neighbors = Vec::new();
        for chunk in body[20..].chunks_exact(4) {
            neighbors.push(Ipv4Address([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }

        Ok(OspfHelloPacket {
            header,
            network_mask,
            hello_interval,
            options,
            priority,
            dead_interval,
            designated_router,
            backup_designated_router,
            neighbors,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let total_len = OSPF_HEADER_LEN + 20 + self.neighbors.len() * 4;
        let mut buf = vec![0u8; total_len];

        let mut hdr = self.header.clone();
        hdr.length = total_len as u16;
        hdr.checksum = 0;
        hdr.serialize_into(&mut buf[..OSPF_HEADER_LEN]);

        let body = &mut buf[OSPF_HEADER_LEN..];
        body[0..4].copy_from_slice(&self.network_mask.0);
        body[4..6].copy_from_slice(&self.hello_interval.to_be_bytes());
        body[6] = self.options;
        body[7] = self.priority;
        body[8..12].copy_from_slice(&self.dead_interval.to_be_bytes());
        body[12..16].copy_from_slice(&self.designated_router.0);
        body[16..20].copy_from_slice(&self.backup_designated_router.0);

        for (i, n) in self.neighbors.iter().enumerate() {
            body[20 + i * 4..20 + (i + 1) * 4].copy_from_slice(&n.0);
        }

        let csum = compute_checksum(&buf);
        buf[12] = (csum >> 8) as u8;
        buf[13] = (csum & 0xFF) as u8;

        buf
    }

    pub fn build_hello(
        router_id: Ipv4Address,
        mask: Ipv4Address,
        dr: Ipv4Address,
        neighbors: Vec<Ipv4Address>,
    ) -> Self {
        let header = OspfHeader {
            version: OSPF_VERSION_2,
            msg_type: OSPF_TYPE_HELLO,
            length: (OSPF_HEADER_LEN + 20 + neighbors.len() * 4) as u16,
            router_id,
            area_id: Ipv4Address::new(0, 0, 0, 0), // Area 0 Backbone
            checksum: 0,
            autype: 0,
            auth: [0; 8],
        };

        OspfHelloPacket {
            header,
            network_mask: mask,
            hello_interval: 10,
            options: 0x02,
            priority: 1,
            dead_interval: 40,
            designated_router: dr,
            backup_designated_router: Ipv4Address::new(0, 0, 0, 0),
            neighbors,
        }
    }
}

// --- Dijkstra Shortest Path First (SPF) Engine ---

#[derive(Copy, Clone, Eq, PartialEq)]
struct SpfNode {
    cost: u32,
    router: Ipv4Address,
}

impl Ord for SpfNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.cmp(&self.cost) // Min-heap
    }
}

impl PartialOrd for SpfNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Link State Database (LSDB)
pub struct OspfLsdb {
    // Router ID -> List of (Neighbor Router ID, Link Metric Cost)
    adjacencies: HashMap<Ipv4Address, Vec<(Ipv4Address, u32)>>,
}

impl Default for OspfLsdb {
    fn default() -> Self {
        Self::new()
    }
}

impl OspfLsdb {
    pub fn new() -> Self {
        let mut lsdb = OspfLsdb {
            adjacencies: HashMap::new(),
        };
        // Setup a 3-node triangle topology: R1 <-> R2 (cost 10), R2 <-> R3 (cost 10), R1 <-> R3 (cost 50)
        let r1 = Ipv4Address::new(1, 1, 1, 1);
        let r2 = Ipv4Address::new(2, 2, 2, 2);
        let r3 = Ipv4Address::new(3, 3, 3, 3);

        lsdb.add_link(r1, r2, 10);
        lsdb.add_link(r2, r3, 10);
        lsdb.add_link(r1, r3, 50); // Direct link is high cost (50 vs 10+10=20 via R2)
        lsdb
    }

    pub fn add_link(&mut self, from: Ipv4Address, to: Ipv4Address, cost: u32) {
        self.adjacencies.entry(from).or_default().push((to, cost));
        self.adjacencies.entry(to).or_default().push((from, cost));
    }

    /// Computes shortest path tree from source router using Dijkstra's algorithm
    pub fn compute_shortest_paths(
        &self,
        src: Ipv4Address,
    ) -> HashMap<Ipv4Address, (u32, Option<Ipv4Address>)> {
        let mut dist: HashMap<Ipv4Address, u32> = HashMap::new();
        let mut next_hop: HashMap<Ipv4Address, Option<Ipv4Address>> = HashMap::new();
        let mut heap = BinaryHeap::new();

        dist.insert(src, 0);
        heap.push(SpfNode {
            cost: 0,
            router: src,
        });

        while let Some(SpfNode { cost, router }) = heap.pop() {
            if cost > *dist.get(&router).unwrap_or(&u32::MAX) {
                continue;
            }

            if let Some(neighbors) = self.adjacencies.get(&router) {
                for &(next, link_cost) in neighbors {
                    let next_cost = cost + link_cost;
                    if next_cost < *dist.get(&next).unwrap_or(&u32::MAX) {
                        dist.insert(next, next_cost);

                        // Determine next hop from source
                        let nh = if router == src {
                            Some(next)
                        } else {
                            next_hop.get(&router).cloned().flatten()
                        };
                        next_hop.insert(next, nh);

                        heap.push(SpfNode {
                            cost: next_cost,
                            router: next,
                        });
                    }
                }
            }
        }

        let mut results = HashMap::new();
        for (rtr, c) in dist {
            if rtr != src {
                results.insert(rtr, (c, next_hop.get(&rtr).cloned().flatten()));
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ospf_hello_roundtrip() {
        let r1 = Ipv4Address::new(192, 168, 1, 1);
        let mask = Ipv4Address::new(255, 255, 255, 0);
        let dr = Ipv4Address::new(192, 168, 1, 10);
        let nbr = vec![Ipv4Address::new(192, 168, 1, 100)];

        let hello = OspfHelloPacket::build_hello(r1, mask, dr, nbr);
        let raw = hello.serialize();

        let parsed = OspfHelloPacket::parse(&raw, true).unwrap();
        assert_eq!(parsed.header.version, OSPF_VERSION_2);
        assert_eq!(parsed.header.msg_type, OSPF_TYPE_HELLO);
        assert_eq!(parsed.header.router_id, r1);
        assert_eq!(parsed.designated_router, dr);
        assert_eq!(parsed.neighbors.len(), 1);
        assert_eq!(parsed.neighbors[0], Ipv4Address::new(192, 168, 1, 100));
    }

    #[test]
    fn test_ospf_dijkstra_spf_calculation() {
        let lsdb = OspfLsdb::new();
        let r1 = Ipv4Address::new(1, 1, 1, 1);
        let r2 = Ipv4Address::new(2, 2, 2, 2);
        let r3 = Ipv4Address::new(3, 3, 3, 3);

        let spf = lsdb.compute_shortest_paths(r1);

        // Path to R2: direct (cost 10, next hop R2)
        assert_eq!(spf.get(&r2), Some(&(10, Some(r2))));

        // Path to R3: via R2 (cost 10+10=20 < direct 50, next hop R2)
        assert_eq!(spf.get(&r3), Some(&(20, Some(r2))));
    }
}
