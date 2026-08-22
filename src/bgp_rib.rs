//! BGP-4 Routing Information Bases and the decision process (RFC 4271 section 9).
//!
//! Three tables, kept genuinely separate rather than collapsed into one map:
//!
//! * `AdjRibIn`  - every path received from every peer, exactly as advertised, indexed
//!   by peer first. Purging a peer is therefore a single scoped operation and cannot
//!   leave another peer's paths behind.
//! * `LocRib`    - the single best path per prefix, chosen by the decision process from
//!   the union of the Adj-RIB-In tables and the locally originated routes.
//! * `AdjRibOut` - what this speaker has actually advertised to each peer, so the next
//!   advertisement run can emit only the differences (announce, replace, withdraw).
//!
//! Nothing here talks to a socket or a forwarding table; `bgp_router` drives it.

use crate::bgp::{AsPath, BGP_DEFAULT_LOCAL_PREF, BgpOrigin, Ipv4Prefix};
use crate::ipv4::Ipv4Address;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

/// How this speaker came to know a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PathSource {
    /// Originated locally by configuration, equivalent to a `network` statement.
    Local,
    /// Learned from a peer in a different autonomous system.
    Ebgp,
    /// Learned from a peer in the same autonomous system.
    Ibgp,
}

impl PathSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            PathSource::Local => "local",
            PathSource::Ebgp => "ebgp",
            PathSource::Ibgp => "ibgp",
        }
    }
}

/// One path to one prefix, with the attributes the decision process actually uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BgpPath {
    pub prefix: Ipv4Prefix,
    pub source: PathSource,
    /// Address of the peer that advertised it; unspecified for locally originated paths.
    pub peer_addr: Ipv4Address,
    pub peer_as: u32,
    pub peer_router_id: Ipv4Address,
    pub origin: BgpOrigin,
    pub as_path: AsPath,
    pub next_hop: Ipv4Address,
    pub med: Option<u32>,
    pub local_pref: u32,
    pub atomic_aggregate: bool,
    /// ORIGINATOR_ID as received (RFC 4456). Present when this path has already
    /// been reflected at least once; the identifier is of the speaker that first
    /// advertised the route inside this AS, not of the reflector that passed it on.
    pub originator_id: Option<Ipv4Address>,
    /// CLUSTER_LIST as received (RFC 4456): the reflectors already traversed.
    pub cluster_list: Vec<Ipv4Address>,
    /// True when this path was learned from a peer configured as a route
    /// reflector client. It decides who the path may be reflected on to, so it
    /// is a property of the path rather than something re-derived at export
    /// time: the peer's role could change while the path is still stored.
    pub from_client: bool,
    /// Simulated time at which this path was received or originated.
    pub received_at_ms: u64,
}

impl BgpPath {
    /// A path this speaker originates itself. It carries an empty AS_PATH; the local
    /// ASN is prepended only when the path is advertised to an external peer.
    pub fn local(prefix: Ipv4Prefix, next_hop: Ipv4Address, router_id: Ipv4Address) -> Self {
        BgpPath {
            prefix,
            source: PathSource::Local,
            peer_addr: Ipv4Address::UNSPECIFIED,
            peer_as: 0,
            peer_router_id: router_id,
            origin: BgpOrigin::Igp,
            as_path: AsPath::empty(),
            next_hop,
            med: None,
            local_pref: BGP_DEFAULT_LOCAL_PREF,
            atomic_aggregate: false,
            originator_id: None,
            cluster_list: Vec::new(),
            from_client: false,
            received_at_ms: 0,
        }
    }

    pub fn is_local(&self) -> bool {
        self.source == PathSource::Local
    }

    pub fn is_ebgp(&self) -> bool {
        self.source == PathSource::Ebgp
    }

    pub fn as_path_len(&self) -> usize {
        self.as_path.length()
    }

