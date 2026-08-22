//! The MP-BGP EVPN control plane: Route Targets, EVPN RIBs, and the decision
//! process that turns received UPDATEs into overlay forwarding state.
//!
//! This is the layer between the BGP speaker and the VXLAN data plane:
//!
//! ```text
//! MP_REACH_NLRI (AFI 25 / SAFI 70)
//!        -> EvpnAdjRibIn        one table per peer, as advertised
//!        -> Route Target import only routes a local VNI asked for
//!        -> EvpnLocRib          best path per (RD, Ethernet Tag, MAC, IP)
//!        -> Vtep                (VNI, MAC) -> remote VTEP
//! ```
//!
//! Two properties are deliberate and are what keep the overlay honest.
//!
//! First, the Loc-RIB is recomputed from the Adj-RIB-In tables rather than
//! patched. A peer going down, a route being withdrawn, and a host moving are
//! then the same operation - the input set changed - so none of them can leave a
//! stale entry behind.
//!
//! Second, three things that are easy to conflate are kept apart: a route that
//! was *received*, a route this speaker *locally imports*, and a route that is
//! *eligible for advertisement*. On an ordinary leaf they coincide, because a
//! route no local instance asked for is dropped at the edge of the Adj-RIB-In and
//! that is what makes two VNIs on the same pair of leaves genuinely separate.
//! On a route reflector they do not: the reflector owns no tenant, imports no
//! tenant Route Target, and must still retain every route it hears so it can
//! reflect it. `EvpnLocRib` therefore holds only what this speaker imports and
//! programs, while the advertisement RIB holds the best path per route
//! regardless of Route Target.

use crate::bgp::{AsPath, BGP_DEFAULT_LOCAL_PREF, BgpOrigin, BgpParseError};
use crate::bgp_ext_comm::{
    BGP_EXT_COMM_SUBTYPE_ROUTE_TARGET, BGP_EXT_COMM_TYPE_2OCTET_AS, BGP_EXT_COMM_TYPE_IPV4_ADDR,
    BgpExtendedCommunity,
};
use crate::ethernet::MacAddress;
use crate::evpn::{
    EVPN_TYPE_INCLUSIVE_MULTICAST, EVPN_TYPE_MAC_IP_ADV, EvpnError, EvpnNlri, RouteDistinguisher,
};
use crate::ipv4::Ipv4Address;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Upper bound on EVPN routes one peer may hold in its Adj-RIB-In. A neighbour
/// advertising MAC addresses forever must not be able to exhaust memory.
pub const MAX_EVPN_ROUTES: usize = 4_096;

/// Largest number of NLRI decoded from a single MP attribute, so a malformed
/// length field cannot turn into an unbounded loop.
pub const MAX_EVPN_NLRI_PER_UPDATE: usize = 512;

// ============================================================================
// Route Targets
// ============================================================================

/// A Route Target: the Extended Community that says which VRF or EVPN instance a
/// route belongs to.
///
/// Kept as its own ordered type rather than as a raw [`BgpExtendedCommunity`] so
/// an import set can be a `BTreeSet` and "does this route match" is a set
/// intersection rather than a linear scan over a mixed community list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RouteTarget {
    /// `asn:value`, the usual `65001:5001` form.
    As2 { asn: u16, value: u32 },
    /// `ip:value`, used where an operator prefers to key on a loopback.
    Ipv4 { ip: Ipv4Address, value: u16 },
}

impl RouteTarget {
    pub fn as2(asn: u16, value: u32) -> Self {
        RouteTarget::As2 { asn, value }
    }

    pub fn ipv4(ip: Ipv4Address, value: u16) -> Self {
        RouteTarget::Ipv4 { ip, value }
    }

    pub fn to_ext_community(self) -> BgpExtendedCommunity {
        match self {
            RouteTarget::As2 { asn, value } => {
                BgpExtendedCommunity::RouteTarget2Octet { asn, value }
            }
            RouteTarget::Ipv4 { ip, value } => BgpExtendedCommunity::RouteTargetIpv4 { ip, value },
        }
    }

    /// Reads a Route Target out of an extended community, or `None` if that
    /// community is something else entirely.
    pub fn from_ext_community(comm: &BgpExtendedCommunity) -> Option<Self> {
        match comm {
            BgpExtendedCommunity::RouteTarget2Octet { asn, value } => Some(RouteTarget::As2 {
                asn: *asn,
                value: *value,
            }),
            BgpExtendedCommunity::RouteTargetIpv4 { ip, value } => Some(RouteTarget::Ipv4 {
                ip: *ip,
                value: *value,
            }),
            _ => None,
        }
    }

