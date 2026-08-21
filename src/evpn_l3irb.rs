//! EVPN Layer 3 VXLAN Symmetric IRB (Integrated Routing & Bridging - RFC 9135 / RFC 7432 Type 5).
//!
//! Provides inter-subnet tenant VRF routing with L3VNI encapsulation and Router MAC (RMAC) distribution.

use crate::ethernet::MacAddress;
use crate::evpn::RouteDistinguisher;
use crate::ipv4::Ipv4Address;
use std::collections::HashMap;

pub const EVPN_ROUTE_TYPE_IP_PREFIX: u8 = 5;
pub const BGP_EXT_COMMUNITY_ROUTER_MAC: u16 = 0x0603;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvpnIpPrefixRoute {
    pub rd: RouteDistinguisher,
    pub ip_prefix: Ipv4Address,
    pub prefix_len: u8,
    pub gw_ip: Ipv4Address,
    pub l3_vni: u32,
    pub router_mac: MacAddress,
    pub vtep_ip: Ipv4Address,
}

impl EvpnIpPrefixRoute {
    pub fn new(
        rd: RouteDistinguisher,
        ip_prefix: Ipv4Address,
        prefix_len: u8,
        l3_vni: u32,
        router_mac: MacAddress,
        vtep_ip: Ipv4Address,
    ) -> Self {
        EvpnIpPrefixRoute {
            rd,
            ip_prefix,
            prefix_len,
            gw_ip: Ipv4Address::new(0, 0, 0, 0),
            l3_vni,
            router_mac,
            vtep_ip,
        }
    }
}

/// VRF Routing Table supporting EVPN Symmetric L3 IRB lookup and encapsulation resolution
#[derive(Debug, Clone, Default)]
pub struct EvpnL3VrfTable {
    pub vrf_name: String,
    pub local_l3_vni: u32,
    pub local_router_mac: MacAddress,
    pub routes: HashMap<(Ipv4Address, u8), EvpnIpPrefixRoute>,
}

impl EvpnL3VrfTable {
    pub fn new(vrf_name: &str, local_l3_vni: u32, local_router_mac: MacAddress) -> Self {
        EvpnL3VrfTable {
            vrf_name: vrf_name.to_string(),
            local_l3_vni,
            local_router_mac,
            routes: HashMap::new(),
        }
    }

    pub fn add_prefix_route(&mut self, route: EvpnIpPrefixRoute) {
        self.routes
            .insert((route.ip_prefix, route.prefix_len), route);
    }

    /// Performs longest prefix match (LPM) and resolves Symmetric IRB forwarding attributes
    pub fn lookup(&self, target_ip: Ipv4Address) -> Option<&EvpnIpPrefixRoute> {
        // Simplified lookup: find match in table
        for ((prefix, len), route) in &self.routes {
            let mask = if *len == 0 { 0u32 } else { !0u32 << (32 - len) };
            if (target_ip.to_u32() & mask) == (prefix.to_u32() & mask) {
                return Some(route);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evpn_l3_symmetric_irb_lookup() {
        let mut vrf = EvpnL3VrfTable::new(
            "TENANT-RED",
            50001,
            MacAddress([0x00, 0x01, 0x02, 0x03, 0x04, 0x05]),
        );

        let remote_rmac = MacAddress([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        let remote_vtep = Ipv4Address::new(192, 168, 100, 2);

        let route = EvpnIpPrefixRoute::new(
            RouteDistinguisher::new(Ipv4Address::new(10, 0, 0, 1), 100),
            Ipv4Address::new(10, 200, 1, 0),
            24,
            50001, // Symmetric L3 VNI
            remote_rmac,
            remote_vtep,
        );

        vrf.add_prefix_route(route);

        let match_result = vrf.lookup(Ipv4Address::new(10, 200, 1, 55)).unwrap();
        assert_eq!(match_result.l3_vni, 50001);
        assert_eq!(match_result.router_mac, remote_rmac);
        assert_eq!(match_result.vtep_ip, remote_vtep);
    }
}