    /// True when two paths describe the same route: everything the decision process
    /// and the forwarding table can actually see.
    ///
    /// `received_at_ms` is deliberately left out. It is a diagnostic, and a peer that
    /// re-sends a route it has already sent - a refresh, a duplicate, a chatty
    /// neighbour - would otherwise look like a change every single time, marking the
    /// RIB dirty and rerunning the whole decision process for nothing.
    ///
    /// Every other field is listed by name rather than skipped with `..`, so adding an
    /// attribute to `BgpPath` fails to compile here until someone decides whether it
    /// belongs in this comparison.
    pub fn same_route_as(&self, other: &BgpPath) -> bool {
        let BgpPath {
            prefix,
            source,
            peer_addr,
            peer_as,
            peer_router_id,
            origin,
            as_path,
            next_hop,
            med,
            local_pref,
            atomic_aggregate,
            originator_id,
            cluster_list,
            from_client,
            received_at_ms: _,
        } = self;

        *prefix == other.prefix
            && *source == other.source
            && *peer_addr == other.peer_addr
            && *peer_as == other.peer_as
            && *peer_router_id == other.peer_router_id
            && *origin == other.origin
            && *as_path == other.as_path
            && *next_hop == other.next_hop
            && *med == other.med
            && *local_pref == other.local_pref
            && *atomic_aggregate == other.atomic_aggregate
            && *originator_id == other.originator_id
            && *cluster_list == other.cluster_list
            && *from_client == other.from_client
    }
}

/// Orders two candidate paths for the same prefix. `Ordering::Less` means `a` wins.
///
/// The tie-break chain is fully deterministic: it ends in a comparison of the peer's
/// BGP identifier and then the peer address, so no two distinct paths can ever compare
/// equal and the winner never depends on hash or insertion order.
pub fn compare_paths(a: &BgpPath, b: &BgpPath) -> Ordering {
    // 0. A path this speaker originates itself outranks anything learned.
    match (a.is_local(), b.is_local()) {
        (true, false) => return Ordering::Less,
        (false, true) => return Ordering::Greater,
        _ => {}
    }

    // 1. Highest LOCAL_PREF.
    match b.local_pref.cmp(&a.local_pref) {
        Ordering::Equal => {}
        other => return other,
    }

    // 2. Shortest AS_PATH.
    match a.as_path_len().cmp(&b.as_path_len()) {
        Ordering::Equal => {}
        other => return other,
    }

    // 3. Lowest ORIGIN (IGP < EGP < INCOMPLETE).
    match a.origin.cmp(&b.origin) {
        Ordering::Equal => {}
        other => return other,
    }

    // 4. Lowest MULTI_EXIT_DISC, but only between paths from the same neighbouring AS.
    //    Comparing MEDs across autonomous systems is not meaningful, so that step is
    //    skipped rather than faked.
    let (fa, fb) = (a.as_path.first_as(), b.as_path.first_as());
    if fa.is_some() && fa == fb {
        match a.med.unwrap_or(0).cmp(&b.med.unwrap_or(0)) {
            Ordering::Equal => {}
            other => return other,
        }
    }

    // 5. Prefer eBGP over iBGP.
    match (a.is_ebgp(), b.is_ebgp()) {
        (true, false) => return Ordering::Less,
        (false, true) => return Ordering::Greater,
        _ => {}
    }

    // 6. Shortest CLUSTER_LIST (RFC 4456 section 9), a route with no CLUSTER_LIST
    //    counting as length zero.
    //
    //    This step is not decoration. Without it a pair of route reflectors can
    //    prefer each other's reflected copy of a client's route over the copy the
    //    client advertised to them directly - which happens whenever the client's
    //    BGP identifier is the higher one, since that is what the next step
    //    compares. Each reflector then sees its best path as coming from the
    //    other, stops advertising to it under split horizon, immediately loses
    //    that path again, and re-advertises: a persistent oscillation that never
    //    settles and never stops sending UPDATEs. Preferring the shorter cluster
    //    list makes the client's own advertisement win every time, because it has
    //    not been through a cluster at all.
    match a.cluster_list.len().cmp(&b.cluster_list.len()) {
        Ordering::Equal => {}
        other => return other,
    }

    // 7. Lowest peer BGP identifier, then 8. lowest peer address.
    a.peer_router_id
        .cmp(&b.peer_router_id)
        .then(a.peer_addr.cmp(&b.peer_addr))
}