    /// Reads a Route Target straight from eight wire bytes.
    ///
    /// The type and subtype are checked here rather than delegated to the generic
    /// community parser, because that parser maps anything it does not recognise
    /// to `Raw`, and a `Raw` that happened to look like an RT must not be
    /// mistaken for one.
    pub fn from_bytes(raw: &[u8; 8]) -> Option<Self> {
        if raw[1] != BGP_EXT_COMM_SUBTYPE_ROUTE_TARGET {
            return None;
        }
        match raw[0] {
            BGP_EXT_COMM_TYPE_2OCTET_AS => Some(RouteTarget::As2 {
                asn: u16::from_be_bytes([raw[2], raw[3]]),
                value: u32::from_be_bytes([raw[4], raw[5], raw[6], raw[7]]),
            }),
            BGP_EXT_COMM_TYPE_IPV4_ADDR => Some(RouteTarget::Ipv4 {
                ip: Ipv4Address([raw[2], raw[3], raw[4], raw[5]]),
                value: u16::from_be_bytes([raw[6], raw[7]]),
            }),
            _ => None,
        }
    }

    pub fn to_bytes(self) -> [u8; 8] {
        self.to_ext_community().serialize()
    }
}

impl fmt::Display for RouteTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RouteTarget::As2 { asn, value } => write!(f, "{}:{}", asn, value),
            RouteTarget::Ipv4 { ip, value } => write!(f, "{}:{}", ip, value),
        }
    }
}

/// Every Route Target in a list of raw extended communities, in order.
pub fn route_targets_from_communities(raw: &[[u8; 8]]) -> Vec<RouteTarget> {
    raw.iter().filter_map(RouteTarget::from_bytes).collect()
}

/// The MAC Mobility sequence number from a list of raw extended communities.
pub fn mac_mobility_from_communities(raw: &[[u8; 8]]) -> Option<u32> {
    raw.iter()
        .find_map(|c| match BgpExtendedCommunity::parse(c) {
            Some(BgpExtendedCommunity::MacMobility { sequence, .. }) => Some(sequence),
            _ => None,
        })
}

/// The extended communities that are neither a Route Target nor MAC Mobility.
///
/// [`EvpnRoute`] stores the two it understands as decoded fields and re-emits
/// them from those. Anything else has to be kept verbatim, or re-advertising a
/// route would quietly drop communities the sender attached and a downstream
/// speaker may act on - which for a route reflector would be a straight
/// violation of RFC 4456 section 10.
pub fn other_ext_communities(raw: &[[u8; 8]]) -> Vec<[u8; 8]> {
    raw.iter()
        .filter(|c| {
            RouteTarget::from_bytes(c).is_none()
                && !matches!(
                    BgpExtendedCommunity::parse(*c),
                    Some(BgpExtendedCommunity::MacMobility { .. })
                )
        })
        .copied()
        .collect()
}

// ============================================================================
// Route identity
// ============================================================================

/// What makes two EVPN advertisements the same route.
///
/// RFC 7432 keys a MAC/IP route on `(RD, Ethernet Tag, MAC, IP)`. The VNI is
/// deliberately *not* part of the key: a host moving between VTEPs keeps its MAC
/// but arrives with a different RD, and a re-advertisement of the same MAC in the
/// same instance must replace the old one rather than sit beside it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvpnRouteKey {
    MacIp {
        rd: (u32, u16),
        eth_tag: u32,
        mac: MacAddress,
        ip: Option<Ipv4Address>,
    },
    Imet {
        rd: (u32, u16),
        eth_tag: u32,
        originator: Ipv4Address,
    },
}

impl EvpnRouteKey {
    pub fn from_nlri(nlri: &EvpnNlri) -> Self {
        match nlri {
            EvpnNlri::MacIpAdv(m) => EvpnRouteKey::MacIp {
                rd: (m.rd.admin, m.rd.assigned),
                eth_tag: m.eth_tag,
                mac: m.mac,
                ip: m.ip,
            },
            EvpnNlri::InclusiveMulticast(im) => EvpnRouteKey::Imet {
                rd: (im.rd.admin, im.rd.assigned),
                eth_tag: im.eth_tag,
                originator: im.originating_router_ip,
            },
        }
    }

