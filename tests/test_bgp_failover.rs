//! BGP-4 best-path selection, route policy, and failover.
//!
//! The diamond topology gives R1 two equal-length paths to the same destination. These
//! tests prove which one enters the real forwarding table, that host traffic follows it,
//! and that when the preferred session dies the alternate takes over automatically -
//! with no test ever touching a routing table.

mod common;

use common::bgp_lab::{
    AS1, AS2, AS3, AS4, best_as_path, bgp_fib_next_hop, build_diamond_lab, captured_echo_requests,
    converge_sessions, has_bgp_fib_route, ip, ping, prefix, run_until,
};
use toy_tcpip::bgp::Ipv4Prefix;
use toy_tcpip::bgp_rib::{PolicyRule, PrefixMatch, RoutePolicy};
use toy_tcpip::bgp_router::BgpState;
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::lab::VirtualLab;
use toy_tcpip::router::RouteSource;

const P_A: Ipv4Prefix = Ipv4Prefix {
    address: Ipv4Address([10, 1, 0, 0]),
    length: 24,
};
const P_D: Ipv4Prefix = Ipv4Prefix {
    address: Ipv4Address([10, 4, 0, 0]),
    length: 24,
};

/// R1's next hop toward R2, and toward R3.
const VIA_R2: Ipv4Address = Ipv4Address([10, 12, 0, 2]);
const VIA_R3: Ipv4Address = Ipv4Address([10, 13, 0, 3]);

/// Prefers the path through R2 by giving routes learned from R2 a higher LOCAL_PREF.
/// This is the only thing that makes the otherwise symmetric diamond decide one way.
fn prefer_r2(lab: &mut VirtualLab) {
    let mut policy = RoutePolicy::new();
    policy.add_rule(PolicyRule::permit(10, PrefixMatch::Any).with_local_pref(200));
    lab.router_mut("r1")
        .unwrap()
        .bgp_mut()
        .unwrap()
        .set_import_policy(VIA_R2, policy);
}

fn converge_diamond(lab: &mut VirtualLab) {
    assert!(
        converge_sessions(lab, 90_000),
        "the diamond never fully established"
    );
    assert!(
        run_until(lab, 90_000, |l| {
            has_bgp_fib_route(l, "r1", P_D) && has_bgp_fib_route(l, "r4", P_A)
        }),
        "the diamond never converged on both edge prefixes"
    );
}

// ============================================================================
// Best-path selection
// ============================================================================

#[test]
fn test_two_candidate_paths_arrive_and_local_pref_decides_which_enters_the_fib() {
    let mut lab = build_diamond_lab();
    prefer_r2(&mut lab);
    converge_diamond(&mut lab);

    let bgp = lab.router("r1").unwrap().bgp().unwrap();

    // 1. R1 really received two independent paths to the destination.
    let candidates = bgp.adj_rib_in.candidates(P_D);
    assert_eq!(
        candidates.len(),
        2,
        "expected one path from each neighbour, got {}",
        candidates.len()
    );
    let mut seen: Vec<Vec<u16>> = candidates.iter().map(|p| p.as_path.flatten()).collect();
    seen.sort();
    assert_eq!(seen, vec![vec![AS2, AS4], vec![AS3, AS4]]);
    // Both are the same length, so nothing but LOCAL_PREF can separate them.
    assert!(candidates.iter().all(|p| p.as_path.length() == 2));

    // 2. The policy gave the R2 path a higher LOCAL_PREF, so it wins.
    let best = bgp.loc_rib.get(&P_D).unwrap();
    assert_eq!(best.local_pref, 200);
    assert_eq!(best.peer_addr, VIA_R2);
    assert_eq!(best.as_path.flatten(), vec![AS2, AS4]);

    // 3. That is the path in the forwarding table.
    assert_eq!(bgp_fib_next_hop(&lab, "r1", P_D), Some(VIA_R2));
    let route = lab
        .router("r1")
        .unwrap()
        .routing_table
        .find_exact(P_D.address, P_D.length)
        .unwrap();
    assert_eq!(route.source, RouteSource::Bgp);
    assert_eq!(route.interface, "eth1");
}

