//! The VTEP: EVPN instances, local MAC learning, and the overlay forwarding
//! table the VXLAN data plane actually consults.
//!
//! A VTEP owns one or more [`EvpnInstance`]s, one per tenant broadcast domain:
//!
//! ```text
//!  access port ---\                            /--- VNI 5001, RD 10.0.0.1:5001
//!                  >-- EvpnInstance (VNI) ----<     import RT 65001:5001
//!  access port ---/                            \--- export RT 65001:5001
//! ```
//!
//! Two directions meet here and they are kept strictly apart.
//!
//! *Outwards*: a frame arriving on an access port teaches the instance a **local**
//! MAC, and [`Vtep::routes_to_originate`] turns every local MAC into an EVPN
//! Type 2 route for the BGP speaker to advertise.
//!
//! *Inwards*: [`Vtep::program_from_rib`] rebuilds all **remote** state from the
//! EVPN Loc-RIB, and nothing else may write it. That is the property the whole
//! phase rests on: remote forwarding state is a pure function of the EVPN
//! Loc-RIB, so a withdrawn route, a dead session and a moved host all remove the
//! old entry for the same reason, and none of them can leave one behind.

use crate::bgp_evpn::{EvpnLocRib, EvpnRoute, EvpnRouteKey, RouteTarget};
use crate::ethernet::MacAddress;
use crate::evpn::{EvpnInclusiveMulticast, EvpnMacIpAdv, EvpnNlri, RouteDistinguisher};
use crate::ipv4::Ipv4Address;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Largest number of local MAC addresses one EVPN instance will learn. A tenant
/// generating addresses on an access port must not be able to exhaust memory or
/// flood the control plane with advertisements.
pub const MAX_LOCAL_MACS_PER_INSTANCE: usize = 1_024;

/// A MAC learned on one of this VTEP's own access ports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalMac {
    pub mac: MacAddress,
    pub ip: Option<Ipv4Address>,
    pub access_interface: String,
    /// MAC Mobility sequence number for this binding (RFC 7432 section 15).
    pub sequence: u32,
}

/// A MAC learned through an EVPN Type 2 route from another VTEP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteMac {
    pub mac: MacAddress,
    pub ip: Option<Ipv4Address>,
    /// The VTEP that owns it, and the address tenant traffic is encapsulated to.
    pub vtep: Ipv4Address,
    pub sequence: u32,
    /// BGP neighbour the route arrived from, for diagnostics.
    pub learned_from: Ipv4Address,
}

/// How a frame leaving an access port should be forwarded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayDecision {
    /// The destination is on another access port of this same VTEP.
    Local { access_interface: String },
    /// The destination is behind a remote VTEP: encapsulate to exactly one place.
    Unicast { vni: u32, vtep: Ipv4Address },
    /// Broadcast, multicast, or a MAC no Type 2 route describes. Replicate to
    /// every VTEP that signalled participation with a Type 3 route.
    Flood { vni: u32, vteps: Vec<Ipv4Address> },
    /// Nothing to do: no instance, or nowhere to send it.
    Drop,
}

/// One tenant broadcast domain on this VTEP.
#[derive(Debug, Clone)]
pub struct EvpnInstance {
    pub vni: u32,
    pub rd: RouteDistinguisher,
    pub import_rts: BTreeSet<RouteTarget>,
    pub export_rts: BTreeSet<RouteTarget>,
    pub access_interfaces: BTreeSet<String>,
    pub local_macs: BTreeMap<MacAddress, LocalMac>,
    /// Rebuilt from the EVPN Loc-RIB on every program pass; never written directly.
    pub remote_macs: BTreeMap<MacAddress, RemoteMac>,
    /// Remote VTEPs that advertised a Type 3 route for this VNI, i.e. the
    /// ingress-replication list for BUM traffic.
    pub remote_vteps: BTreeSet<Ipv4Address>,
    /// Local MACs dropped because the instance hit its learning limit.
    pub learn_limit_hits: u64,
}