/// Picks the winner from a candidate set. Returns `None` for an empty set.
pub fn select_best<'a>(candidates: &[&'a BgpPath]) -> Option<&'a BgpPath> {
    candidates
        .iter()
        .copied()
        .reduce(|best, next| match compare_paths(next, best) {
            Ordering::Less => next,
            _ => best,
        })
}

/// Every path received from every peer, indexed by peer and then prefix.
#[derive(Debug, Clone, Default)]
pub struct AdjRibIn {
    tables: BTreeMap<Ipv4Address, BTreeMap<Ipv4Prefix, BgpPath>>,
}

impl AdjRibIn {
    pub fn new() -> Self {
        AdjRibIn::default()
    }

    /// Records a path received from `peer`, replacing any previous path for that
    /// prefix from that peer (an implicit withdrawal, RFC 4271 section 3.1).
    pub fn insert(&mut self, peer: Ipv4Address, path: BgpPath) -> Option<BgpPath> {
        self.tables
            .entry(peer)
            .or_default()
            .insert(path.prefix, path)
    }

    pub fn remove(&mut self, peer: Ipv4Address, prefix: Ipv4Prefix) -> Option<BgpPath> {
        let removed = self.tables.get_mut(&peer).and_then(|t| t.remove(&prefix));
        if self.tables.get(&peer).is_some_and(|t| t.is_empty()) {
            self.tables.remove(&peer);
        }
        removed
    }

    /// Drops everything learned from `peer`. Called the moment a session leaves
    /// ESTABLISHED, so no stale path can survive the peer that taught it.
    pub fn clear_peer(&mut self, peer: Ipv4Address) -> usize {
        self.tables.remove(&peer).map(|t| t.len()).unwrap_or(0)
    }

    pub fn peer_table(&self, peer: Ipv4Address) -> Option<&BTreeMap<Ipv4Prefix, BgpPath>> {
        self.tables.get(&peer)
    }

    /// The stored paths from one peer, for in-place amendment.
    ///
    /// Used when something about the *peer* changes rather than the routes: a
    /// neighbour promoted to route reflector client is still advertising exactly
    /// what it was, but what may now be done with it has changed.
    pub fn peer_table_mut(
        &mut self,
        peer: Ipv4Address,
    ) -> Option<&mut BTreeMap<Ipv4Prefix, BgpPath>> {
        self.tables.get_mut(&peer)
    }

    pub fn prefix_count(&self, peer: Ipv4Address) -> usize {
        self.tables.get(&peer).map(|t| t.len()).unwrap_or(0)
    }

    /// Total number of stored paths across all peers.
    pub fn path_count(&self) -> usize {
        self.tables.values().map(|t| t.len()).sum()
    }

    pub fn peers(&self) -> Vec<Ipv4Address> {
        self.tables.keys().copied().collect()
    }

    /// Every prefix any peer has advertised, in address order.
    pub fn prefixes(&self) -> BTreeSet<Ipv4Prefix> {
        self.tables
            .values()
            .flat_map(|t| t.keys().copied())
            .collect()
    }

    /// All paths to `prefix`, ordered by peer address for determinism.
    pub fn candidates(&self, prefix: Ipv4Prefix) -> Vec<&BgpPath> {
        self.tables
            .values()
            .filter_map(|t| t.get(&prefix))
            .collect()
    }

    /// All stored paths, ordered by peer then prefix.
    pub fn iter_paths(&self) -> impl Iterator<Item = &BgpPath> {
        self.tables.values().flat_map(|t| t.values())
    }
}

/// The best path per prefix.
#[derive(Debug, Clone, Default)]
pub struct LocRib {
    best: BTreeMap<Ipv4Prefix, BgpPath>,
}

impl LocRib {
    pub fn new() -> Self {
        LocRib::default()
    }

    pub fn get(&self, prefix: &Ipv4Prefix) -> Option<&BgpPath> {
        self.best.get(prefix)
    }

    pub fn contains(&self, prefix: &Ipv4Prefix) -> bool {
        self.best.contains_key(prefix)
    }

    pub fn insert(&mut self, path: BgpPath) -> Option<BgpPath> {
        self.best.insert(path.prefix, path)
    }

    pub fn remove(&mut self, prefix: &Ipv4Prefix) -> Option<BgpPath> {
        self.best.remove(prefix)
    }

    pub fn len(&self) -> usize {
        self.best.len()
    }

