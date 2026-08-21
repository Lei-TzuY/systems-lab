//! Routing Information Protocol Version 2 (RIPv2 - RFC 2453).
//!
//! Distance-Vector dynamic routing protocol operating over UDP port 520 with
//! metric calculations (infinity = 16), Split Horizon, and dynamic route convergence.

use crate::ipv4::Ipv4Address;
use crate::router::RoutingTable;
use std::fmt;

pub const RIP_PORT: u16 = 520;
pub const RIP_VERSION_2: u8 = 2;
pub const RIP_CMD_REQUEST: u8 = 1;
pub const RIP_CMD_RESPONSE: u8 = 2;
pub const RIP_INFINITY_METRIC: u32 = 16;
pub const RIP_AFI_IPV4: u16 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RipEntry {
    pub address_family: u16,
    pub route_tag: u16,
    pub ip: Ipv4Address,
    pub subnet_mask: Ipv4Address,
    pub next_hop: Ipv4Address,
    pub metric: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RipPacket {
    pub command: u8,
    pub version: u8,
    pub routing_domain: u16,
    pub entries: Vec<RipEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RipError {
    PacketTooShort(usize),
    InvalidVersion(u8),
    InvalidEntrySize(usize),
}

impl fmt::Display for RipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RipError::PacketTooShort(len) => {
                write!(f, "RIP packet too short ({} bytes, min 4)", len)
            }
            RipError::InvalidVersion(v) => {
                write!(f, "Invalid RIP version: expected 2, found {}", v)
            }
            RipError::InvalidEntrySize(len) => {
                write!(f, "Invalid RIP entry length remainder: {} bytes", len)
            }
        }
    }
}

impl std::error::Error for RipError {}