#[test]
fn test_a_total_tie_is_broken_deterministically_by_the_peer_router_id() {
    // With no policy anywhere the two paths are identical in LOCAL_PREF, AS_PATH
    // length, ORIGIN, and MED, and both are eBGP. The documented final tie-break is
    // the lowest peer BGP identifier, and it must give the same answer every run.
    let mut lab = build_diamond_lab();
    converge_diamond(&mut lab);

    let bgp = lab.router("r1").unwrap().bgp().unwrap();
    let candidates = bgp.adj_rib_in.candidates(P_D);
    assert_eq!(candidates.len(), 2);
    assert!(candidates.iter().all(|p| p.local_pref == 100));
    assert!(candidates.iter().all(|p| p.as_path.length() == 2));

    let best = bgp.loc_rib.get(&P_D).unwrap();
    assert_eq!(
        best.peer_router_id,
        ip(2, 2, 2, 2),
        "the lowest BGP identifier should break a total tie"
    );
    assert_eq!(bgp_fib_next_hop(&lab, "r1", P_D), Some(VIA_R2));

    // The tie-break is a property of the attributes, not of arrival order: rebuilding
    // the same lab must produce the same winner.
    let mut lab2 = build_diamond_lab();
    converge_diamond(&mut lab2);
    assert_eq!(bgp_fib_next_hop(&lab2, "r1", P_D), Some(VIA_R2));
}

#[test]
fn test_a_prefix_denied_on_one_session_is_reached_the_long_way_round() {
    let mut lab = build_diamond_lab();
    // Make the R2 path longer by routing R4's prefix to R2 the long way: deny R4's
    // prefix on the R2 <-> R4 session, so R2 can only learn it via R1 <- R3 <- R4.
    let mut deny = RoutePolicy::new();
    deny.add_rule(PolicyRule::deny(10, PrefixMatch::Exact(P_D)));
    lab.router_mut("r2")
        .unwrap()
        .bgp_mut()
        .unwrap()
        .set_import_policy(ip(10, 24, 0, 4), deny);

    assert!(converge_sessions(&mut lab, 90_000));
    assert!(run_until(&mut lab, 90_000, |l| has_bgp_fib_route(
        l, "r1", P_D
    )));

    // R1 now has only the R3 path (R2 has nothing to offer for that prefix).
    let bgp = lab.router("r1").unwrap().bgp().unwrap();
    let best = bgp.loc_rib.get(&P_D).unwrap();
    assert_eq!(best.as_path.flatten(), vec![AS3, AS4]);
    assert_eq!(bgp_fib_next_hop(&lab, "r1", P_D), Some(VIA_R3));

    // R2 learned it back from R1 the long way round, so its path is three hops.
    assert!(
        run_until(&mut lab, 90_000, |l| {
            best_as_path(l, "r2", P_D) == Some(vec![AS1, AS3, AS4])
        }),
        "R2 should have learned the destination the long way, got {:?}",
        best_as_path(&lab, "r2", P_D)
    );
}

// ============================================================================
// Data plane follows the selected path
// ============================================================================

#[test]
fn test_host_traffic_follows_the_selected_path_and_not_the_alternate() {
    let mut lab = build_diamond_lab();
    prefer_r2(&mut lab);
    converge_diamond(&mut lab);
    lab.enable_pcap("l12");
    lab.enable_pcap("l13");

    assert!(
        ping(&mut lab, "host_a", ip(10, 4, 0, 2), 0x5005, 1, 60_000),
        "host A could not reach host D"
    );

    let via_r2 = captured_echo_requests(&lab.export_pcap("l12").unwrap());
    let via_r3 = captured_echo_requests(&lab.export_pcap("l13").unwrap());

    assert!(
        via_r2
            .iter()
            .any(|(s, d, _)| *s == ip(10, 1, 0, 2) && *d == ip(10, 4, 0, 2)),
        "the probe did not cross the preferred R1-R2 link"
    );
    assert!(
        !via_r3
            .iter()
            .any(|(s, d, _)| *s == ip(10, 1, 0, 2) && *d == ip(10, 4, 0, 2)),
        "the probe leaked onto the non-selected R1-R3 link"
    );
}