    pub fn route_type(&self) -> u8 {
        match self {
            EvpnRouteKey::MacIp { .. } => EVPN_TYPE_MAC_IP_ADV,
            EvpnRouteKey::Imet { .. } => EVPN_TYPE_INCLUSIVE_MULTICAST,
        }
    }

    pub fn rd(&self) -> RouteDistinguisher {
        let (admin, assigned) = match self {
            EvpnRouteKey::MacIp { rd, .. } | EvpnRouteKey::Imet { rd, .. } => *rd,
        };
        RouteDistinguisher { admin, assigned }
    }

    /// The MAC this route describes, for a Type 2 route.
    pub fn mac(&self) -> Option<MacAddress> {
        match self {
            EvpnRouteKey::MacIp { mac, .. } => Some(*mac),
            EvpnRouteKey::Imet { .. } => None,
        }
    }
}

impl fmt::Display for EvpnRouteKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvpnRouteKey::MacIp {
                eth_tag, mac, ip, ..
            } => write!(
                f,
                "[2]:[{}]:[{}]:[{}]",
                eth_tag,
                mac,
                ip.map(|i| i.to_string()).unwrap_or_else(|| "-".to_string())
            ),
            EvpnRouteKey::Imet {
                eth_tag,
                originator,
                ..
            } => write!(f, "[3]:[{}]:[{}]", eth_tag, originator),
        }
    }
}

// ============================================================================
// Routes and paths
// ============================================================================

/// One EVPN route as this speaker originates or re-advertises it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvpnRoute {
    pub nlri: EvpnNlri,
    /// The VTEP that owns the MAC. This is the MP_REACH next hop, and it is what
    /// the data plane ends up encapsulating towards.
    pub next_hop: Ipv4Address,
    pub route_targets: Vec<RouteTarget>,
    /// MAC Mobility sequence number, present once a MAC has moved at least once.
    pub mobility_seq: Option<u32>,
    /// Extended Communities that arrived with the route and are neither a Route
    /// Target nor MAC Mobility.
    ///
    /// They are kept verbatim so that re-advertising - and in particular
    /// reflecting - a route does not silently strip communities the receiver may
    /// depend on. RFC 4456 section 10 is explicit that a reflector must not
    /// modify the path attributes it passes on, and rebuilding the community
    /// list from only the parts this speaker happens to understand would do
    /// exactly that.
    pub other_communities: Vec<[u8; 8]>,
}

impl EvpnRoute {
    pub fn new(nlri: EvpnNlri, next_hop: Ipv4Address, route_targets: Vec<RouteTarget>) -> Self {
        EvpnRoute {
            nlri,
            next_hop,
            route_targets,
            mobility_seq: None,
            other_communities: Vec::new(),
        }
    }

    pub fn with_mobility(mut self, sequence: u32) -> Self {
        self.mobility_seq = Some(sequence);
        self
    }

    pub fn key(&self) -> EvpnRouteKey {
        EvpnRouteKey::from_nlri(&self.nlri)
    }

    /// The VNI this route belongs to. A Type 3 route carries no label field of
    /// its own in this encoding, so its Ethernet Tag doubles as the identifier.
    pub fn vni(&self) -> u32 {
        match &self.nlri {
            EvpnNlri::MacIpAdv(m) => m.vni,
            EvpnNlri::InclusiveMulticast(im) => im.eth_tag,
        }
    }

    pub fn mac(&self) -> Option<MacAddress> {
        match &self.nlri {
            EvpnNlri::MacIpAdv(m) => Some(m.mac),
            EvpnNlri::InclusiveMulticast(_) => None,
        }
    }

    pub fn host_ip(&self) -> Option<Ipv4Address> {
        match &self.nlri {
            EvpnNlri::MacIpAdv(m) => m.ip,
            EvpnNlri::InclusiveMulticast(_) => None,
        }
    }

    /// The extended communities that go on the wire with this route.
    pub fn ext_communities(&self) -> Vec<[u8; 8]> {
        let mut out: Vec<[u8; 8]> = self.route_targets.iter().map(|rt| rt.to_bytes()).collect();
        if let Some(seq) = self.mobility_seq {
            out.push(
                BgpExtendedCommunity::MacMobility {
                    sticky: false,
                    sequence: seq,
                }
                .serialize(),
            );
        }
        out.extend_from_slice(&self.other_communities);
        out
    }

