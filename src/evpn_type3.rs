//! BGP EVPN Route Type 3: Inclusive Multicast Ethernet Tag Route (IMET / RFC 7432 & RFC 6514).
//!
//! Implements EVPN Route Type 3 NLRI and PMSI (Provider Multicast Service Interface)
//! Tunnel Attribute for BUM (Broadcast, Unknown unicast, Multicast) flooding across overlays.

use crate::evpn::RouteDistinguisher;
use crate::ipv4::Ipv4Address;

pub const EVPN_ROUTE_TYPE_IMET: u8 = 3;
pub const PMSI_TUNNEL_TYPE_INGRESS_REPLICATION: u8 = 6;

/// PMSI Tunnel Attribute (RFC 6514 / RFC 7432)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmsiTunnelAttribute {
    pub flags: u8,                  // e.g. Leaf Information Required (0x01)
    pub tunnel_type: u8,            // 0x06 = Ingress Replication (IR)
    pub mpls_label_or_vni: u32,     // 24-bit VNI or 20-bit MPLS label
    pub tunnel_endpoint: Ipv4Address, // Tunnel Egress IP
}

impl PmsiTunnelAttribute {
    pub fn new_ingress_replication(vni: u32, endpoint: Ipv4Address) -> Self {
        PmsiTunnelAttribute {
            flags: 0,
            tunnel_type: PMSI_TUNNEL_TYPE_INGRESS_REPLICATION,
            mpls_label_or_vni: vni,
            tunnel_endpoint: endpoint,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(9);
        buf.push(self.flags);
        buf.push(self.tunnel_type);
        // 3-byte label/VNI
        buf.push(((self.mpls_label_or_vni >> 16) & 0xFF) as u8);
        buf.push(((self.mpls_label_or_vni >> 8) & 0xFF) as u8);
        buf.push((self.mpls_label_or_vni & 0xFF) as u8);
        buf.extend_from_slice(&self.tunnel_endpoint.0);
        buf
    }

    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < 9 {
            return None;
        }
        let flags = buf[0];
        let tunnel_type = buf[1];
        let vni = ((buf[2] as u32) << 16) | ((buf[3] as u32) << 8) | (buf[4] as u32);
        let endpoint = Ipv4Address([buf[5], buf[6], buf[7], buf[8]]);
        Some(PmsiTunnelAttribute {
            flags,
            tunnel_type,
            mpls_label_or_vni: vni,
            tunnel_endpoint: endpoint,
        })
    }
}

/// EVPN Route Type 3: Inclusive Multicast Ethernet Tag Route (IMET)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvpnType3Route {
    pub rd: RouteDistinguisher,
    pub eth_tag: u32,
    pub ip_len: u8,
    pub originating_router_ip: Ipv4Address,
    pub pmsi: PmsiTunnelAttribute,
}

impl EvpnType3Route {
    pub fn new_ipv4(
        rd: RouteDistinguisher,
        eth_tag: u32,
        originating_router_ip: Ipv4Address,
        vni: u32,
    ) -> Self {
        let pmsi = PmsiTunnelAttribute::new_ingress_replication(vni, originating_router_ip);
        EvpnType3Route {
            rd,
            eth_tag,
            ip_len: 32,
            originating_router_ip,
            pmsi,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(26);
        buf.push(EVPN_ROUTE_TYPE_IMET);
        buf.push(17); // Length of following NLRI fields (8 + 4 + 1 + 4 = 17)
        buf.extend_from_slice(&self.rd.serialize());
        buf.extend_from_slice(&self.eth_tag.to_be_bytes());
        buf.push(self.ip_len);
        buf.extend_from_slice(&self.originating_router_ip.0);
        buf.extend_from_slice(&self.pmsi.serialize());
        buf
    }

    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < 28 || buf[0] != EVPN_ROUTE_TYPE_IMET {
            return None;
        }

        let rd = RouteDistinguisher::parse(&buf[2..10]).ok()?;
        let eth_tag = u32::from_be_bytes([buf[10], buf[11], buf[12], buf[13]]);
        let ip_len = buf[14];
        let originating_router_ip = Ipv4Address([buf[15], buf[16], buf[17], buf[18]]);
        let pmsi = PmsiTunnelAttribute::parse(&buf[19..])?;

        Some(EvpnType3Route {
            rd,
            eth_tag,
            ip_len,
            originating_router_ip,
            pmsi,
        })
    }
}

/// EVPN BUM (Broadcast, Unknown Unicast, Multicast) Flooding Table
#[derive(Debug, Clone, Default)]
pub struct EvpnBumFloodingTree {
    pub routes: Vec<EvpnType3Route>,
}

impl EvpnBumFloodingTree {
    pub fn new() -> Self {
        EvpnBumFloodingTree { routes: Vec::new() }
    }

    pub fn add_route(&mut self, route: EvpnType3Route) {
        self.routes.retain(|r| !(r.rd == route.rd && r.eth_tag == route.eth_tag && r.originating_router_ip == route.originating_router_ip));
        self.routes.push(route);
    }

    /// Retrieves all remote VTEP flooding endpoints registered for a specific VNI / Ethernet Tag
    pub fn get_flood_endpoints(&self, vni: u32, exclude_ip: Ipv4Address) -> Vec<Ipv4Address> {
        self.routes
            .iter()
            .filter(|r| r.pmsi.mpls_label_or_vni == vni && r.pmsi.tunnel_endpoint != exclude_ip)
            .map(|r| r.pmsi.tunnel_endpoint)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evpn_type3_serialization_and_parsing() {
        let rd = RouteDistinguisher::new(Ipv4Address::new(192, 0, 2, 1), 100);
        let route = EvpnType3Route::new_ipv4(
            rd,
            0,
            Ipv4Address::new(192, 0, 2, 1),
            10010,
        );

        let bytes = route.serialize();
        let parsed = EvpnType3Route::parse(&bytes).unwrap();
        assert_eq!(parsed, route);
    }

    #[test]
    fn test_evpn_bum_flooding_tree_endpoints() {
        let mut tree = EvpnBumFloodingTree::new();
        let local_ip = Ipv4Address::new(10, 0, 0, 1);
        let vtep2 = Ipv4Address::new(10, 0, 0, 2);
        let vtep3 = Ipv4Address::new(10, 0, 0, 3);

        tree.add_route(EvpnType3Route::new_ipv4(
            RouteDistinguisher::new(local_ip, 1),
            0,
            local_ip,
            20001,
        ));
        tree.add_route(EvpnType3Route::new_ipv4(
            RouteDistinguisher::new(vtep2, 1),
            0,
            vtep2,
            20001,
        ));
        tree.add_route(EvpnType3Route::new_ipv4(
            RouteDistinguisher::new(vtep3, 1),
            0,
            vtep3,
            20001,
        ));

        let flood_list = tree.get_flood_endpoints(20001, local_ip);
        assert_eq!(flood_list.len(), 2);
        assert!(flood_list.contains(&vtep2));
        assert!(flood_list.contains(&vtep3));
    }
}