// ============================================================================
// Failover
// ============================================================================

#[test]
fn test_losing_the_preferred_session_moves_the_fib_to_the_alternate_path() {
    let mut lab = build_diamond_lab();
    prefer_r2(&mut lab);
    converge_diamond(&mut lab);

    // Baseline: the preferred path is in the FIB and traffic works.
    assert_eq!(bgp_fib_next_hop(&lab, "r1", P_D), Some(VIA_R2));
    assert!(ping(&mut lab, "host_a", ip(10, 4, 0, 2), 0x6006, 1, 60_000));

    // Cut the preferred link. Nothing else is touched: no route is added or removed by
    // the test, and the FSM is not poked.
    lab.link_mut("l12").unwrap().set_blackhole(true);

    assert!(
        run_until(&mut lab, 180_000, |l| {
            bgp_fib_next_hop(l, "r1", P_D) == Some(VIA_R3)
        }),
        "the FIB never moved to the alternate path (still {:?})",
        bgp_fib_next_hop(&lab, "r1", P_D)
    );

    // The session with R2 is down and everything it taught is gone.
    let bgp = lab.router("r1").unwrap().bgp().unwrap();
    assert_ne!(bgp.peer_state(VIA_R2), Some(BgpState::Established));
    assert_eq!(bgp.adj_rib_in.prefix_count(VIA_R2), 0);
    assert_eq!(
        bgp.peer_state(VIA_R3),
        Some(BgpState::Established),
        "the surviving session must be unaffected"
    );

    // The alternate path is now best, and it is the one in the table.
    let best = bgp.loc_rib.get(&P_D).unwrap();
    assert_eq!(best.as_path.flatten(), vec![AS3, AS4]);
    assert_eq!(best.peer_addr, VIA_R3);
    let route = lab
        .router("r1")
        .unwrap()
        .routing_table
        .find_exact(P_D.address, P_D.length)
        .unwrap();
    assert_eq!(route.source, RouteSource::Bgp);
    assert_eq!(route.interface, "eth2");

    // There is exactly one BGP entry for the prefix: the old one was replaced, not
    // left behind alongside the new one.
    assert_eq!(
        lab.router("r1")
            .unwrap()
            .routing_table
            .routes_from(RouteSource::Bgp)
            .iter()
            .filter(|r| r.destination == P_D.address && r.prefix_len == P_D.length)
            .count(),
        1
    );
}

#[test]
fn test_connectivity_recovers_through_the_alternate_path_after_a_failure() {
    let mut lab = build_diamond_lab();
    prefer_r2(&mut lab);
    converge_diamond(&mut lab);
    assert!(ping(&mut lab, "host_a", ip(10, 4, 0, 2), 0x7007, 1, 60_000));

    lab.link_mut("l12").unwrap().set_blackhole(true);
    assert!(
        run_until(&mut lab, 180_000, |l| {
            bgp_fib_next_hop(l, "r1", P_D) == Some(VIA_R3)
                && bgp_fib_next_hop(l, "r4", P_A) == Some(ip(10, 34, 0, 3))
        }),
        "both directions did not reconverge onto the surviving path"
    );

    // Capture only the surviving path, then prove the data plane really uses it.
    lab.enable_pcap("l13");
    lab.enable_pcap("l34");
    assert!(
        ping(&mut lab, "host_a", ip(10, 4, 0, 2), 0x7007, 2, 60_000),
        "connectivity did not recover after failover"
    );

    let on_l13 = captured_echo_requests(&lab.export_pcap("l13").unwrap());
    let on_l34 = captured_echo_requests(&lab.export_pcap("l34").unwrap());
    assert!(
        on_l13
            .iter()
            .any(|(_, d, seq)| *d == ip(10, 4, 0, 2) && *seq == 2),
        "recovered traffic did not use R1 -> R3"
    );
    assert!(
        on_l34
            .iter()
            .any(|(_, d, seq)| *d == ip(10, 4, 0, 2) && *seq == 2),
        "recovered traffic did not use R3 -> R4"
    );
}