    /// True when any of this route's Route Targets is one `import` asks for.
    pub fn matches_import(&self, import: &BTreeSet<RouteTarget>) -> bool {
        self.route_targets.iter().any(|rt| import.contains(rt))
    }
}

/// One EVPN route as received from a peer, with the attributes the decision
/// process compares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvpnPath {
    pub route: EvpnRoute,
    /// Peer that advertised it; unspecified for a locally originated route.
    pub peer_addr: Ipv4Address,
    pub peer_as: u32,
    pub peer_router_id: Ipv4Address,
    pub origin: BgpOrigin,
    pub as_path: AsPath,
    pub local_pref: u32,
    /// ORIGINATOR_ID as received (RFC 4456), when the route has been reflected.
    pub originator_id: Option<Ipv4Address>,
    /// CLUSTER_LIST as received (RFC 4456).
    pub cluster_list: Vec<Ipv4Address>,
    /// True when the peer that advertised this path is a route reflector client
    /// of ours, which is what decides who it may be reflected on to.
    pub from_client: bool,
    /// True when one of this route's Route Targets is one this speaker imports.
    ///
    /// A route reflector retains routes it does not import - that is the whole
    /// point of it - so "stored" and "usable here" are separate questions and
    /// the answer to the second is recorded rather than re-derived.
    pub importable: bool,
    pub received_at_ms: u64,
    pub local: bool,
}

impl EvpnPath {
    /// A route this speaker originates for one of its own attached hosts.
    pub fn local(route: EvpnRoute, router_id: Ipv4Address, now_ms: u64) -> Self {
        EvpnPath {
            route,
            peer_addr: Ipv4Address::UNSPECIFIED,
            peer_as: 0,
            peer_router_id: router_id,
            origin: BgpOrigin::Igp,
            as_path: AsPath::empty(),
            local_pref: BGP_DEFAULT_LOCAL_PREF,
            originator_id: None,
            cluster_list: Vec::new(),
            from_client: false,
            importable: true,
            received_at_ms: now_ms,
            local: true,
        }
    }

    pub fn key(&self) -> EvpnRouteKey {
        self.route.key()
    }

    /// True when two paths describe the same route in every way the overlay can
    /// observe. `received_at_ms` is excluded so a peer re-sending an unchanged
    /// route does not look like a change and rerun the decision process.
    pub fn same_route_as(&self, other: &EvpnPath) -> bool {
        let EvpnPath {
            route,
            peer_addr,
            peer_as,
            peer_router_id,
            origin,
            as_path,
            local_pref,
            originator_id,
            cluster_list,
            from_client,
            importable,
            received_at_ms: _,
            local,
        } = self;

        *route == other.route
            && *peer_addr == other.peer_addr
            && *peer_as == other.peer_as
            && *peer_router_id == other.peer_router_id
            && *origin == other.origin
            && *as_path == other.as_path
            && *local_pref == other.local_pref
            && *originator_id == other.originator_id
            && *cluster_list == other.cluster_list
            && *from_client == other.from_client
            && *importable == other.importable
            && *local == other.local
    }
}

/// Orders two candidate EVPN paths. `Ordering::Less` means `a` wins.
///
/// The MAC Mobility sequence number comes *first*, ahead of every ordinary BGP
/// attribute (RFC 7432 section 15). That is the whole mechanism by which a host
/// that moves is followed: the new location advertises a higher sequence number,
/// and every speaker prefers it no matter what the rest of the path looks like.
/// Running the normal tie-break chain first would let an arbitrary detail such as
/// a lower peer address pin traffic to the location the host has left.
pub fn compare_evpn_paths(a: &EvpnPath, b: &EvpnPath) -> Ordering {
    match b
        .route
        .mobility_seq
        .unwrap_or(0)
        .cmp(&a.route.mobility_seq.unwrap_or(0))
    {
        Ordering::Equal => {}
        other => return other,
    }

    // A route for a host attached here outranks anything learned about it.
    match (a.local, b.local) {
        (true, false) => return Ordering::Less,
        (false, true) => return Ordering::Greater,
        _ => {}
    }

    match b.local_pref.cmp(&a.local_pref) {
        Ordering::Equal => {}
        other => return other,
    }
    match a.as_path.length().cmp(&b.as_path.length()) {
        Ordering::Equal => {}
        other => return other,
    }
    match a.origin.cmp(&b.origin) {
        Ordering::Equal => {}
        other => return other,
    }
    // Shortest CLUSTER_LIST (RFC 4456 section 9). A route that has been through
    // no cluster has length zero and so beats any reflected copy of itself.
    //
    // Without this, two reflectors serving the same leaf each prefer the other's
    // reflected copy over the leaf's own advertisement whenever the leaf's BGP
    // identifier is the higher one - the tie-break below is what would decide it.
    // Each reflector then withdraws from the other under split horizon, loses the
    // path it had just preferred, and re-advertises, for ever. The fabric stays
    // correct throughout but never goes quiet, which in an EVPN fabric means
    // every leaf reprogramming its overlay on every cycle.
    match a.cluster_list.len().cmp(&b.cluster_list.len()) {
        Ordering::Equal => {}
        other => return other,
    }
    // Ends in the peer identity, so no two distinct paths ever compare equal and
    // the winner never depends on iteration order.
    a.peer_router_id
        .cmp(&b.peer_router_id)
        .then(a.peer_addr.cmp(&b.peer_addr))
}