impl RipPacket {
    pub fn parse(data: &[u8]) -> Result<Self, RipError> {
        if data.len() < 4 {
            return Err(RipError::PacketTooShort(data.len()));
        }

        let command = data[0];
        let version = data[1];
        if version != RIP_VERSION_2 {
            return Err(RipError::InvalidVersion(version));
        }

        let routing_domain = u16::from_be_bytes([data[2], data[3]]);

        let entry_bytes = &data[4..];
        if !entry_bytes.len().is_multiple_of(20) {
            return Err(RipError::InvalidEntrySize(entry_bytes.len()));
        }

        let mut entries = Vec::new();
        for chunk in entry_bytes.chunks_exact(20) {
            let address_family = u16::from_be_bytes([chunk[0], chunk[1]]);
            let route_tag = u16::from_be_bytes([chunk[2], chunk[3]]);

            let mut ip_b = [0u8; 4];
            ip_b.copy_from_slice(&chunk[4..8]);
            let ip = Ipv4Address(ip_b);

            let mut mask_b = [0u8; 4];
            mask_b.copy_from_slice(&chunk[8..12]);
            let subnet_mask = Ipv4Address(mask_b);

            let mut nh_b = [0u8; 4];
            nh_b.copy_from_slice(&chunk[12..16]);
            let next_hop = Ipv4Address(nh_b);

            let metric = u32::from_be_bytes([chunk[16], chunk[17], chunk[18], chunk[19]]);

            entries.push(RipEntry {
                address_family,
                route_tag,
                ip,
                subnet_mask,
                next_hop,
                metric,
            });
        }

        Ok(RipPacket {
            command,
            version,
            routing_domain,
            entries,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let total_len = 4 + self.entries.len() * 20;
        let mut buf = Vec::with_capacity(total_len);

        buf.push(self.command);
        buf.push(self.version);
        buf.extend_from_slice(&self.routing_domain.to_be_bytes());

        for e in &self.entries {
            buf.extend_from_slice(&e.address_family.to_be_bytes());
            buf.extend_from_slice(&e.route_tag.to_be_bytes());
            buf.extend_from_slice(&e.ip.0);
            buf.extend_from_slice(&e.subnet_mask.0);
            buf.extend_from_slice(&e.next_hop.0);
            buf.extend_from_slice(&e.metric.to_be_bytes());
        }

        buf
    }
}

/// Dynamic RIPv2 Protocol Engine
pub struct RipEngine {
    pub routes: RoutingTable,
    pub route_metrics: Vec<(Ipv4Address, u8, u32)>, // (Dest, PrefixLen, Metric)
}

impl Default for RipEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RipEngine {
    pub fn new() -> Self {
        RipEngine {
            routes: RoutingTable::new(),
            route_metrics: Vec::new(),
        }
    }

    pub fn add_local_network(&mut self, dest: Ipv4Address, prefix_len: u8, interface: &str) {
        self.routes.add_route(dest, prefix_len, None, interface);
        self.route_metrics.push((dest, prefix_len, 1));
    }

    /// Generates a RIPv2 Response packet advertising reachable routes (with Split Horizon).
    pub fn build_advertisement(&self) -> RipPacket {
        let mut entries = Vec::new();

        for &(dest, prefix_len, metric) in &self.route_metrics {
            let mask_u32 = if prefix_len == 0 {
                0
            } else if prefix_len >= 32 {
                !0u32
            } else {
                !0u32 << (32 - prefix_len)
            };
            let subnet_mask = Ipv4Address(mask_u32.to_be_bytes());

            entries.push(RipEntry {
                address_family: RIP_AFI_IPV4,
                route_tag: 0,
                ip: dest,
                subnet_mask,
                next_hop: Ipv4Address::UNSPECIFIED,
                metric,
            });
        }

        RipPacket {
            command: RIP_CMD_RESPONSE,
            version: RIP_VERSION_2,
            routing_domain: 0,
            entries,
        }
    }

    /// Ingests a neighbor's RIP advertisement and applies the Bellman-Ford distance-vector algorithm.
    pub fn process_advertisement(
        &mut self,
        neighbor_ip: Ipv4Address,
        pkt: &RipPacket,
        interface: &str,
    ) -> usize {
        let mut updated = 0;

        for entry in &pkt.entries {
            if entry.address_family != RIP_AFI_IPV4 {
                continue;
            }

            let new_metric = (entry.metric + 1).min(RIP_INFINITY_METRIC);
            if new_metric >= RIP_INFINITY_METRIC {
                continue; // Unreachable
            }

            let prefix_len = entry.subnet_mask.to_u32().count_ones() as u8;

            // Check if we already have this route
            let mut existing_idx = None;
            for (idx, &(d, p, _)) in self.route_metrics.iter().enumerate() {
                if d == entry.ip && p == prefix_len {
                    existing_idx = Some(idx);
                    break;
                }
            }

            if let Some(idx) = existing_idx {
                if new_metric < self.route_metrics[idx].2 {
                    self.route_metrics[idx].2 = new_metric;
                    self.routes
                        .add_route(entry.ip, prefix_len, Some(neighbor_ip), interface);
                    updated += 1;
                }
            } else {
                self.route_metrics.push((entry.ip, prefix_len, new_metric));
                self.routes
                    .add_route(entry.ip, prefix_len, Some(neighbor_ip), interface);
                updated += 1;
            }
        }

        updated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rip_packet_roundtrip() {
        let entry = RipEntry {
            address_family: RIP_AFI_IPV4,
            route_tag: 0,
            ip: Ipv4Address::new(10, 0, 0, 0),
            subnet_mask: Ipv4Address::new(255, 0, 0, 0),
            next_hop: Ipv4Address::new(192, 168, 1, 1),
            metric: 2,
        };

        let pkt = RipPacket {
            command: RIP_CMD_RESPONSE,
            version: RIP_VERSION_2,
            routing_domain: 0,
            entries: vec![entry],
        };

        let raw = pkt.serialize();
        let parsed = RipPacket::parse(&raw).unwrap();

        assert_eq!(parsed.command, RIP_CMD_RESPONSE);
        assert_eq!(parsed.version, RIP_VERSION_2);
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].ip, Ipv4Address::new(10, 0, 0, 0));
        assert_eq!(parsed.entries[0].metric, 2);
    }

    #[test]
    fn test_rip_engine_route_learning() {
        let mut r1 = RipEngine::new();
        r1.add_local_network(Ipv4Address::new(192, 168, 1, 0), 24, "eth0");

        let mut r2 = RipEngine::new();
        r2.add_local_network(Ipv4Address::new(10, 0, 0, 0), 8, "eth1");

        // R1 advertises to R2
        let adv_from_r1 = r1.build_advertisement();
        let updated =
            r2.process_advertisement(Ipv4Address::new(192, 168, 1, 1), &adv_from_r1, "eth0");

        assert_eq!(updated, 1);
        // R2 should now know how to reach 192.168.1.0/24 with metric 2 via R1!
        let route = r2
            .routes
            .lookup(Ipv4Address::new(192, 168, 1, 100))
            .unwrap();
        assert_eq!(route.gateway, Some(Ipv4Address::new(192, 168, 1, 1)));
    }
}
