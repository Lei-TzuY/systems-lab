//! EVPN Route Type 5: IP Prefix Route with Overlay Index (RFC 9136 / RFC 7432).
//!
//! Provides inter-subnet tenant prefix advertisement across EVPN VXLAN/MPLS overlays,
//! supporting Gateway IP (GW-IP) and Ethernet Segment Identifier (ESI) overlay routing.

use crate::evpn::RouteDistinguisher;
use crate::ipv4::Ipv4Address;

pub const EVPN_ROUTE_TYPE_IP_PREFIX: u8 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvpnType5Route {
    pub rd: RouteDistinguisher,
    pub esi: [u8; 10],
    pub eth_tag: u32,
    pub ip_prefix: Ipv4Address,
    pub prefix_len: u8,
    pub gw_ip: Ipv4Address,
    pub label_or_vni: u32, // 24-bit VNI or 20-bit MPLS label
}

impl EvpnType5Route {
    pub fn new_ipv4(
        rd: RouteDistinguisher,
        ip_prefix: Ipv4Address,
        prefix_len: u8,
        gw_ip: Ipv4Address,
        label_or_vni: u32,
    ) -> Self {
        EvpnType5Route {
            rd,
            esi: [0u8; 10],
            eth_tag: 0,
            ip_prefix,
            prefix_len,
            gw_ip,
            label_or_vni,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(34);
        buf.push(EVPN_ROUTE_TYPE_IP_PREFIX);
        buf.push(32); // NLRI Length (approx 32 bytes for IPv4 Type 5)
        buf.extend_from_slice(&self.rd.serialize());
        buf.extend_from_slice(&self.esi);
        buf.extend_from_slice(&self.eth_tag.to_be_bytes());
        buf.push(self.prefix_len);
        buf.extend_from_slice(&self.ip_prefix.0);
        buf.extend_from_slice(&self.gw_ip.0);
        // 3-byte Label / VNI
        let label_bytes = self.label_or_vni.to_be_bytes();
        buf.extend_from_slice(&label_bytes[1..4]);
        buf
    }

    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < 34 || buf[0] != EVPN_ROUTE_TYPE_IP_PREFIX {
            return None;
        }

        let rd = RouteDistinguisher::parse(&buf[2..10]).ok()?;
        let mut esi = [0u8; 10];
        esi.copy_from_slice(&buf[10..20]);
        let eth_tag = u32::from_be_bytes([buf[20], buf[21], buf[22], buf[23]]);
        let prefix_len = buf[24];
        let ip_prefix = Ipv4Address::new(buf[25], buf[26], buf[27], buf[28]);
        let gw_ip = Ipv4Address::new(buf[29], buf[30], buf[31], buf[32]);
        let label_or_vni = u32::from_be_bytes([0, buf[33], buf[34], buf[35]]);

        Some(EvpnType5Route {
            rd,
            esi,
            eth_tag,
            ip_prefix,
            prefix_len,
            gw_ip,
            label_or_vni,
        })
    }
}

/// EVPN Type 5 Prefix Routing Information Base (RIB)
#[derive(Debug, Clone, Default)]
pub struct EvpnType5Rib {
    pub routes: Vec<EvpnType5Route>,
}

impl EvpnType5Rib {
    pub fn new() -> Self {
        EvpnType5Rib {
            routes: Vec::new(),
        }
    }

    pub fn add_route(&mut self, route: EvpnType5Route) {
        if let Some(pos) = self.routes.iter().position(|r| r.rd == route.rd && r.ip_prefix == route.ip_prefix && r.prefix_len == route.prefix_len) {
            self.routes[pos] = route;
        } else {
            self.routes.push(route);
        }
    }

    /// Performs Longest Prefix Match (LPM) on tenant IP
    pub fn lookup_lpm(&self, ip: Ipv4Address) -> Option<&EvpnType5Route> {
        let mut best_match: Option<(&EvpnType5Route, u8)> = None;

        for route in &self.routes {
            let mask = if route.prefix_len == 0 {
                0u32
            } else {
                !0u32 << (32 - route.prefix_len)
            };

            if (ip.to_u32() & mask) == (route.ip_prefix.to_u32() & mask) {
                if let Some((_, best_len)) = best_match {
                    if route.prefix_len > best_len {
                        best_match = Some((route, route.prefix_len));
                    }
                } else {
                    best_match = Some((route, route.prefix_len));
                }
            }
        }

        best_match.map(|(r, _)| r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evpn_type5_serialization_and_parsing() {
        let rd = RouteDistinguisher::new(Ipv4Address::new(192, 168, 1, 1), 100);
        let route = EvpnType5Route::new_ipv4(
            rd,
            Ipv4Address::new(10, 100, 0, 0),
            16,
            Ipv4Address::new(192, 168, 1, 254),
            50001, // L3VNI
        );

        let bytes = route.serialize();
        let parsed = EvpnType5Route::parse(&bytes).unwrap();
        assert_eq!(parsed, route);
    }

    #[test]
    fn test_evpn_type5_lpm_lookup() {
        let mut rib = EvpnType5Rib::new();
        let rd = RouteDistinguisher::new(Ipv4Address::new(10, 0, 0, 1), 1);

        rib.add_route(EvpnType5Route::new_ipv4(
            rd.clone(),
            Ipv4Address::new(10, 0, 0, 0),
            8,
            Ipv4Address::new(10, 0, 0, 1),
            10001,
        ));
        rib.add_route(EvpnType5Route::new_ipv4(
            rd,
            Ipv4Address::new(10, 20, 0, 0),
            16,
            Ipv4Address::new(10, 0, 0, 2),
            10002,
        ));

        let matched = rib.lookup_lpm(Ipv4Address::new(10, 20, 30, 40)).unwrap();
        assert_eq!(matched.prefix_len, 16);
        assert_eq!(matched.label_or_vni, 10002);
    }
}