impl EvpnInstance {
    pub fn new(vni: u32, rd: RouteDistinguisher) -> Self {
        EvpnInstance {
            vni,
            rd,
            import_rts: BTreeSet::new(),
            export_rts: BTreeSet::new(),
            access_interfaces: BTreeSet::new(),
            local_macs: BTreeMap::new(),
            remote_macs: BTreeMap::new(),
            remote_vteps: BTreeSet::new(),
            learn_limit_hits: 0,
        }
    }

    /// True when this instance would import a route carrying `targets`.
    pub fn imports(&self, targets: &[RouteTarget]) -> bool {
        targets.iter().any(|rt| self.import_rts.contains(rt))
    }
}

/// A VXLAN tunnel endpoint driven by MP-BGP EVPN.
#[derive(Debug, Clone)]
pub struct Vtep {
    /// Source address of every VXLAN packet this VTEP sends, and the next hop it
    /// advertises in its own EVPN routes.
    pub source_ip: Ipv4Address,
    /// Interface the underlay is reached through.
    pub underlay_interface: String,
    pub instances: BTreeMap<u32, EvpnInstance>,
    /// Access interface name to VNI. An interface belongs to exactly one instance.
    access_to_vni: BTreeMap<String, u32>,
}

impl Vtep {
    pub fn new(source_ip: Ipv4Address, underlay_interface: &str) -> Self {
        Vtep {
            source_ip,
            underlay_interface: underlay_interface.to_string(),
            instances: BTreeMap::new(),
            access_to_vni: BTreeMap::new(),
        }
    }

    /// Configures a tenant instance. Re-adding an existing VNI updates its Route
    /// Targets and leaves learned state alone.
    pub fn add_instance(
        &mut self,
        vni: u32,
        rd: RouteDistinguisher,
        import_rts: &[RouteTarget],
        export_rts: &[RouteTarget],
    ) {
        let inst = self
            .instances
            .entry(vni)
            .or_insert_with(|| EvpnInstance::new(vni, rd.clone()));
        inst.rd = rd;
        inst.import_rts.extend(import_rts.iter().copied());
        inst.export_rts.extend(export_rts.iter().copied());
    }

    /// Puts an access interface into an instance.
    ///
    /// An interface already in another instance is moved, not duplicated: a port
    /// in two broadcast domains at once would bridge the tenants together.
    pub fn attach_access_port(&mut self, vni: u32, interface: &str) {
        if let Some(previous) = self.access_to_vni.insert(interface.to_string(), vni)
            && previous != vni
            && let Some(old) = self.instances.get_mut(&previous)
        {
            old.access_interfaces.remove(interface);
        }
        if let Some(inst) = self.instances.get_mut(&vni) {
            inst.access_interfaces.insert(interface.to_string());
        }
    }

    pub fn instance(&self, vni: u32) -> Option<&EvpnInstance> {
        self.instances.get(&vni)
    }

    pub fn instance_mut(&mut self, vni: u32) -> Option<&mut EvpnInstance> {
        self.instances.get_mut(&vni)
    }

    /// The VNI an access interface belongs to.
    pub fn vni_for_access(&self, interface: &str) -> Option<u32> {
        self.access_to_vni.get(interface).copied()
    }

    /// True when this VNI is configured here, i.e. a decapsulated packet for it
    /// has somewhere to go.
    pub fn has_vni(&self, vni: u32) -> bool {
        self.instances.contains_key(&vni)
    }

    /// Every Route Target any instance imports. This is what the BGP speaker
    /// filters its EVPN Adj-RIB-In on.
    pub fn all_import_rts(&self) -> BTreeSet<RouteTarget> {
        self.instances
            .values()
            .flat_map(|i| i.import_rts.iter().copied())
            .collect()
    }

    // ------------------------------------------------------------------
    // Local learning
    // ------------------------------------------------------------------