#[test]
fn test_an_administrative_shutdown_withdraws_downstream() {
    let mut lab = build_diamond_lab();
    prefer_r2(&mut lab);
    converge_diamond(&mut lab);
    assert_eq!(bgp_fib_next_hop(&lab, "r1", P_D), Some(VIA_R2));

    // R2 shuts its session with R4 down administratively. It then has nothing to offer
    // for that prefix and must withdraw it from R1, which falls back to R3.
    lab.router_mut("r2")
        .unwrap()
        .bgp_shutdown_peer(ip(10, 24, 0, 4));

    assert!(
        run_until(&mut lab, 120_000, |l| {
            bgp_fib_next_hop(l, "r1", P_D) == Some(VIA_R3)
        }),
        "R1 did not fall back after R2 lost its own upstream"
    );

    let r2 = lab.router("r2").unwrap().bgp().unwrap();
    assert_eq!(r2.peer_state(ip(10, 24, 0, 4)), Some(BgpState::Idle));
    assert_eq!(
        r2.adj_rib_in.prefix_count(ip(10, 24, 0, 4)),
        0,
        "R2 kept paths from the peer it shut down"
    );
    // R2 may well relearn the prefix the long way round, from R1. If it does, the path
    // must be the one R1 offered and must never be advertised back to R1.
    if let Some(best) = r2.loc_rib.get(&P_D) {
        assert_eq!(best.peer_addr, ip(10, 12, 0, 1));
        assert!(
            best.as_path.contains(AS1),
            "a relearned path must carry AS65001, got [{}]",
            best.as_path
        );
    }
    assert!(
        !r2.adj_rib_out.prefixes(ip(10, 12, 0, 1)).contains(&P_D),
        "R2 advertised a route straight back to the peer it learned it from"
    );

    // Traffic still gets through, the long way.
    assert!(ping(&mut lab, "host_a", ip(10, 4, 0, 2), 0x8008, 1, 60_000));
}

// ============================================================================
// Route policy
// ============================================================================

#[test]
fn test_an_import_deny_blocks_a_prefix_from_ever_reaching_the_fib() {
    let mut lab = build_diamond_lab();
    // R1 refuses the destination prefix from R2 but accepts everything else from it.
    let mut policy = RoutePolicy::new();
    policy.add_rule(PolicyRule::deny(10, PrefixMatch::Exact(P_D)));
    lab.router_mut("r1")
        .unwrap()
        .bgp_mut()
        .unwrap()
        .set_import_policy(VIA_R2, policy);

    converge_diamond(&mut lab);

    let bgp = lab.router("r1").unwrap().bgp().unwrap();
    // Only one candidate survived import, and it is the R3 one.
    let candidates = bgp.adj_rib_in.candidates(P_D);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].peer_addr, VIA_R3);
    assert!(bgp.peer(VIA_R2).unwrap().counters.policy_rejected > 0);
    assert_eq!(bgp_fib_next_hop(&lab, "r1", P_D), Some(VIA_R3));

    // The session with R2 is healthy and still carrying other prefixes.
    assert_eq!(bgp.peer_state(VIA_R2), Some(BgpState::Established));
}

#[test]
fn test_an_export_deny_stops_an_advertisement_at_the_source() {
    let mut lab = build_diamond_lab();
    // R4 refuses to advertise its prefix to R2 at all.
    let mut policy = RoutePolicy::new();
    policy.add_rule(PolicyRule::deny(10, PrefixMatch::Exact(P_D)));
    lab.router_mut("r4")
        .unwrap()
        .bgp_mut()
        .unwrap()
        .set_export_policy(ip(10, 24, 0, 2), policy);

    assert!(converge_sessions(&mut lab, 90_000));
    assert!(run_until(&mut lab, 90_000, |l| has_bgp_fib_route(
        l, "r1", P_D
    )));

    // R2 never heard about the prefix from R4.
    let r2 = lab.router("r2").unwrap().bgp().unwrap();
    assert!(
        !r2.adj_rib_in
            .peer_table(ip(10, 24, 0, 4))
            .map(|t| t.contains_key(&P_D))
            .unwrap_or(false),
        "the export deny leaked the prefix to R2"
    );
    assert!(!r4_advertised_to_r2(&lab).contains(&P_D));

    // R1 therefore has exactly one path, through R3.
    assert_eq!(bgp_fib_next_hop(&lab, "r1", P_D), Some(VIA_R3));
}

