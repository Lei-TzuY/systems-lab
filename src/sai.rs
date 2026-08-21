//! OpenCompute Project Switch Abstraction Interface (OCP SAI / SONiC Hardware Abstraction).
//!
//! Provides a standardized, ASIC-independent hardware table programming model for
//! FDB MAC learning, VLAN membership, NextHops, and IP Route tables.

use crate::ethernet::MacAddress;
use crate::ipv4::Ipv4Address;
use std::collections::HashMap;

pub const SAI_STATUS_SUCCESS: u32 = 0x00000000;
pub const SAI_STATUS_ITEM_NOT_FOUND: u32 = 0x00000002;
pub const SAI_STATUS_TABLE_FULL: u32 = 0x00000003;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SaiFdbEntry {
    pub switch_id: u64,
    pub mac_address: MacAddress,
    pub bv_id: u64, // Bridge/VLAN ID
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SaiRouteEntry {
    pub switch_id: u64,
    pub vr_id: u64, // Virtual Router ID
    pub destination: Ipv4Address,
    pub prefix_len: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaiNextHop {
    pub id: u64,
    pub ip: Ipv4Address,
    pub mac: MacAddress,
    pub port_id: u64,
}

/// Simulated SAI ASIC Hardware Forwarding Tables
#[derive(Debug, Clone, Default)]
pub struct SaiSwitchAdapter {
    pub switch_id: u64,
    pub fdb_table: HashMap<SaiFdbEntry, u64>, // Entry -> Port ID
    pub next_hops: HashMap<u64, SaiNextHop>,
    pub route_table: HashMap<SaiRouteEntry, u64>, // Route -> NextHop ID
    pub next_id: u64,
}

impl SaiSwitchAdapter {
    pub fn new(switch_id: u64) -> Self {
        SaiSwitchAdapter {
            switch_id,
            fdb_table: HashMap::new(),
            next_hops: HashMap::new(),
            route_table: HashMap::new(),
            next_id: 100,
        }
    }

    /// SAI FDB Create: Programs hardware MAC forwarding entry
    pub fn create_fdb_entry(&mut self, mac: MacAddress, vlan_id: u64, port_id: u64) -> u32 {
        let entry = SaiFdbEntry {
            switch_id: self.switch_id,
            mac_address: mac,
            bv_id: vlan_id,
        };
        self.fdb_table.insert(entry, port_id);
        SAI_STATUS_SUCCESS
    }

    /// SAI NextHop Create: Programs hardware NextHop object
    pub fn create_next_hop(&mut self, ip: Ipv4Address, mac: MacAddress, port_id: u64) -> u64 {
        let nh_id = self.next_id;
        self.next_id += 1;
        self.next_hops.insert(
            nh_id,
            SaiNextHop {
                id: nh_id,
                ip,
                mac,
                port_id,
            },
        );
        nh_id
    }

    /// SAI Route Create: Programs hardware Route entry mapped to NextHop object
    pub fn create_route_entry(
        &mut self,
        vr_id: u64,
        destination: Ipv4Address,
        prefix_len: u8,
        next_hop_id: u64,
    ) -> u32 {
        let entry = SaiRouteEntry {
            switch_id: self.switch_id,
            vr_id,
            destination,
            prefix_len,
        };
        self.route_table.insert(entry, next_hop_id);
        SAI_STATUS_SUCCESS
    }

    /// Performs hardware L2 FDB forwarding lookup
    pub fn lookup_fdb(&self, mac: MacAddress, vlan_id: u64) -> Option<u64> {
        let entry = SaiFdbEntry {
            switch_id: self.switch_id,
            mac_address: mac,
            bv_id: vlan_id,
        };
        self.fdb_table.get(&entry).copied()
    }

    /// Performs hardware L3 Route forwarding lookup
    pub fn lookup_route(&self, vr_id: u64, ip: Ipv4Address) -> Option<&SaiNextHop> {
        let mut best_match: Option<(&SaiRouteEntry, u8, u64)> = None;

        for (entry, &nh_id) in &self.route_table {
            if entry.vr_id != vr_id {
                continue;
            }
            let mask = if entry.prefix_len == 0 {
                0u32
            } else {
                !0u32 << (32 - entry.prefix_len)
            };
            if (ip.to_u32() & mask) == (entry.destination.to_u32() & mask) {
                if let Some((_, best_len, _)) = best_match {
                    if entry.prefix_len > best_len {
                        best_match = Some((entry, entry.prefix_len, nh_id));
                    }
                } else {
                    best_match = Some((entry, entry.prefix_len, nh_id));
                }
            }
        }

        if let Some((_, _, nh_id)) = best_match {
            self.next_hops.get(&nh_id)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sai_fdb_and_route_programming() {
        let mut adapter = SaiSwitchAdapter::new(1);
        let mac = MacAddress([0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);

        // Program FDB
        assert_eq!(adapter.create_fdb_entry(mac, 100, 5), SAI_STATUS_SUCCESS);
        assert_eq!(adapter.lookup_fdb(mac, 100), Some(5));

        // Program NextHop and Route
        let nh_id = adapter.create_next_hop(
            Ipv4Address::new(192, 168, 1, 1),
            MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]),
            1,
        );
        assert_eq!(
            adapter.create_route_entry(0, Ipv4Address::new(10, 0, 0, 0), 8, nh_id),
            SAI_STATUS_SUCCESS
        );

        let resolved_nh = adapter
            .lookup_route(0, Ipv4Address::new(10, 20, 30, 40))
            .unwrap();
        assert_eq!(resolved_nh.port_id, 1);
        assert_eq!(resolved_nh.ip, Ipv4Address::new(192, 168, 1, 1));
    }
}