    /// Learns a MAC seen arriving on an access port.
    ///
    /// Returns true when this changed something, which is the signal to
    /// re-originate. A MAC currently known at a *remote* VTEP is being learned
    /// here because the host moved, so the new binding takes a sequence number
    /// one above the remote one - that is what makes every other speaker prefer
    /// this location (RFC 7432 section 15).
    pub fn learn_local(
        &mut self,
        interface: &str,
        mac: MacAddress,
        ip: Option<Ipv4Address>,
    ) -> bool {
        let Some(vni) = self.vni_for_access(interface) else {
            return false;
        };
        let Some(inst) = self.instances.get_mut(&vni) else {
            return false;
        };
        // A broadcast or multicast source address is not a host and must never
        // be advertised as one.
        if mac.is_broadcast() || mac.is_multicast() {
            return false;
        }

        let remote_seq = inst.remote_macs.get(&mac).map(|r| r.sequence);
        let existing = inst.local_macs.get(&mac);

        let sequence = match (existing, remote_seq) {
            // Already ours, and no remote claim: keep the sequence we have.
            (Some(l), None) => l.sequence,
            // Already ours and a remote claim exists: only climb above it,
            // never below, so a stale remote route cannot walk the number back.
            (Some(l), Some(r)) => l.sequence.max(r.saturating_add(1)),
            // New here, claimed elsewhere: the host moved to us.
            (None, Some(r)) => r.saturating_add(1),
            (None, None) => 0,
        };

        let binding = LocalMac {
            mac,
            // An advertisement that once carried a host IP keeps it when a later
            // frame happens to have none to offer; dropping it would withdraw
            // and re-add the route on every ARP-less packet.
            ip: ip.or_else(|| existing.and_then(|l| l.ip)),
            access_interface: interface.to_string(),
            sequence,
        };
        if existing == Some(&binding) {
            return false;
        }
        if existing.is_none() && inst.local_macs.len() >= MAX_LOCAL_MACS_PER_INSTANCE {
            inst.learn_limit_hits += 1;
            return false;
        }
        inst.local_macs.insert(mac, binding);
        true
    }

    /// Forgets a local MAC, e.g. because the host went away.
    pub fn forget_local(&mut self, vni: u32, mac: &MacAddress) -> bool {
        self.instances
            .get_mut(&vni)
            .is_some_and(|i| i.local_macs.remove(mac).is_some())
    }

    // ------------------------------------------------------------------
    // Origination
    // ------------------------------------------------------------------

    /// Every EVPN route this VTEP should be advertising right now: one Type 2 per
    /// local MAC and one Type 3 per instance.
    ///
    /// The Type 3 route is unconditional. It is how a leaf says "I am attached to
    /// this VNI" so the other leaves can put it in their ingress-replication list
    /// before any host has spoken.
    pub fn routes_to_originate(&self) -> Vec<EvpnRoute> {
        let mut out = Vec::new();
        for inst in self.instances.values() {
            if inst.export_rts.is_empty() {
                continue;
            }
            let export: Vec<RouteTarget> = inst.export_rts.iter().copied().collect();

            out.push(EvpnRoute::new(
                EvpnNlri::InclusiveMulticast(EvpnInclusiveMulticast {
                    rd: inst.rd.clone(),
                    // The Ethernet Tag identifies the broadcast domain, which in a
                    // VNI-per-instance fabric is the VNI itself.
                    eth_tag: inst.vni,
                    originating_router_ip: self.source_ip,
                }),
                self.source_ip,
                export.clone(),
            ));

            for local in inst.local_macs.values() {
                let route = EvpnRoute::new(
                    EvpnNlri::MacIpAdv(EvpnMacIpAdv {
                        rd: inst.rd.clone(),
                        esi: [0u8; 10],
                        eth_tag: 0,
                        mac: local.mac,
                        ip: local.ip,
                        vni: inst.vni & 0x00FF_FFFF,
                    }),
                    self.source_ip,
                    export.clone(),
                );
                // A sequence number of zero means "never moved", and RFC 7432
                // says the community is then simply absent.
                out.push(if local.sequence > 0 {
                    route.with_mobility(local.sequence)
                } else {
                    route
                });
            }
        }
        out
    }