pub fn select_best_evpn<'a>(candidates: &[&'a EvpnPath]) -> Option<&'a EvpnPath> {
    candidates
        .iter()
        .copied()
        .reduce(|best, next| match compare_evpn_paths(next, best) {
            Ordering::Less => next,
            _ => best,
        })
}

// ============================================================================
// RIBs
// ============================================================================

/// Every EVPN route received from every peer, indexed by peer first so purging
/// one peer cannot touch another's routes.
#[derive(Debug, Clone, Default)]
pub struct EvpnAdjRibIn {
    tables: BTreeMap<Ipv4Address, BTreeMap<EvpnRouteKey, EvpnPath>>,
}

impl EvpnAdjRibIn {
    pub fn new() -> Self {
        EvpnAdjRibIn::default()
    }

    pub fn insert(&mut self, peer: Ipv4Address, path: EvpnPath) -> Option<EvpnPath> {
        self.tables
            .entry(peer)
            .or_default()
            .insert(path.key(), path)
    }

    pub fn remove(&mut self, peer: Ipv4Address, key: &EvpnRouteKey) -> Option<EvpnPath> {
        let removed = self.tables.get_mut(&peer).and_then(|t| t.remove(key));
        if self.tables.get(&peer).is_some_and(|t| t.is_empty()) {
            self.tables.remove(&peer);
        }
        removed
    }

    /// Drops everything learned from `peer`, which is what a session going down
    /// must do before any of it can be used again.
    pub fn clear_peer(&mut self, peer: Ipv4Address) -> usize {
        self.tables.remove(&peer).map(|t| t.len()).unwrap_or(0)
    }

    pub fn route_count(&self, peer: Ipv4Address) -> usize {
        self.tables.get(&peer).map(|t| t.len()).unwrap_or(0)
    }

    pub fn total_routes(&self) -> usize {
        self.tables.values().map(|t| t.len()).sum()
    }

    pub fn keys(&self) -> BTreeSet<EvpnRouteKey> {
        self.tables
            .values()
            .flat_map(|t| t.keys().cloned())
            .collect()
    }

    pub fn candidates(&self, key: &EvpnRouteKey) -> Vec<&EvpnPath> {
        self.tables.values().filter_map(|t| t.get(key)).collect()
    }

    pub fn iter_paths(&self) -> impl Iterator<Item = &EvpnPath> {
        self.tables.values().flat_map(|t| t.values())
    }

    pub fn peer_table(&self, peer: Ipv4Address) -> Option<&BTreeMap<EvpnRouteKey, EvpnPath>> {
        self.tables.get(&peer)
    }

    /// The stored paths from one peer, for in-place amendment when something
    /// about the peer changes rather than about the routes it sent.
    pub fn peer_table_mut(
        &mut self,
        peer: Ipv4Address,
    ) -> Option<&mut BTreeMap<EvpnRouteKey, EvpnPath>> {
        self.tables.get_mut(&peer)
    }
}

/// The best EVPN path per route. This is the only thing the VTEP is programmed
/// from, so anything not in here cannot be forwarded to.
#[derive(Debug, Clone, Default)]
pub struct EvpnLocRib {
    best: BTreeMap<EvpnRouteKey, EvpnPath>,
}

impl EvpnLocRib {
    pub fn new() -> Self {
        EvpnLocRib::default()
    }

    pub fn insert(&mut self, path: EvpnPath) -> Option<EvpnPath> {
        self.best.insert(path.key(), path)
    }