    pub fn is_empty(&self) -> bool {
        self.best.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Ipv4Prefix, &BgpPath)> {
        self.best.iter()
    }

    pub fn prefixes(&self) -> Vec<Ipv4Prefix> {
        self.best.keys().copied().collect()
    }

    pub fn clear(&mut self) {
        self.best.clear();
    }
}

/// One route as advertised to one peer. Compared against the next computed
/// advertisement so an unchanged route is not re-sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvertisedRoute {
    pub origin: BgpOrigin,
    pub as_path: AsPath,
    pub next_hop: Ipv4Address,
    pub med: Option<u32>,
    pub local_pref: Option<u32>,
    /// ORIGINATOR_ID sent with this advertisement, present only when this
    /// speaker is reflecting the route (RFC 4456).
    pub originator_id: Option<Ipv4Address>,
    /// CLUSTER_LIST sent with this advertisement, with the local cluster ID
    /// already prepended when reflecting.
    pub cluster_list: Vec<Ipv4Address>,
}

/// What this speaker has advertised to each peer.
#[derive(Debug, Clone, Default)]
pub struct AdjRibOut {
    tables: BTreeMap<Ipv4Address, BTreeMap<Ipv4Prefix, AdvertisedRoute>>,
}

impl AdjRibOut {
    pub fn new() -> Self {
        AdjRibOut::default()
    }

    pub fn get(&self, peer: Ipv4Address, prefix: &Ipv4Prefix) -> Option<&AdvertisedRoute> {
        self.tables.get(&peer).and_then(|t| t.get(prefix))
    }

    pub fn insert(&mut self, peer: Ipv4Address, prefix: Ipv4Prefix, route: AdvertisedRoute) {
        self.tables.entry(peer).or_default().insert(prefix, route);
    }

    pub fn remove(&mut self, peer: Ipv4Address, prefix: &Ipv4Prefix) -> Option<AdvertisedRoute> {
        let removed = self.tables.get_mut(&peer).and_then(|t| t.remove(prefix));
        if self.tables.get(&peer).is_some_and(|t| t.is_empty()) {
            self.tables.remove(&peer);
        }
        removed
    }

    pub fn clear_peer(&mut self, peer: Ipv4Address) -> usize {
        self.tables.remove(&peer).map(|t| t.len()).unwrap_or(0)
    }

    pub fn prefix_count(&self, peer: Ipv4Address) -> usize {
        self.tables.get(&peer).map(|t| t.len()).unwrap_or(0)
    }

    pub fn prefixes(&self, peer: Ipv4Address) -> Vec<Ipv4Prefix> {
        self.tables
            .get(&peer)
            .map(|t| t.keys().copied().collect())
            .unwrap_or_default()
    }

    pub fn peer_table(&self, peer: Ipv4Address) -> Option<&BTreeMap<Ipv4Prefix, AdvertisedRoute>> {
        self.tables.get(&peer)
    }
}

// ============================================================================
// Route policy
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PolicyAction {
    #[default]
    Permit,
    Deny,
}

/// How a policy rule selects prefixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixMatch {
    /// Matches every prefix.
    Any,
    /// Matches only this exact prefix and length.
    Exact(Ipv4Prefix),
    /// Matches this prefix and anything more specific inside it.
    OrLonger(Ipv4Prefix),
}

impl PrefixMatch {
    pub fn matches(&self, prefix: Ipv4Prefix) -> bool {
        match self {
            PrefixMatch::Any => true,
            PrefixMatch::Exact(p) => *p == prefix,
            PrefixMatch::OrLonger(p) => prefix.length >= p.length && p.contains(prefix.address),
        }
    }
}

/// One ordered policy statement.
#[derive(Debug, Clone)]
pub struct PolicyRule {
    pub seq: u32,
    pub match_prefix: PrefixMatch,
    pub action: PolicyAction,
    pub set_local_pref: Option<u32>,
    pub set_med: Option<u32>,
}

impl PolicyRule {
    pub fn permit(seq: u32, match_prefix: PrefixMatch) -> Self {
        PolicyRule {
            seq,
            match_prefix,
            action: PolicyAction::Permit,
            set_local_pref: None,
            set_med: None,
        }
    }