    // ------------------------------------------------------------------
    // Programming from the control plane
    // ------------------------------------------------------------------

    /// Rebuilds every instance's remote MAC table and ingress-replication list
    /// from the EVPN Loc-RIB.
    ///
    /// Returns the keys of any locally originated routes that must now be
    /// withdrawn, which happens when a host this VTEP was advertising has turned
    /// up somewhere else with a higher mobility sequence.
    ///
    /// The tables are cleared and refilled rather than incrementally patched.
    /// That is what makes withdrawal, session loss and mobility all correct by
    /// construction: whatever is no longer in the Loc-RIB is, a moment later, no
    /// longer in the data plane either.
    pub fn program_from_rib(&mut self, rib: &EvpnLocRib) -> Vec<EvpnRouteKey> {
        for inst in self.instances.values_mut() {
            inst.remote_macs.clear();
            inst.remote_vteps.clear();
        }

        for path in rib.remote_paths() {
            let route = &path.route;
            let vni = route.vni();
            let Some(inst) = self.instances.get_mut(&vni) else {
                continue;
            };
            // Two independent conditions, and both must hold.
            //
            // The Route Target says which tenant asked for this route. The VNI
            // says which broadcast domain the sender put it in. Accepting a route
            // whose RT matches but whose VNI names a different instance would let
            // a neighbour inject a MAC into a tenant it has no business reaching,
            // so they are required to agree.
            if !inst.imports(&route.route_targets) {
                continue;
            }
            // A route naming this VTEP as its own next hop describes a host that
            // is supposed to be local; tunnelling to ourselves would loop.
            if route.next_hop == self.source_ip {
                continue;
            }

            match route.mac() {
                Some(mac) => {
                    inst.remote_macs.insert(
                        mac,
                        RemoteMac {
                            mac,
                            ip: route.host_ip(),
                            vtep: route.next_hop,
                            sequence: route.mobility_seq.unwrap_or(0),
                            learned_from: path.peer_addr,
                        },
                    );
                    // A Type 2 route also proves the far end is in this VNI, so it
                    // joins the flood list even if its Type 3 has not arrived yet.
                    inst.remote_vteps.insert(route.next_hop);
                }
                None => {
                    inst.remote_vteps.insert(route.next_hop);
                }
            }
        }

        // A host that has appeared behind another VTEP with a strictly higher
        // sequence number has moved away from here. The local binding goes, and
        // the route this VTEP was originating for it has to be withdrawn, or two
        // leaves would keep claiming the same MAC forever.
        let mut withdraw = Vec::new();
        for inst in self.instances.values_mut() {
            let moved: Vec<MacAddress> = inst
                .local_macs
                .iter()
                .filter(|(mac, local)| {
                    inst.remote_macs
                        .get(*mac)
                        .is_some_and(|r| r.sequence > local.sequence)
                })
                .map(|(mac, _)| *mac)
                .collect();
            for mac in moved {
                if let Some(local) = inst.local_macs.remove(&mac) {
                    withdraw.push(EvpnRouteKey::MacIp {
                        rd: (inst.rd.admin, inst.rd.assigned),
                        eth_tag: 0,
                        mac: local.mac,
                        ip: local.ip,
                    });
                }
            }
        }
        withdraw
    }

    // ------------------------------------------------------------------
    // Forwarding
    // ------------------------------------------------------------------