    pub fn get(&self, key: &EvpnRouteKey) -> Option<&EvpnPath> {
        self.best.get(key)
    }

    pub fn len(&self) -> usize {
        self.best.len()
    }

    pub fn is_empty(&self) -> bool {
        self.best.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&EvpnRouteKey, &EvpnPath)> {
        self.best.iter()
    }

    pub fn keys(&self) -> Vec<EvpnRouteKey> {
        self.best.keys().cloned().collect()
    }

    /// Every path learned from a peer, i.e. everything that describes a *remote*
    /// location. Locally originated routes are excluded: a VTEP must not program
    /// a tunnel to itself for a host sitting on its own access port.
    pub fn remote_paths(&self) -> impl Iterator<Item = &EvpnPath> {
        self.best.values().filter(|p| !p.local)
    }
}

/// One EVPN route as advertised to one peer, together with the path attributes
/// that went on the wire with it.
///
/// The attributes belong here rather than being recomputed at send time because
/// two advertisements of the same route are not necessarily the same
/// advertisement: the same MAC behind the same VTEP reached through a different
/// route reflector carries a different CLUSTER_LIST, and a receiver that never
/// heard the correction would keep loop-prevention state that no longer matches
/// the path it is actually using.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvpnAdvertisedRoute {
    pub route: EvpnRoute,
    pub origin: BgpOrigin,
    pub as_path: AsPath,
    /// LOCAL_PREF, sent on internal sessions only (RFC 4271 section 5.1.5).
    pub local_pref: Option<u32>,
    /// ORIGINATOR_ID, present only when this speaker is reflecting the route.
    pub originator_id: Option<Ipv4Address>,
    /// CLUSTER_LIST, with the local cluster ID already prepended when reflecting.
    pub cluster_list: Vec<Ipv4Address>,
}

impl EvpnAdvertisedRoute {
    pub fn key(&self) -> EvpnRouteKey {
        self.route.key()
    }

    /// True when this advertisement carries RFC 4456 reflection metadata, which
    /// is exactly when it was produced by reflecting somebody else's route.
    pub fn is_reflected(&self) -> bool {
        self.originator_id.is_some() || !self.cluster_list.is_empty()
    }
}

/// What this speaker has advertised to each peer, so only differences are sent.
#[derive(Debug, Clone, Default)]
pub struct EvpnAdjRibOut {
    tables: BTreeMap<Ipv4Address, BTreeMap<EvpnRouteKey, EvpnAdvertisedRoute>>,
}

impl EvpnAdjRibOut {
    pub fn new() -> Self {
        EvpnAdjRibOut::default()
    }

    pub fn get(&self, peer: Ipv4Address, key: &EvpnRouteKey) -> Option<&EvpnAdvertisedRoute> {
        self.tables.get(&peer).and_then(|t| t.get(key))
    }

    pub fn insert(&mut self, peer: Ipv4Address, advert: EvpnAdvertisedRoute) {
        self.tables
            .entry(peer)
            .or_default()
            .insert(advert.key(), advert);
    }

    pub fn remove(&mut self, peer: Ipv4Address, key: &EvpnRouteKey) -> Option<EvpnAdvertisedRoute> {
        let removed = self.tables.get_mut(&peer).and_then(|t| t.remove(key));
        if self.tables.get(&peer).is_some_and(|t| t.is_empty()) {
            self.tables.remove(&peer);
        }
        removed
    }

    pub fn clear_peer(&mut self, peer: Ipv4Address) -> usize {
        self.tables.remove(&peer).map(|t| t.len()).unwrap_or(0)
    }

