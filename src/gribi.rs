//! gRPC Routing Information Base Interface (gRIBI - OpenConfig SDN Control Plane Injection).
//!
//! Provides direct SDN controller programming of Abstract Forwarding Tables (AFT)
//! including IPv4/IPv6 prefixes, Next Hop Groups (NHG), and Next Hops (NH).

use crate::ethernet::MacAddress;
use crate::ipv4::Ipv4Address;
use std::collections::HashMap;

pub const GRIBI_PORT: u16 = 9340;
pub const GRIBI_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GribiOpType {
    Add,
    Modify,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GribiNextHop {
    pub id: u64,
    pub ip: Ipv4Address,
    pub mac: MacAddress,
    pub weight: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GribiNextHopGroup {
    pub id: u64,
    pub next_hop_ids: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GribiIpv4Entry {
    pub prefix: Ipv4Address,
    pub prefix_len: u8,
    pub next_hop_group_id: u64,
}

/// Abstract Forwarding Table (AFT) managed via gRIBI
#[derive(Debug, Clone, Default)]
pub struct GribiAftTable {
    pub next_hops: HashMap<u64, GribiNextHop>,
    pub next_hop_groups: HashMap<u64, GribiNextHopGroup>,
    pub ipv4_entries: HashMap<(Ipv4Address, u8), GribiIpv4Entry>,
    pub programmed_operations_count: u32,
}

impl GribiAftTable {
    pub fn new() -> Self {
        GribiAftTable {
            next_hops: HashMap::new(),
            next_hop_groups: HashMap::new(),
            ipv4_entries: HashMap::new(),
            programmed_operations_count: 0,
        }
    }

    /// Injects or modifies a NextHop (NH)
    pub fn set_next_hop(&mut self, nh: GribiNextHop) {
        self.next_hops.insert(nh.id, nh);
        self.programmed_operations_count += 1;
    }

    /// Injects or modifies a NextHopGroup (NHG)
    pub fn set_next_hop_group(&mut self, nhg: GribiNextHopGroup) {
        self.next_hop_groups.insert(nhg.id, nhg);
        self.programmed_operations_count += 1;
    }

    /// Injects or modifies an IPv4 prefix route entry
    pub fn set_ipv4_entry(&mut self, entry: GribiIpv4Entry) {
        self.ipv4_entries
            .insert((entry.prefix, entry.prefix_len), entry);
        self.programmed_operations_count += 1;
    }

    /// Deletes an IPv4 prefix route entry
    pub fn delete_ipv4_entry(&mut self, prefix: Ipv4Address, prefix_len: u8) -> bool {
        let removed = self.ipv4_entries.remove(&(prefix, prefix_len)).is_some();
        if removed {
            self.programmed_operations_count += 1;
        }
        removed
    }

    /// Resolves an IP destination to its active NextHop via AFT LPM lookup
    pub fn resolve_fib(&self, dst: Ipv4Address) -> Option<&GribiNextHop> {
        let mut best_match: Option<(&GribiIpv4Entry, u8)> = None;

        for ((prefix, len), entry) in &self.ipv4_entries {
            let mask = if *len == 0 { 0u32 } else { !0u32 << (32 - len) };
            if (dst.to_u32() & mask) == (prefix.to_u32() & mask) {
                if let Some((_, best_len)) = best_match {
                    if *len > best_len {
                        best_match = Some((entry, *len));
                    }
                } else {
                    best_match = Some((entry, *len));
                }
            }
        }

        if let Some((entry, _)) = best_match
            && let Some(nhg) = self.next_hop_groups.get(&entry.next_hop_group_id)
            && let Some(&first_nh_id) = nhg.next_hop_ids.first()
        {
            return self.next_hops.get(&first_nh_id);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gribi_aft_programming_and_fib_resolution() {
        let mut aft = GribiAftTable::new();

        // 1. Program NextHop
        aft.set_next_hop(GribiNextHop {
            id: 1,
            ip: Ipv4Address::new(192, 168, 1, 254),
            mac: MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]),
            weight: 100,
        });

        // 2. Program NextHopGroup
        aft.set_next_hop_group(GribiNextHopGroup {
            id: 10,
            next_hop_ids: vec![1],
        });

        // 3. Program IPv4 Entry
        aft.set_ipv4_entry(GribiIpv4Entry {
            prefix: Ipv4Address::new(10, 0, 0, 0),
            prefix_len: 8,
            next_hop_group_id: 10,
        });

        // 4. Test FIB Resolution
        let resolved = aft.resolve_fib(Ipv4Address::new(10, 12, 34, 56)).unwrap();
        assert_eq!(resolved.id, 1);
        assert_eq!(resolved.ip, Ipv4Address::new(192, 168, 1, 254));
        assert_eq!(aft.programmed_operations_count, 3);
    }
}