    /// Decides what to do with a tenant frame that arrived on `interface`.
    ///
    /// A destination with a Type 2 route is sent to exactly one VTEP. Flooding is
    /// what happens when the control plane genuinely does not know the
    /// destination, which is the point of running EVPN in the first place.
    pub fn forward(&self, interface: &str, dst: MacAddress) -> OverlayDecision {
        let Some(vni) = self.vni_for_access(interface) else {
            return OverlayDecision::Drop;
        };
        let Some(inst) = self.instances.get(&vni) else {
            return OverlayDecision::Drop;
        };

        if dst.is_broadcast() || dst.is_multicast() {
            return self.flood(inst, Some(interface));
        }

        if let Some(local) = inst.local_macs.get(&dst)
            && local.access_interface != interface
        {
            return OverlayDecision::Local {
                access_interface: local.access_interface.clone(),
            };
        }

        match inst.remote_macs.get(&dst) {
            Some(remote) => OverlayDecision::Unicast {
                vni,
                vtep: remote.vtep,
            },
            None => self.flood(inst, Some(interface)),
        }
    }

    /// Where a decapsulated frame for `vni` should be delivered locally.
    pub fn access_ports_for(&self, vni: u32, dst: MacAddress) -> Vec<String> {
        let Some(inst) = self.instances.get(&vni) else {
            return Vec::new();
        };
        if !dst.is_broadcast()
            && !dst.is_multicast()
            && let Some(local) = inst.local_macs.get(&dst)
        {
            return vec![local.access_interface.clone()];
        }
        inst.access_interfaces.iter().cloned().collect()
    }

    fn flood(&self, inst: &EvpnInstance, _ingress: Option<&str>) -> OverlayDecision {
        let vteps: Vec<Ipv4Address> = inst.remote_vteps.iter().copied().collect();
        if vteps.is_empty() {
            return OverlayDecision::Drop;
        }
        OverlayDecision::Flood {
            vni: inst.vni,
            vteps,
        }
    }

    /// The remote VTEP a MAC lives behind, if the control plane knows.
    pub fn lookup_remote(&self, vni: u32, mac: &MacAddress) -> Option<Ipv4Address> {
        self.instances
            .get(&vni)
            .and_then(|i| i.remote_macs.get(mac))
            .map(|r| r.vtep)
    }

    /// Total remote MAC entries across every instance.
    pub fn remote_mac_count(&self) -> usize {
        self.instances.values().map(|i| i.remote_macs.len()).sum()
    }

    pub fn local_mac_count(&self) -> usize {
        self.instances.values().map(|i| i.local_macs.len()).sum()
    }
}