    pub fn keys(&self, peer: Ipv4Address) -> Vec<EvpnRouteKey> {
        self.tables
            .get(&peer)
            .map(|t| t.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub fn route_count(&self, peer: Ipv4Address) -> usize {
        self.tables.get(&peer).map(|t| t.len()).unwrap_or(0)
    }

    /// Total advertisements held across all peers.
    pub fn total_routes(&self) -> usize {
        self.tables.values().map(|t| t.len()).sum()
    }

    /// How many of the advertisements to `peer` carry reflection metadata.
    pub fn reflected_count(&self, peer: Ipv4Address) -> usize {
        self.tables
            .get(&peer)
            .map(|t| t.values().filter(|a| a.is_reflected()).count())
            .unwrap_or(0)
    }

    pub fn peer_table(
        &self,
        peer: Ipv4Address,
    ) -> Option<&BTreeMap<EvpnRouteKey, EvpnAdvertisedRoute>> {
        self.tables.get(&peer)
    }
}

// ============================================================================
// NLRI list codec
// ============================================================================

/// Encodes a list of EVPN NLRI for the payload of an MP attribute.
pub fn encode_evpn_nlri_list(nlri: &[EvpnNlri]) -> Vec<u8> {
    let mut out = Vec::new();
    for n in nlri {
        out.extend_from_slice(&n.serialize());
    }
    out
}

/// Decodes the NLRI payload of an MP_REACH or MP_UNREACH attribute.
///
/// Each entry is `type(1) length(1) body(length)`, so the walk is driven by the
/// declared length rather than by what the body turns out to contain. An entry
/// whose length runs past the end is a hard error; an entry of a route type this
/// speaker does not implement is *skipped*, because RFC 7432 keeps adding types
/// and refusing the whole UPDATE would drop the routes we do understand alongside
/// it.
pub fn decode_evpn_nlri_list(data: &[u8]) -> Result<Vec<EvpnNlri>, BgpParseError> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < data.len() {
        if i + 2 > data.len() {
            return Err(BgpParseError::update(
                crate::bgp::BGP_SUB_INVALID_NETWORK_FIELD,
                "truncated EVPN NLRI header",
            ));
        }
        let route_type = data[i];
        let len = data[i + 1] as usize;
        let end = i + 2 + len;
        if end > data.len() {
            return Err(BgpParseError::update(
                crate::bgp::BGP_SUB_INVALID_NETWORK_FIELD,
                format!(
                    "EVPN route type {} claims {} bytes but only {} remain",
                    route_type,
                    len,
                    data.len() - (i + 2)
                ),
            ));
        }
        if len == 0 {
            return Err(BgpParseError::update(
                crate::bgp::BGP_SUB_INVALID_NETWORK_FIELD,
                format!("EVPN route type {} has a zero-length body", route_type),
            ));
        }
        if out.len() >= MAX_EVPN_NLRI_PER_UPDATE {
            return Err(BgpParseError::update(
                crate::bgp::BGP_SUB_INVALID_NETWORK_FIELD,
                format!(
                    "more than {} EVPN NLRI in one UPDATE",
                    MAX_EVPN_NLRI_PER_UPDATE
                ),
            ));
        }

        match EvpnNlri::parse(&data[i..end]) {
            Ok(n) => out.push(n),
            // The length field was honest, so the walk stays in step; only this
            // one entry is unusable.
            Err(EvpnError::InvalidRouteType(_)) => {}
            Err(e) => {
                return Err(BgpParseError::update(
                    crate::bgp::BGP_SUB_INVALID_NETWORK_FIELD,
                    format!("malformed EVPN route type {}: {}", route_type, e),
                ));
            }
        }
        i = end;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evpn::EvpnNlri;

    fn rd() -> RouteDistinguisher {
        RouteDistinguisher::new(Ipv4Address::new(10, 0, 0, 1), 5001)
    }

    fn mac(last: u8) -> MacAddress {
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x00, last])
    }

    fn route(seq: Option<u32>, vtep: Ipv4Address) -> EvpnRoute {
        let nlri = EvpnNlri::build_mac_ip(rd(), mac(1), None, 5001);
        let mut r = EvpnRoute::new(nlri, vtep, vec![RouteTarget::as2(65001, 5001)]);
        r.mobility_seq = seq;
        r
    }

    fn path(seq: Option<u32>, vtep: Ipv4Address, peer: Ipv4Address) -> EvpnPath {
        EvpnPath {
            route: route(seq, vtep),
            peer_addr: peer,
            peer_as: 65002,
            peer_router_id: peer,
            origin: BgpOrigin::Igp,
            as_path: AsPath::sequence(vec![65002]),
            local_pref: BGP_DEFAULT_LOCAL_PREF,
            originator_id: None,
            cluster_list: Vec::new(),
            from_client: false,
            importable: true,
            received_at_ms: 0,
            local: false,
        }
    }

    #[test]
    fn test_route_target_round_trips_through_wire_bytes() {
        for rt in [
            RouteTarget::as2(65001, 5001),
            RouteTarget::ipv4(Ipv4Address::new(10, 0, 0, 1), 7),
        ] {
            assert_eq!(RouteTarget::from_bytes(&rt.to_bytes()), Some(rt));
        }
    }