    pub fn deny(seq: u32, match_prefix: PrefixMatch) -> Self {
        PolicyRule {
            seq,
            match_prefix,
            action: PolicyAction::Deny,
            set_local_pref: None,
            set_med: None,
        }
    }

    pub fn with_local_pref(mut self, lp: u32) -> Self {
        self.set_local_pref = Some(lp);
        self
    }

    pub fn with_med(mut self, med: u32) -> Self {
        self.set_med = Some(med);
        self
    }
}

/// The result of running a policy over one prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyOutcome {
    Denied,
    Permitted {
        set_local_pref: Option<u32>,
        set_med: Option<u32>,
    },
}

/// A small ordered prefix policy: first matching rule wins, and an unmatched prefix
/// falls through to `default_action`. Deliberately not a configuration language.
#[derive(Debug, Clone)]
pub struct RoutePolicy {
    rules: Vec<PolicyRule>,
    pub default_action: PolicyAction,
}

impl Default for RoutePolicy {
    fn default() -> Self {
        RoutePolicy {
            rules: Vec::new(),
            default_action: PolicyAction::Permit,
        }
    }
}

impl RoutePolicy {
    pub fn new() -> Self {
        RoutePolicy::default()
    }

    /// Adds a rule. Rules are evaluated in ascending sequence number; ties keep
    /// insertion order, so evaluation is never ambiguous.
    pub fn add_rule(&mut self, rule: PolicyRule) {
        self.rules.push(rule);
        self.rules.sort_by_key(|r| r.seq);
    }

    pub fn rules(&self) -> &[PolicyRule] {
        &self.rules
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty() && self.default_action == PolicyAction::Permit
    }