impl fmt::Display for Vtep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "VTEP source {} underlay {}",
            self.source_ip, self.underlay_interface
        )?;
        for inst in self.instances.values() {
            let imports: Vec<String> = inst.import_rts.iter().map(|r| r.to_string()).collect();
            let exports: Vec<String> = inst.export_rts.iter().map(|r| r.to_string()).collect();
            writeln!(
                f,
                "  VNI {}  RD {}  import [{}]  export [{}]  access [{}]",
                inst.vni,
                inst.rd,
                imports.join(", "),
                exports.join(", "),
                inst.access_interfaces
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bgp_evpn::EvpnPath;

    fn ip(a: u8, b: u8, c: u8, d: u8) -> Ipv4Address {
        Ipv4Address::new(a, b, c, d)
    }

    fn mac(last: u8) -> MacAddress {
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x00, last])
    }

    fn rt(value: u32) -> RouteTarget {
        RouteTarget::as2(65001, value)
    }

    fn leaf(source: Ipv4Address) -> Vtep {
        let mut v = Vtep::new(source, "eth1");
        v.add_instance(
            5001,
            RouteDistinguisher::new(source, 5001),
            &[rt(5001)],
            &[rt(5001)],
        );
        v.attach_access_port(5001, "eth0");
        v
    }

    /// Builds the Loc-RIB a leaf would have after importing `routes` from a peer.
    fn rib_from(routes: Vec<EvpnRoute>, peer: Ipv4Address) -> EvpnLocRib {
        let mut rib = EvpnLocRib::new();
        for route in routes {
            let mut path = EvpnPath::local(route, peer, 0);
            path.local = false;
            path.peer_addr = peer;
            rib.insert(path);
        }
        rib
    }

    #[test]
    fn test_a_local_mac_produces_a_type_2_and_a_type_3_route() {
        let mut v = leaf(ip(10, 0, 0, 1));
        assert!(v.learn_local("eth0", mac(0xAA), Some(ip(192, 168, 10, 11))));

        let routes = v.routes_to_originate();
        assert_eq!(routes.len(), 2);
        assert!(routes.iter().any(|r| matches!(
            r.nlri,
            EvpnNlri::InclusiveMulticast(ref im) if im.eth_tag == 5001
        )));
        let t2 = routes
            .iter()
            .find(|r| r.mac() == Some(mac(0xAA)))
            .expect("a Type 2 route for the learned MAC");
        assert_eq!(t2.next_hop, ip(10, 0, 0, 1));
        assert_eq!(t2.host_ip(), Some(ip(192, 168, 10, 11)));
        assert_eq!(t2.route_targets, vec![rt(5001)]);
        assert_eq!(t2.mobility_seq, None);
    }

    #[test]
    fn test_relearning_the_same_mac_is_not_a_change() {
        let mut v = leaf(ip(10, 0, 0, 1));
        assert!(v.learn_local("eth0", mac(0xAA), None));
        assert!(!v.learn_local("eth0", mac(0xAA), None));
    }

    #[test]
    fn test_a_broadcast_source_is_never_learned() {
        let mut v = leaf(ip(10, 0, 0, 1));
        assert!(!v.learn_local("eth0", MacAddress::BROADCAST, None));
        assert_eq!(v.local_mac_count(), 0);
    }

    #[test]
    fn test_learning_is_bounded() {
        let mut v = leaf(ip(10, 0, 0, 1));
        for i in 0..(MAX_LOCAL_MACS_PER_INSTANCE + 50) {
            let m = MacAddress([2, 0, (i >> 16) as u8, (i >> 8) as u8, i as u8, 1]);
            v.learn_local("eth0", m, None);
        }
        assert_eq!(v.local_mac_count(), MAX_LOCAL_MACS_PER_INSTANCE);
        assert!(v.instance(5001).unwrap().learn_limit_hits > 0);
    }

    #[test]
    fn test_a_remote_type_2_programs_unicast_forwarding() {
        let mut v = leaf(ip(10, 0, 0, 1));
        v.learn_local("eth0", mac(0xAA), None);

        let remote = EvpnRoute::new(
            EvpnNlri::build_mac_ip(
                RouteDistinguisher::new(ip(10, 0, 0, 2), 5001),
                mac(0xBB),
                Some(ip(192, 168, 10, 22)),
                5001,
            ),
            ip(10, 0, 0, 2),
            vec![rt(5001)],
        );
        assert!(
            v.program_from_rib(&rib_from(vec![remote], ip(10, 12, 0, 2)))
                .is_empty()
        );

        assert_eq!(v.lookup_remote(5001, &mac(0xBB)), Some(ip(10, 0, 0, 2)));
        assert_eq!(
            v.forward("eth0", mac(0xBB)),
            OverlayDecision::Unicast {
                vni: 5001,
                vtep: ip(10, 0, 0, 2)
            }
        );
    }

    #[test]
    fn test_a_route_target_that_does_not_match_programs_nothing() {
        let mut v = leaf(ip(10, 0, 0, 1));
        let foreign = EvpnRoute::new(
            EvpnNlri::build_mac_ip(
                RouteDistinguisher::new(ip(10, 0, 0, 2), 5002),
                mac(0xBB),
                None,
                5001,
            ),
            ip(10, 0, 0, 2),
            vec![rt(5002)],
        );
        v.program_from_rib(&rib_from(vec![foreign], ip(10, 12, 0, 2)));
        assert_eq!(v.remote_mac_count(), 0);
    }

    #[test]
    fn test_a_matching_rt_with_the_wrong_vni_is_not_imported() {
        // Defence against a neighbour reusing a valid RT to inject a MAC into a
        // broadcast domain the label says it does not belong to.
        let mut v = leaf(ip(10, 0, 0, 1));
        let mismatched = EvpnRoute::new(
            EvpnNlri::build_mac_ip(
                RouteDistinguisher::new(ip(10, 0, 0, 2), 5001),
                mac(0xBB),
                None,
                6001,
            ),
            ip(10, 0, 0, 2),
            vec![rt(5001)],
        );
        v.program_from_rib(&rib_from(vec![mismatched], ip(10, 12, 0, 2)));
        assert_eq!(v.remote_mac_count(), 0);
    }

    #[test]
    fn test_an_empty_rib_removes_every_remote_entry() {
        let mut v = leaf(ip(10, 0, 0, 1));
        let remote = EvpnRoute::new(
            EvpnNlri::build_mac_ip(
                RouteDistinguisher::new(ip(10, 0, 0, 2), 5001),
                mac(0xBB),
                None,
                5001,
            ),
            ip(10, 0, 0, 2),
            vec![rt(5001)],
        );
        v.program_from_rib(&rib_from(vec![remote], ip(10, 12, 0, 2)));
        assert_eq!(v.remote_mac_count(), 1);

        v.program_from_rib(&EvpnLocRib::new());
        assert_eq!(v.remote_mac_count(), 0);
        assert_eq!(v.lookup_remote(5001, &mac(0xBB)), None);
    }

    #[test]
    fn test_type_3_alone_builds_the_ingress_replication_list() {
        let mut v = leaf(ip(10, 0, 0, 1));
        v.learn_local("eth0", mac(0xAA), None);
        let imet = EvpnRoute::new(
            EvpnNlri::InclusiveMulticast(EvpnInclusiveMulticast {
                rd: RouteDistinguisher::new(ip(10, 0, 0, 2), 5001),
                eth_tag: 5001,
                originating_router_ip: ip(10, 0, 0, 2),
            }),
            ip(10, 0, 0, 2),
            vec![rt(5001)],
        );
        v.program_from_rib(&rib_from(vec![imet], ip(10, 12, 0, 2)));

        // An unknown unicast destination floods to the Type 3 list.
        assert_eq!(
            v.forward("eth0", mac(0xCC)),
            OverlayDecision::Flood {
                vni: 5001,
                vteps: vec![ip(10, 0, 0, 2)]
            }
        );
        assert_eq!(
            v.forward("eth0", MacAddress::BROADCAST),
            OverlayDecision::Flood {
                vni: 5001,
                vteps: vec![ip(10, 0, 0, 2)]
            }
        );
    }

    #[test]
    fn test_a_higher_sequence_elsewhere_evicts_the_local_binding() {
        let mut v = leaf(ip(10, 0, 0, 1));
        v.learn_local("eth0", mac(0xAA), None);
        assert_eq!(v.local_mac_count(), 1);

        let moved = EvpnRoute::new(
            EvpnNlri::build_mac_ip(
                RouteDistinguisher::new(ip(10, 0, 0, 2), 5001),
                mac(0xAA),
                None,
                5001,
            ),
            ip(10, 0, 0, 2),
            vec![rt(5001)],
        )
        .with_mobility(1);

        let withdraw = v.program_from_rib(&rib_from(vec![moved], ip(10, 12, 0, 2)));
        assert_eq!(withdraw.len(), 1);
        assert_eq!(v.local_mac_count(), 0);
        assert_eq!(v.lookup_remote(5001, &mac(0xAA)), Some(ip(10, 0, 0, 2)));
    }

    #[test]
    fn test_learning_a_mac_that_is_remote_climbs_above_its_sequence() {
        let mut v = leaf(ip(10, 0, 0, 2));
        let elsewhere = EvpnRoute::new(
            EvpnNlri::build_mac_ip(
                RouteDistinguisher::new(ip(10, 0, 0, 1), 5001),
                mac(0xAA),
                None,
                5001,
            ),
            ip(10, 0, 0, 1),
            vec![rt(5001)],
        )
        .with_mobility(3);
        v.program_from_rib(&rib_from(vec![elsewhere], ip(10, 12, 0, 1)));

        assert!(v.learn_local("eth0", mac(0xAA), None));
        let t2 = v
            .routes_to_originate()
            .into_iter()
            .find(|r| r.mac() == Some(mac(0xAA)))
            .unwrap();
        assert_eq!(t2.mobility_seq, Some(4));
    }

    #[test]
    fn test_two_vnis_keep_the_same_mac_apart() {
        let mut v = Vtep::new(ip(10, 0, 0, 1), "eth2");
        v.add_instance(
            5001,
            RouteDistinguisher::new(ip(10, 0, 0, 1), 5001),
            &[rt(5001)],
            &[rt(5001)],
        );
        v.add_instance(
            5002,
            RouteDistinguisher::new(ip(10, 0, 0, 1), 5002),
            &[rt(5002)],
            &[rt(5002)],
        );
        v.attach_access_port(5001, "eth0");
        v.attach_access_port(5002, "eth1");

        let shared = mac(0xBB);
        let in_5001 = EvpnRoute::new(
            EvpnNlri::build_mac_ip(
                RouteDistinguisher::new(ip(10, 0, 0, 2), 5001),
                shared,
                None,
                5001,
            ),
            ip(10, 0, 0, 2),
            vec![rt(5001)],
        );
        let in_5002 = EvpnRoute::new(
            EvpnNlri::build_mac_ip(
                RouteDistinguisher::new(ip(10, 0, 0, 3), 5002),
                shared,
                None,
                5002,
            ),
            ip(10, 0, 0, 3),
            vec![rt(5002)],
        );
        v.program_from_rib(&rib_from(vec![in_5001, in_5002], ip(10, 12, 0, 9)));

        assert_eq!(v.lookup_remote(5001, &shared), Some(ip(10, 0, 0, 2)));
        assert_eq!(v.lookup_remote(5002, &shared), Some(ip(10, 0, 0, 3)));
    }

    #[test]
    fn test_a_route_pointing_back_at_this_vtep_is_ignored() {
        let mut v = leaf(ip(10, 0, 0, 1));
        let mirror = EvpnRoute::new(
            EvpnNlri::build_mac_ip(
                RouteDistinguisher::new(ip(10, 0, 0, 1), 5001),
                mac(0xAA),
                None,
                5001,
            ),
            ip(10, 0, 0, 1),
            vec![rt(5001)],
        );
        v.program_from_rib(&rib_from(vec![mirror], ip(10, 12, 0, 2)));
        assert_eq!(v.remote_mac_count(), 0);
    }

    #[test]
    fn test_an_access_port_moved_between_instances_leaves_the_first() {
        let mut v = Vtep::new(ip(10, 0, 0, 1), "eth2");
        v.add_instance(
            5001,
            RouteDistinguisher::new(ip(10, 0, 0, 1), 5001),
            &[rt(5001)],
            &[rt(5001)],
        );
        v.add_instance(
            5002,
            RouteDistinguisher::new(ip(10, 0, 0, 1), 5002),
            &[rt(5002)],
            &[rt(5002)],
        );
        v.attach_access_port(5001, "eth0");
        v.attach_access_port(5002, "eth0");

        assert_eq!(v.vni_for_access("eth0"), Some(5002));
        assert!(!v.instance(5001).unwrap().access_interfaces.contains("eth0"));
        assert!(v.instance(5002).unwrap().access_interfaces.contains("eth0"));
    }
}
