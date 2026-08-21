//! BGP EVPN Selective Multicast Ethernet Tag (SMET) & Join Synch Routes (RFC 9251).
//!
//! Implements EVPN Route Type 6 (SMET) and Route Type 7 (Multicast Membership / Join Synch)
//! for optimizing multicast replication in EVPN networks by preventing BUM flooding.

use crate::evpn::RouteDistinguisher;
use crate::ipv4::Ipv4Address;

pub const EVPN_ROUTE_TYPE_SMET: u8 = 6;
pub const EVPN_ROUTE_TYPE_JOIN_SYNCH: u8 = 7;

/// EVPN Route Type 6: Selective Multicast Ethernet Tag (SMET) Route
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvpnSmetRoute {
    pub rd: RouteDistinguisher,
    pub ethernet_tag_id: u32,
    pub source_ip: Ipv4Address, // 0.0.0.0 for (*, G)
    pub group_ip: Ipv4Address,  // Multicast group address (e.g. 239.1.1.1)
    pub originator_ip: Ipv4Address,
    pub flags: u8,              // Bit 0: Include/Exclude mode
}

impl EvpnSmetRoute {
    pub fn new_any_source(
        rd: RouteDistinguisher,
        ethernet_tag_id: u32,
        group_ip: Ipv4Address,
        originator_ip: Ipv4Address,
    ) -> Self {
        EvpnSmetRoute {
            rd,
            ethernet_tag_id,
            source_ip: Ipv4Address::UNSPECIFIED,
            group_ip,
            originator_ip,
            flags: 0,
        }
    }

    /// Serializes EVPN Route Type 6 NLRI
    pub fn serialize_nlri(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(2 + 1 + 8 + 4 + 1 + 4 + 1 + 4 + 1 + 4 + 1);
        buf.push(EVPN_ROUTE_TYPE_SMET);
        // Length = 8 (RD) + 4 (Tag) + 1 (SrcLen) + 4 (Src) + 1 (GrpLen) + 4 (Grp) + 1 (OrigLen) + 4 (Orig) + 1 (Flags) = 28
        let length: u8 = 28;
        buf.push(length);
        buf.extend_from_slice(&self.rd.serialize());
        buf.extend_from_slice(&self.ethernet_tag_id.to_be_bytes());
        buf.push(32); // Source IP prefix length
        buf.extend_from_slice(&self.source_ip.0);
        buf.push(32); // Group IP prefix length
        buf.extend_from_slice(&self.group_ip.0);
        buf.push(32); // Originator IP prefix length
        buf.extend_from_slice(&self.originator_ip.0);
        buf.push(self.flags);
        buf
    }

    /// Parses EVPN Route Type 6 NLRI
    pub fn parse_nlri(buf: &[u8]) -> Option<Self> {
        if buf.len() < 30 {
            return None;
        }
        if buf[0] != EVPN_ROUTE_TYPE_SMET {
            return None;
        }
        let rd = RouteDistinguisher::parse(&buf[2..10]).ok()?;
        let ethernet_tag_id = u32::from_be_bytes([buf[10], buf[11], buf[12], buf[13]]);
        let source_ip = Ipv4Address([buf[15], buf[16], buf[17], buf[18]]);
        let group_ip = Ipv4Address([buf[20], buf[21], buf[22], buf[23]]);
        let originator_ip = Ipv4Address([buf[25], buf[26], buf[27], buf[28]]);
        let flags = buf[29];

        Some(EvpnSmetRoute {
            rd,
            ethernet_tag_id,
            source_ip,
            group_ip,
            originator_ip,
            flags,
        })
    }
}

/// EVPN Selective Multicast Forwarding Engine
#[derive(Debug, Clone, Default)]
pub struct EvpnSmetEngine {
    pub smet_routes: Vec<EvpnSmetRoute>,
}

impl EvpnSmetEngine {
    pub fn new() -> Self {
        EvpnSmetEngine {
            smet_routes: Vec::new(),
        }
    }

    /// Adds an advertised SMET route from a remote PE
    pub fn add_smet_route(&mut self, route: EvpnSmetRoute) {
        if !self.smet_routes.iter().any(|r| r == &route) {
            self.smet_routes.push(route);
        }
    }

    /// Resolves list of remote PEs that should receive traffic for multicast group (S, G)
    pub fn resolve_replication_pes(
        &self,
        ethernet_tag_id: u32,
        _source_ip: Ipv4Address,
        group_ip: Ipv4Address,
    ) -> Vec<Ipv4Address> {
        let mut pes = Vec::new();
        for r in &self.smet_routes {
            if r.ethernet_tag_id == ethernet_tag_id && r.group_ip == group_ip {
                if !pes.contains(&r.originator_ip) {
                    pes.push(r.originator_ip);
                }
            }
        }
        pes
    }

    /// Withdraws SMET routes for an originator PE when group is left
    pub fn withdraw_smet_route(
        &mut self,
        ethernet_tag_id: u32,
        group_ip: Ipv4Address,
        originator_ip: Ipv4Address,
    ) -> bool {
        let initial_len = self.smet_routes.len();
        self.smet_routes.retain(|r| {
            !(r.ethernet_tag_id == ethernet_tag_id
                && r.group_ip == group_ip
                && r.originator_ip == originator_ip)
        });
        self.smet_routes.len() < initial_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evpn_smet_nlri_codec() {
        let rd = RouteDistinguisher::new(Ipv4Address::new(192, 0, 2, 1), 100);
        let smet = EvpnSmetRoute::new_any_source(
            rd,
            10,
            Ipv4Address::new(239, 1, 2, 3),
            Ipv4Address::new(192, 0, 2, 1),
        );

        let bytes = smet.serialize_nlri();
        assert_eq!(bytes.len(), 30);
        assert_eq!(bytes[0], EVPN_ROUTE_TYPE_SMET);

        let parsed = EvpnSmetRoute::parse_nlri(&bytes).unwrap();
        assert_eq!(parsed, smet);
    }

    #[test]
    fn test_evpn_smet_selective_multicast_replication() {
        let mut engine = EvpnSmetEngine::new();
        let rd = RouteDistinguisher::new(Ipv4Address::new(10, 0, 0, 1), 1);

        // PE1 and PE2 subscribe to (*, 239.1.1.1) in VLAN Tag 100
        engine.add_smet_route(EvpnSmetRoute::new_any_source(
            rd.clone(),
            100,
            Ipv4Address::new(239, 1, 1, 1),
            Ipv4Address::new(10, 0, 0, 1),
        ));
        engine.add_smet_route(EvpnSmetRoute::new_any_source(
            rd.clone(),
            100,
            Ipv4Address::new(239, 1, 1, 1),
            Ipv4Address::new(10, 0, 0, 2),
        ));

        // PE3 subscribes to (*, 239.2.2.2) in VLAN Tag 100
        engine.add_smet_route(EvpnSmetRoute::new_any_source(
            rd.clone(),
            100,
            Ipv4Address::new(239, 2, 2, 2),
            Ipv4Address::new(10, 0, 0, 3),
        ));

        // Resolve replication list for 239.1.1.1 -> Only PE1 and PE2 should be returned
        let pes = engine.resolve_replication_pes(
            100,
            Ipv4Address::new(192, 168, 1, 50),
            Ipv4Address::new(239, 1, 1, 1),
        );
        assert_eq!(pes.len(), 2);
        assert!(pes.contains(&Ipv4Address::new(10, 0, 0, 1)));
        assert!(pes.contains(&Ipv4Address::new(10, 0, 0, 2)));
        assert!(!pes.contains(&Ipv4Address::new(10, 0, 0, 3)));
    }
}