fn r4_advertised_to_r2(lab: &VirtualLab) -> Vec<Ipv4Prefix> {
    lab.router("r4")
        .unwrap()
        .bgp()
        .unwrap()
        .adj_rib_out
        .prefixes(ip(10, 24, 0, 2))
}

#[test]
fn test_policy_local_pref_flips_the_winner_the_other_way() {
    // Same topology, opposite preference: the tie-break alone would choose R2, so a
    // policy that makes R3 preferred proves policy really drives the decision.
    let mut lab = build_diamond_lab();
    let mut policy = RoutePolicy::new();
    policy.add_rule(
        PolicyRule::permit(10, PrefixMatch::OrLonger(prefix(10, 4, 0, 0, 16))).with_local_pref(300),
    );
    lab.router_mut("r1")
        .unwrap()
        .bgp_mut()
        .unwrap()
        .set_import_policy(VIA_R3, policy);

    converge_diamond(&mut lab);

    let bgp = lab.router("r1").unwrap().bgp().unwrap();
    let best = bgp.loc_rib.get(&P_D).unwrap();
    assert_eq!(best.local_pref, 300);
    assert_eq!(best.peer_addr, VIA_R3);
    assert_eq!(best.as_path.flatten(), vec![AS3, AS4]);
    assert_eq!(bgp_fib_next_hop(&lab, "r1", P_D), Some(VIA_R3));

    // The R1 prefix, which the rule does not match, is unaffected and still 100.
    let r1_own = bgp.loc_rib.get(&P_A);
    assert!(r1_own.is_some_and(|p| p.is_local()));
}

// ============================================================================
// MULTI_EXIT_DISC
// ============================================================================

#[test]
fn test_med_decides_between_two_entry_points_into_the_same_neighbour_as() {
    use common::bgp_lab::build_med_lab;

    let target = prefix(10, 9, 0, 0, 24);
    let via_r2 = ip(10, 12, 0, 2);
    let via_r3 = ip(10, 13, 0, 3);

    // R3 attaches the lower MED, so R3 is the preferred entry point.
    let mut lab = build_med_lab(50, 10);
    assert!(converge_sessions(&mut lab, 90_000));
    assert!(run_until(&mut lab, 90_000, |l| {
        l.router("r1")
            .unwrap()
            .bgp()
            .unwrap()
            .adj_rib_in
            .candidates(target)
            .len()
            == 2
    }));

    let bgp = lab.router("r1").unwrap().bgp().unwrap();
    let candidates = bgp.adj_rib_in.candidates(target);
    // Both paths are identical except for the MED, and both start with AS65002, which
    // is what makes the comparison meaningful in the first place.
    assert!(candidates.iter().all(|p| p.local_pref == 100));
    assert!(candidates.iter().all(|p| p.as_path.flatten() == vec![AS2]));
    let mut meds: Vec<Option<u32>> = candidates.iter().map(|p| p.med).collect();
    meds.sort();
    assert_eq!(meds, vec![Some(10), Some(50)]);

    let best = bgp.loc_rib.get(&target).unwrap();
    assert_eq!(best.med, Some(10));
    assert_eq!(best.peer_addr, via_r3);
    assert_eq!(bgp_fib_next_hop(&lab, "r1", target), Some(via_r3));

    // Reverse the MEDs and the winner reverses with them, which rules out the
    // router-ID tie-break having been what decided it.
    let mut lab = build_med_lab(10, 50);
    assert!(converge_sessions(&mut lab, 90_000));
    assert!(run_until(&mut lab, 90_000, |l| {
        bgp_fib_next_hop(l, "r1", target).is_some()
    }));
    let best = lab
        .router("r1")
        .unwrap()
        .bgp()
        .unwrap()
        .loc_rib
        .get(&target)
        .unwrap()
        .clone();
    assert_eq!(best.med, Some(10));
    assert_eq!(best.peer_addr, via_r2);
    assert_eq!(bgp_fib_next_hop(&lab, "r1", target), Some(via_r2));
}