    #[test]
    fn test_a_non_route_target_community_is_not_read_as_one() {
        let color = BgpExtendedCommunity::Color {
            flags: 0,
            color: 100,
        }
        .serialize();
        assert_eq!(RouteTarget::from_bytes(&color), None);
    }

    #[test]
    fn test_import_requires_a_matching_route_target() {
        let r = route(None, Ipv4Address::new(10, 0, 0, 1));
        assert!(r.matches_import(&BTreeSet::from([RouteTarget::as2(65001, 5001)])));
        assert!(!r.matches_import(&BTreeSet::from([RouteTarget::as2(65001, 5002)])));
        assert!(!r.matches_import(&BTreeSet::new()));
    }

    #[test]
    fn test_a_higher_mobility_sequence_wins_over_a_lower_peer_address() {
        // The lower peer address would win the ordinary tie-break; the sequence
        // number must override it, or a moved host keeps drawing traffic to
        // where it used to be.
        let old = path(
            Some(1),
            Ipv4Address::new(10, 0, 0, 1),
            Ipv4Address::new(1, 1, 1, 1),
        );
        let new = path(
            Some(2),
            Ipv4Address::new(10, 0, 0, 2),
            Ipv4Address::new(9, 9, 9, 9),
        );
        let best = select_best_evpn(&[&old, &new]).unwrap();
        assert_eq!(best.route.next_hop, Ipv4Address::new(10, 0, 0, 2));
        // Order of the candidate list must not change the answer.
        let best = select_best_evpn(&[&new, &old]).unwrap();
        assert_eq!(best.route.next_hop, Ipv4Address::new(10, 0, 0, 2));
    }

    #[test]
    fn test_equal_sequences_fall_back_to_the_deterministic_tie_break() {
        let a = path(
            Some(1),
            Ipv4Address::new(10, 0, 0, 1),
            Ipv4Address::new(1, 1, 1, 1),
        );
        let b = path(
            Some(1),
            Ipv4Address::new(10, 0, 0, 2),
            Ipv4Address::new(9, 9, 9, 9),
        );
        assert_eq!(
            select_best_evpn(&[&a, &b]).unwrap().route.next_hop,
            Ipv4Address::new(10, 0, 0, 1)
        );
    }

    #[test]
    fn test_nlri_list_round_trips() {
        let list = vec![
            EvpnNlri::build_mac_ip(rd(), mac(1), Some(Ipv4Address::new(192, 168, 10, 11)), 5001),
            EvpnNlri::build_mac_ip(rd(), mac(2), None, 5001),
            EvpnNlri::build_inclusive_multicast(rd(), Ipv4Address::new(10, 0, 0, 1)),
        ];
        let raw = encode_evpn_nlri_list(&list);
        assert_eq!(decode_evpn_nlri_list(&raw).unwrap(), list);
    }

    #[test]
    fn test_an_nlri_length_past_the_end_is_refused() {
        assert!(decode_evpn_nlri_list(&[2, 200, 0, 0]).is_err());
        assert!(decode_evpn_nlri_list(&[2]).is_err());
        assert!(decode_evpn_nlri_list(&[2, 0]).is_err());
    }

    #[test]
    fn test_an_unknown_route_type_is_skipped_without_losing_the_rest() {
        let mut raw = vec![4u8, 3, 0xAA, 0xBB, 0xCC]; // Type 4, not implemented
        raw.extend_from_slice(&EvpnNlri::build_mac_ip(rd(), mac(7), None, 5001).serialize());
        let decoded = decode_evpn_nlri_list(&raw).unwrap();
        assert_eq!(decoded.len(), 1);
        assert!(matches!(decoded[0], EvpnNlri::MacIpAdv(_)));
    }

    #[test]
    fn test_adj_rib_in_purges_one_peer_without_touching_another() {
        let mut rib = EvpnAdjRibIn::new();
        let p1 = Ipv4Address::new(10, 0, 0, 1);
        let p2 = Ipv4Address::new(10, 0, 0, 2);
        rib.insert(p1, path(None, p1, p1));
        rib.insert(p2, path(None, p2, p2));
        assert_eq!(rib.total_routes(), 2);
        assert_eq!(rib.clear_peer(p1), 1);
        assert_eq!(rib.route_count(p1), 0);
        assert_eq!(rib.route_count(p2), 1);
    }
}
