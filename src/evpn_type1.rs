//! EVPN Route Type 1 Ethernet Auto-Discovery (A-D) Route (RFC 7432).
//!
//! Implements Ethernet A-D per ES and per EVI routes for multi-homing aliasing,
//! backup paths, and sub-millisecond fast mass withdrawal upon link failures.

use crate::evpn::RouteDistinguisher;
use crate::ipv4::Ipv4Address;
use std::collections::HashMap;

pub const EVPN_ROUTE_TYPE_ETHERNET_AD: u8 = 1;
pub const ETHERNET_TAG_MAX_PER_ES: u32 = 0xFFFFFFFF;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvpnEthernetAdRoute {
    pub rd: RouteDistinguisher,
    pub esi: [u8; 10],
    pub ethernet_tag_id: u32, // 0xFFFFFFFF for per-ES, or specific EVI tag
    pub mpls_label: u32,
    pub next_hop: Ipv4Address,
}

impl EvpnEthernetAdRoute {
    pub fn new_per_es(rd: RouteDistinguisher, esi: [u8; 10], next_hop: Ipv4Address) -> Self {
        EvpnEthernetAdRoute {
            rd,
            esi,
            ethernet_tag_id: ETHERNET_TAG_MAX_PER_ES,
            mpls_label: 0,
            next_hop,
        }
    }

    pub fn new_per_evi(
        rd: RouteDistinguisher,
        esi: [u8; 10],
        ethernet_tag_id: u32,
        mpls_label: u32,
        next_hop: Ipv4Address,
    ) -> Self {
        EvpnEthernetAdRoute {
            rd,
            esi,
            ethernet_tag_id,
            mpls_label,
            next_hop,
        }
    }

    pub fn is_per_es(&self) -> bool {
        self.ethernet_tag_id == ETHERNET_TAG_MAX_PER_ES
    }
}

/// EVPN Aliasing & Fast Mass Withdrawal Engine
#[derive(Debug, Clone, Default)]
pub struct EvpnAliasingEngine {
    /// ESI -> List of active multi-homed PE Next-Hops advertising A-D per ES
    pub active_es_nexthops: HashMap<[u8; 10], Vec<Ipv4Address>>,
}

impl EvpnAliasingEngine {
    pub fn new() -> Self {
        EvpnAliasingEngine {
            active_es_nexthops: HashMap::new(),
        }
    }

    /// Ingests an incoming Type 1 Ethernet A-D per ES route (Enables Aliasing / ECMP)
    pub fn add_ad_route(&mut self, route: EvpnEthernetAdRoute) {
        if route.is_per_es() {
            let nexthops = self.active_es_nexthops.entry(route.esi).or_default();
            if !nexthops.contains(&route.next_hop) {
                nexthops.push(route.next_hop);
            }
        }
    }

    /// Processes a Fast Mass Withdrawal for an ESI when a PE access link fails
    pub fn mass_withdraw(&mut self, esi: &[u8; 10], failed_pe: Ipv4Address) -> usize {
        if let Some(nexthops) = self.active_es_nexthops.get_mut(esi) {
            let before = nexthops.len();
            nexthops.retain(|&nh| nh != failed_pe);
            before - nexthops.len()
        } else {
            0
        }
    }

    /// Returns active ECMP Next-Hops for aliasing load-balancing across multi-homed PEs
    pub fn get_aliasing_nexthops(&self, esi: &[u8; 10]) -> &[Ipv4Address] {
        self.active_es_nexthops.get(esi).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evpn_type1_ad_aliasing_and_mass_withdrawal() {
        let mut engine = EvpnAliasingEngine::new();
        let esi = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99];

        let pe1 = Ipv4Address::new(192, 168, 1, 10);
        let pe2 = Ipv4Address::new(192, 168, 1, 20);

        // PE1 and PE2 advertise Type 1 A-D per ES routes for the same multi-homed ESI
        let ad_pe1 = EvpnEthernetAdRoute::new_per_es(RouteDistinguisher::new(pe1, 1), esi, pe1);
        let ad_pe2 = EvpnEthernetAdRoute::new_per_es(RouteDistinguisher::new(pe2, 1), esi, pe2);

        engine.add_ad_route(ad_pe1);
        engine.add_ad_route(ad_pe2);

        // Aliasing: Remote node can load balance to both PE1 and PE2
        let nexthops = engine.get_aliasing_nexthops(&esi);
        assert_eq!(nexthops.len(), 2);
        assert!(nexthops.contains(&pe1));
        assert!(nexthops.contains(&pe2));

        // PE1 suffers local link failure -> Fast Mass Withdrawal
        let withdrawn = engine.mass_withdraw(&esi, pe1);
        assert_eq!(withdrawn, 1);

        // Instant failover: Only PE2 remains active for all MACs on this ESI
        let updated_nexthops = engine.get_aliasing_nexthops(&esi);
        assert_eq!(updated_nexthops.len(), 1);
        assert_eq!(updated_nexthops[0], pe2);
    }
}