// ============================================================================
// iBGP rules
// ============================================================================

#[test]
fn test_ibgp_does_not_prepend_and_ebgp_does() {
    use common::bgp_lab::build_ibgp_lab;
    use toy_tcpip::bgp_rib::PathSource;

    let mut lab = build_ibgp_lab();
    let target = prefix(10, 4, 0, 0, 24);
    assert!(converge_sessions(&mut lab, 120_000));
    assert!(
        run_until(&mut lab, 120_000, |l| {
            l.router("r1")
                .unwrap()
                .bgp()
                .unwrap()
                .loc_rib
                .contains(&target)
        }),
        "the prefix never reached R1 through the AS65002 core"
    );

    // R3 learned it over eBGP from R4: one AS on the path.
    let r3 = lab.router("r3").unwrap().bgp().unwrap();
    assert_eq!(
        r3.loc_rib.get(&target).unwrap().as_path.flatten(),
        vec![AS4]
    );

    // R2 learned it over iBGP from R3. Crossing an iBGP session must NOT prepend, so
    // the path is still one AS long, and the source is recorded as internal.
    let r2 = lab.router("r2").unwrap().bgp().unwrap();
    let r2_best = r2.loc_rib.get(&target).unwrap();
    assert_eq!(
        r2_best.as_path.flatten(),
        vec![AS4],
        "iBGP prepended the local ASN"
    );
    assert_eq!(r2_best.source, PathSource::Ibgp);
    // next-hop-self made the next hop R3's address on the shared subnet, so R2 can
    // actually resolve it and install the route.
    assert_eq!(r2_best.next_hop, ip(10, 23, 0, 3));
    assert_eq!(bgp_fib_next_hop(&lab, "r2", target), Some(ip(10, 23, 0, 3)));

    // R1 is in a different AS, so R2 prepended on the way out.
    assert_eq!(
        best_as_path(&lab, "r1", target),
        Some(vec![AS2, AS4]),
        "eBGP export did not prepend the local ASN"
    );
}

#[test]
fn test_an_ibgp_learned_route_is_not_re_advertised_to_another_ibgp_peer() {
    use common::bgp_lab::build_ibgp_lab;

    let mut lab = build_ibgp_lab();
    let from_ebgp = prefix(10, 1, 0, 0, 24); // originated by R1, reaches R2 over eBGP
    let from_ibgp = prefix(10, 4, 0, 0, 24); // reaches R2 over iBGP from R3

    assert!(converge_sessions(&mut lab, 120_000));
    assert!(run_until(&mut lab, 120_000, |l| {
        l.router("r5")
            .unwrap()
            .bgp()
            .unwrap()
            .loc_rib
            .contains(&from_ebgp)
    }));
    // Give any (incorrect) re-advertisement plenty of simulated time to show up.
    for _ in 0..10 {
        lab.advance_time(1_000);
        lab.run_pumped(30);
    }

    let r5 = lab.router("r5").unwrap().bgp().unwrap();
    // R2 learned 10.1.0.0/24 externally, so passing it to an internal peer is correct.
    assert!(
        r5.loc_rib.contains(&from_ebgp),
        "R5 should have learned the externally sourced prefix"
    );
    assert_eq!(
        r5.loc_rib.get(&from_ebgp).unwrap().as_path.flatten(),
        vec![AS1]
    );

    // R2 learned 10.4.0.0/24 from another internal peer, so it must not pass it on.
    assert!(
        !r5.loc_rib.contains(&from_ibgp),
        "an iBGP-learned route was re-advertised to another iBGP peer"
    );
    assert!(
        !lab.router("r2")
            .unwrap()
            .bgp()
            .unwrap()
            .adj_rib_out
            .prefixes(ip(10, 25, 0, 5))
            .contains(&from_ibgp)
    );
}