    pub fn apply(&self, prefix: Ipv4Prefix) -> PolicyOutcome {
        for rule in &self.rules {
            if rule.match_prefix.matches(prefix) {
                return match rule.action {
                    PolicyAction::Deny => PolicyOutcome::Denied,
                    PolicyAction::Permit => PolicyOutcome::Permitted {
                        set_local_pref: rule.set_local_pref,
                        set_med: rule.set_med,
                    },
                };
            }
        }
        match self.default_action {
            PolicyAction::Deny => PolicyOutcome::Denied,
            PolicyAction::Permit => PolicyOutcome::Permitted {
                set_local_pref: None,
                set_med: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(peer: u8, local_pref: u32, as_path: Vec<u32>, origin: BgpOrigin) -> BgpPath {
        BgpPath {
            prefix: Ipv4Prefix::new(Ipv4Address::new(10, 30, 0, 0), 24),
            source: PathSource::Ebgp,
            peer_addr: Ipv4Address::new(10, 0, 0, peer),
            peer_as: 65000 + peer as u32,
            peer_router_id: Ipv4Address::new(1, 1, 1, peer),
            origin,
            as_path: AsPath::sequence(as_path),
            next_hop: Ipv4Address::new(10, 0, 0, peer),
            med: None,
            local_pref,
            atomic_aggregate: false,
            originator_id: None,
            cluster_list: Vec::new(),
            from_client: false,
            received_at_ms: 0,
        }
    }

    #[test]
    fn test_local_pref_outranks_shorter_as_path() {
        let long_but_preferred = path(1, 200, vec![65002, 65004, 65009], BgpOrigin::Igp);
        let short = path(2, 100, vec![65003], BgpOrigin::Igp);
        let best = select_best(&[&short, &long_but_preferred]).unwrap();
        assert_eq!(best.peer_addr, Ipv4Address::new(10, 0, 0, 1));
    }

    #[test]
    fn test_as_path_then_origin_then_router_id() {
        let a = path(1, 100, vec![65002, 65004], BgpOrigin::Igp);
        let b = path(2, 100, vec![65003], BgpOrigin::Igp);
        assert_eq!(select_best(&[&a, &b]).unwrap().peer_addr, b.peer_addr);

        let c = path(3, 100, vec![65005], BgpOrigin::Incomplete);
        assert_eq!(select_best(&[&b, &c]).unwrap().peer_addr, b.peer_addr);

        // Identical in every attribute: the lower router-ID has to win, and the
        // answer must not depend on the order the candidates are presented in.
        let d = path(4, 100, vec![65005], BgpOrigin::Igp);
        let e = path(5, 100, vec![65006], BgpOrigin::Igp);
        assert_eq!(select_best(&[&d, &e]).unwrap().peer_addr, d.peer_addr);
        assert_eq!(select_best(&[&e, &d]).unwrap().peer_addr, d.peer_addr);
    }

    #[test]
    fn test_med_only_compared_within_the_same_neighbour_as() {
        let mut a = path(1, 100, vec![65002, 65009], BgpOrigin::Igp);
        a.med = Some(50);
        let mut b = path(2, 100, vec![65002, 65009], BgpOrigin::Igp);
        b.med = Some(10);
        // Same first AS, so MED decides and the lower value wins.
        assert_eq!(select_best(&[&a, &b]).unwrap().peer_addr, b.peer_addr);

        // Different first AS: MED is skipped and the router-ID tie-break decides.
        let mut c = path(3, 100, vec![65003, 65009], BgpOrigin::Igp);
        c.med = Some(9_000);
        assert_eq!(select_best(&[&c, &a]).unwrap().peer_addr, a.peer_addr);
    }

    #[test]
    fn test_ebgp_is_preferred_over_ibgp_when_everything_else_ties() {
        let external = path(1, 100, vec![65009], BgpOrigin::Igp);
        let mut internal = path(2, 100, vec![65009], BgpOrigin::Igp);
        internal.source = PathSource::Ibgp;
        // The internal path has the lower router-ID, so if eBGP preference were missing
        // the tie-break would pick it instead.
        internal.peer_router_id = Ipv4Address::new(0, 0, 0, 1);
        assert!(select_best(&[&external, &internal]).unwrap().is_ebgp());
        assert!(select_best(&[&internal, &external]).unwrap().is_ebgp());
    }

    #[test]
    fn test_locally_originated_path_wins() {
        let learned = path(1, 500, vec![65002], BgpOrigin::Igp);
        let local = BgpPath::local(
            Ipv4Prefix::new(Ipv4Address::new(10, 30, 0, 0), 24),
            Ipv4Address::new(10, 30, 0, 1),
            Ipv4Address::new(1, 1, 1, 1),
        );
        assert!(select_best(&[&learned, &local]).unwrap().is_local());
    }

    #[test]
    fn test_adj_rib_in_is_scoped_per_peer() {
        let mut rib = AdjRibIn::new();
        let p1 = path(1, 100, vec![65002], BgpOrigin::Igp);
        let p2 = path(2, 100, vec![65003], BgpOrigin::Igp);
        rib.insert(p1.peer_addr, p1.clone());
        rib.insert(p2.peer_addr, p2.clone());
        assert_eq!(rib.path_count(), 2);
        assert_eq!(rib.candidates(p1.prefix).len(), 2);

        assert_eq!(rib.clear_peer(p1.peer_addr), 1);
        assert_eq!(rib.path_count(), 1);
        assert_eq!(rib.candidates(p1.prefix).len(), 1);
        assert_eq!(rib.prefix_count(p1.peer_addr), 0);
    }

    #[test]
    fn test_policy_first_match_wins_and_sets_local_pref() {
        let target = Ipv4Prefix::new(Ipv4Address::new(10, 30, 0, 0), 24);
        let other = Ipv4Prefix::new(Ipv4Address::new(10, 40, 0, 0), 24);

        let mut policy = RoutePolicy::new();
        policy.add_rule(PolicyRule::deny(20, PrefixMatch::Exact(target)));
        policy.add_rule(PolicyRule::permit(10, PrefixMatch::Exact(target)).with_local_pref(300));

        // Sequence 10 is evaluated first even though it was added second.
        assert_eq!(
            policy.apply(target),
            PolicyOutcome::Permitted {
                set_local_pref: Some(300),
                set_med: None
            }
        );
        assert_eq!(
            policy.apply(other),
            PolicyOutcome::Permitted {
                set_local_pref: None,
                set_med: None
            }
        );

        let mut deny_all = RoutePolicy::new();
        deny_all.default_action = PolicyAction::Deny;
        assert_eq!(deny_all.apply(target), PolicyOutcome::Denied);
    }
}
