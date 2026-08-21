//! BGP-4 control plane: UPDATE exchange over TCP, the three RIBs, AS_PATH handling,
//! withdrawal, FIB installation, and the resulting IPv4 data plane.
//!
//! The chain under test is the whole one:
//!
//! ```text
//! BGP peer -> TCP socket -> OPEN/KEEPALIVE/UPDATE -> Adj-RIB-In -> best path
//!          -> Loc-RIB -> RoutingTable -> IPv4 forwarding -> host traffic
//! ```
//!
//! No test in this file installs a route by hand.

mod common;

use common::bgp_lab::{
    AS1, AS2, AS3, LAB_HOLD_TIME, best_as_path, build_linear_lab, captured_echo_requests,
    converge_sessions, has_bgp_fib_route, ip, ping, prefix, run_until,
};
use toy_tcpip::bgp::{
    AsPath, BgpOrigin, BgpPathAttributes, BgpPdu, BgpUpdateMessage, Ipv4Prefix,
    peek_bgp_message_type,
};
use toy_tcpip::bgp_rib::PathSource;
use toy_tcpip::bgp_router::{BgpPeerMode, BgpState};
use toy_tcpip::ethernet::{EtherType, EthernetFrame, MacAddress};
use toy_tcpip::ipv4::{IpProtocol, Ipv4Address, Ipv4Packet};
use toy_tcpip::lab::{LabRouter, VirtualLab};
use toy_tcpip::pcap::PcapReader;
use toy_tcpip::router::RouteSource;
use toy_tcpip::stack::NetStackConfig;
use toy_tcpip::tcp::TcpSegment;

const P_A: Ipv4Prefix = Ipv4Prefix {
    address: Ipv4Address([10, 1, 0, 0]),
    length: 24,
};
const P_C: Ipv4Prefix = Ipv4Prefix {
    address: Ipv4Address([10, 3, 0, 0]),
    length: 24,
};

/// Runs the linear lab until R1 has installed 10.3.0.0/24 and R3 has installed
/// 10.1.0.0/24, i.e. until BGP has converged in both directions.
fn converge_linear(lab: &mut VirtualLab) {
    assert!(
        converge_sessions(lab, 60_000),
        "BGP sessions never established"
    );
    assert!(
        run_until(lab, 60_000, |l| {
            has_bgp_fib_route(l, "r1", P_C) && has_bgp_fib_route(l, "r3", P_A)
        }),
        "routes never reached both edge routers"
    );
}

// ============================================================================
// UPDATE propagation across autonomous systems
// ============================================================================

#[test]
fn test_route_propagates_across_three_autonomous_systems_with_correct_as_path() {
    let mut lab = build_linear_lab();

    // 1. Before anything runs, R1 knows nothing about R3's prefix.
    assert!(
        lab.router("r1")
            .unwrap()
            .routing_table
            .find_exact(P_C.address, P_C.length)
            .is_none(),
        "R1 started with a route it should have had to learn"
    );
    assert!(lab.router("r1").unwrap().bgp().unwrap().loc_rib.is_empty());

    converge_linear(&mut lab);

    // 2. R2 learned it straight from R3, which prepended only its own ASN.
    assert_eq!(
        best_as_path(&lab, "r2", P_C),
        Some(vec![AS3]),
        "R2 should see AS_PATH [65003]"
    );

    // 3. R1 learned it through R2, which prepended AS65002 in front.
    assert_eq!(
        best_as_path(&lab, "r1", P_C),
        Some(vec![AS2, AS3]),
        "R1 should see AS_PATH [65002 65003]"
    );

    // 4. And the reverse direction is symmetric.
    assert_eq!(best_as_path(&lab, "r2", P_A), Some(vec![AS1]));
    assert_eq!(best_as_path(&lab, "r3", P_A), Some(vec![AS2, AS1]));

    // 5. The NEXT_HOP was rewritten by each eBGP speaker to its own address.
    let r1_best = lab
        .router("r1")
        .unwrap()
        .bgp()
        .unwrap()
        .loc_rib
        .get(&P_C)
        .unwrap()
        .clone();
    assert_eq!(r1_best.next_hop, ip(10, 12, 0, 2));
    assert_eq!(r1_best.source, PathSource::Ebgp);
    assert_eq!(r1_best.peer_as, AS2);
    assert_eq!(r1_best.origin, BgpOrigin::Igp);

    // 6. Real UPDATE messages were exchanged over the sessions.
    let r2 = lab.router("r2").unwrap().bgp().unwrap();
    let from_r3 = r2.peer(ip(10, 23, 0, 3)).unwrap();
    assert!(from_r3.counters.updates_received >= 1);
    let to_r1 = r2.peer(ip(10, 12, 0, 1)).unwrap();
    assert!(to_r1.counters.updates_sent >= 1);
}

#[test]
fn test_the_three_ribs_are_separate_and_consistent() {
    let mut lab = build_linear_lab();
    converge_linear(&mut lab);

    let r2 = lab.router("r2").unwrap().bgp().unwrap();

    // Adj-RIB-In is per peer: R3's prefix arrived from R3, R1's from R1.
    assert!(
        r2.adj_rib_in
            .peer_table(ip(10, 23, 0, 3))
            .unwrap()
            .contains_key(&P_C)
    );
    assert!(
        r2.adj_rib_in
            .peer_table(ip(10, 12, 0, 1))
            .unwrap()
            .contains_key(&P_A)
    );
    assert_eq!(r2.adj_rib_in.prefix_count(ip(10, 23, 0, 3)), 1);
    assert_eq!(r2.adj_rib_in.prefix_count(ip(10, 12, 0, 1)), 1);

    // Loc-RIB holds one best path per prefix.
    assert_eq!(r2.loc_rib.len(), 2);

    // Adj-RIB-Out records what each peer was actually told, and split horizon means
    // R3 is never told about its own prefix.
    let to_r3 = r2.adj_rib_out.prefixes(ip(10, 23, 0, 3));
    assert_eq!(
        to_r3,
        vec![P_A],
        "R2 should advertise only 10.1.0.0/24 to R3"
    );
    let to_r1 = r2.adj_rib_out.prefixes(ip(10, 12, 0, 1));
    assert_eq!(
        to_r1,
        vec![P_C],
        "R2 should advertise only 10.3.0.0/24 to R1"
    );

    // What R2 recorded as advertised matches what R1 actually holds.
    let advertised = r2.adj_rib_out.get(ip(10, 12, 0, 1), &P_C).unwrap().clone();
    let received = lab
        .router("r1")
        .unwrap()
        .bgp()
        .unwrap()
        .adj_rib_in
        .peer_table(ip(10, 12, 0, 2))
        .unwrap()
        .get(&P_C)
        .unwrap()
        .clone();
    assert_eq!(advertised.as_path.flatten(), received.as_path.flatten());
    assert_eq!(advertised.next_hop, received.next_hop);
}

#[test]
fn test_a_locally_originated_prefix_is_not_reflected_back_to_its_source() {
    let mut lab = build_linear_lab();
    converge_linear(&mut lab);

    // R3 originates 10.3.0.0/24. It must never end up with a *learned* path to it,
    // and must never install a BGP FIB entry over its own connected subnet.
    let r3 = lab.router("r3").unwrap().bgp().unwrap();
    assert!(
        r3.adj_rib_in
            .candidates(P_C)
            .iter()
            .all(|p| p.source != PathSource::Ebgp),
        "R3 received its own prefix back"
    );
    assert!(r3.loc_rib.get(&P_C).unwrap().is_local());
    assert!(
        !has_bgp_fib_route(&lab, "r3", P_C),
        "R3 installed a BGP route over its own connected subnet"
    );

    // The connected route is still what forwarding uses.
    let route = lab
        .router("r3")
        .unwrap()
        .routing_table
        .find_exact(P_C.address, P_C.length)
        .unwrap();
    assert_eq!(route.source, RouteSource::Connected);
}

// ============================================================================
// FIB installation
// ============================================================================

#[test]
fn test_the_selected_path_is_installed_in_the_real_forwarding_table() {
    let mut lab = build_linear_lab();
    converge_linear(&mut lab);

    let r1 = lab.router("r1").unwrap();
    let route = r1
        .routing_table
        .find_exact(P_C.address, P_C.length)
        .expect("R1 has no route to 10.3.0.0/24");

    // The entry is attributed to BGP, points at the eBGP next hop, and leaves through
    // the interface that reaches it.
    assert_eq!(route.source, RouteSource::Bgp);
    assert_eq!(route.gateway, Some(ip(10, 12, 0, 2)));
    assert_eq!(route.interface, "eth1");

    // And it is the entry an actual forwarding lookup would pick.
    let looked_up = r1.routing_table.lookup(ip(10, 3, 0, 2)).unwrap();
    assert_eq!(looked_up.source, RouteSource::Bgp);
    assert_eq!(looked_up.gateway, Some(ip(10, 12, 0, 2)));

    // Exactly one BGP route on R1: 10.3.0.0/24. Its own prefix stays connected.
    let bgp_routes: Vec<String> = r1
        .routing_table
        .routes_from(RouteSource::Bgp)
        .iter()
        .map(|r| format!("{}/{}", r.destination, r.prefix_len))
        .collect();
    assert_eq!(bgp_routes, vec!["10.3.0.0/24".to_string()]);

    // The speaker's own bookkeeping agrees with the table.
    assert_eq!(
        lab.router("r1")
            .unwrap()
            .bgp()
            .unwrap()
            .installed_prefixes(),
        vec![P_C]
    );
}

#[test]
fn test_connected_routes_outrank_a_bgp_route_for_the_same_prefix() {
    let mut lab = build_linear_lab();
    // R2 also originates R3's prefix, so R3 will hear a BGP path to a subnet it owns.
    lab.router_mut("r2")
        .unwrap()
        .bgp_mut()
        .unwrap()
        .originate(P_C, ip(10, 23, 0, 2));

    converge_linear(&mut lab);

    // R3's forwarding decision for its own LAN must still be the connected interface.
    let r3 = lab.router("r3").unwrap();
    let chosen = r3.routing_table.lookup(ip(10, 3, 0, 2)).unwrap();
    assert_eq!(chosen.source, RouteSource::Connected);
    assert_eq!(chosen.interface, "eth1");
    assert_eq!(chosen.gateway, None);
}

// ============================================================================
// End-to-end data plane
// ============================================================================

#[test]
fn test_host_traffic_flows_end_to_end_over_the_bgp_learned_path() {
    let mut lab = build_linear_lab();
    lab.enable_pcap("r2r3");

    // Before convergence there is no path at all, which is what makes the later
    // success attributable to BGP.
    assert!(!ping(&mut lab, "host_a", ip(10, 3, 0, 2), 0x1001, 1, 3_000));

    converge_linear(&mut lab);

    // Now the same probe must succeed, using only routes BGP installed.
    assert!(
        ping(&mut lab, "host_a", ip(10, 3, 0, 2), 0x1001, 2, 30_000),
        "host A could not reach host C over the BGP-learned route"
    );

    // The reply came from host C itself.
    let replies = &lab.host("host_a").unwrap().stack.received_icmp_replies;
    assert!(
        replies
            .iter()
            .any(|(src, id, seq)| *src == ip(10, 3, 0, 2) && *id == 0x1001 && *seq == 2)
    );

    // The traffic really crossed the R2 <-> R3 link, so it took R1 -> R2 -> R3.
    let pcap = lab.export_pcap("r2r3").expect("capture");
    let echoes = captured_echo_requests(&pcap);
    assert!(
        echoes
            .iter()
            .any(|(s, d, seq)| *s == ip(10, 1, 0, 2) && *d == ip(10, 3, 0, 2) && *seq == 2),
        "no echo request from 10.1.0.2 to 10.3.0.2 on the R2-R3 link: {:?}",
        echoes
    );

    // And nobody installed the destination route by hand: it is BGP-sourced on every
    // transit router along the way.
    for r in ["r1", "r2"] {
        assert_eq!(
            lab.router(r)
                .unwrap()
                .routing_table
                .find_exact(P_C.address, P_C.length)
                .unwrap()
                .source,
            RouteSource::Bgp
        );
    }
}

#[test]
fn test_capture_contains_the_whole_session_from_arp_to_data_plane() {
    let mut lab = build_linear_lab();
    lab.enable_pcap("r1r2");
    converge_linear(&mut lab);
    assert!(ping(&mut lab, "host_a", ip(10, 3, 0, 2), 0x2002, 1, 30_000));

    // Withdrawing at the far end puts a withdrawal UPDATE on this link too.
    lab.router_mut("r3")
        .unwrap()
        .bgp_mut()
        .unwrap()
        .withdraw_originated(P_C);
    assert!(run_until(&mut lab, 30_000, |l| !has_bgp_fib_route(
        l, "r1", P_C
    )));

    let pcap = lab.export_pcap("r1r2").expect("capture");
    let mut reader = PcapReader::new(std::io::Cursor::new(pcap)).expect("the reader must parse it");
    let packets = reader.read_all_packets().expect("read packets");
    assert!(packets.len() > 10, "capture is suspiciously short");

    let mut saw_arp = false;
    let mut saw_syn = false;
    let mut saw_syn_ack = false;
    let mut saw_ack = false;
    let mut saw_icmp = false;
    let mut bgp_types = std::collections::BTreeSet::new();

    for pkt in &packets {
        let Ok(eth) = EthernetFrame::parse(&pkt.data) else {
            continue;
        };
        match eth.ethertype {
            EtherType::Arp => saw_arp = true,
            EtherType::IPv4 => {
                let Ok(ipv4) = Ipv4Packet::parse(eth.payload, false) else {
                    continue;
                };
                match ipv4.header.protocol {
                    IpProtocol::Icmp => saw_icmp = true,
                    IpProtocol::Tcp => {
                        let Ok(seg) = TcpSegment::parse(
                            ipv4.header.src_ip,
                            ipv4.header.dst_ip,
                            ipv4.payload,
                            false,
                        ) else {
                            continue;
                        };
                        // Wireshark keys BGP off TCP port 179, and so does this check.
                        assert!(
                            seg.src_port == 179 || seg.dst_port == 179,
                            "unexpected TCP port pair {} -> {}",
                            seg.src_port,
                            seg.dst_port
                        );
                        if seg.flags.syn && !seg.flags.ack {
                            saw_syn = true;
                        } else if seg.flags.syn && seg.flags.ack {
                            saw_syn_ack = true;
                        } else if seg.flags.ack && seg.payload.is_empty() {
                            saw_ack = true;
                        }
                        // BGP messages start on a segment boundary here, so the first
                        // 19 bytes of a data segment are a BGP header.
                        if let Some(t) = peek_bgp_message_type(seg.payload) {
                            bgp_types.insert(t);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    assert!(saw_arp, "no ARP in the capture");
    assert!(saw_syn, "no TCP SYN");
    assert!(saw_syn_ack, "no TCP SYN-ACK");
    assert!(saw_ack, "no bare TCP ACK");
    assert!(saw_icmp, "no data-plane packets using the learned route");
    assert!(bgp_types.contains(&1), "no BGP OPEN in the capture");
    assert!(bgp_types.contains(&2), "no BGP UPDATE in the capture");
    assert!(bgp_types.contains(&4), "no BGP KEEPALIVE in the capture");
}

// ============================================================================
// Withdrawal
// ============================================================================

#[test]
fn test_withdrawal_propagates_and_clears_the_fib_everywhere() {
    let mut lab = build_linear_lab();
    converge_linear(&mut lab);
    assert!(ping(&mut lab, "host_a", ip(10, 3, 0, 2), 0x3003, 1, 30_000));

    // R3 stops originating. It sends a withdrawal to R2, which sends one to R1.
    assert!(
        lab.router_mut("r3")
            .unwrap()
            .bgp_mut()
            .unwrap()
            .withdraw_originated(P_C)
    );

    assert!(
        run_until(&mut lab, 60_000, |l| {
            !l.router("r1")
                .unwrap()
                .bgp()
                .unwrap()
                .loc_rib
                .contains(&P_C)
                && !l
                    .router("r2")
                    .unwrap()
                    .bgp()
                    .unwrap()
                    .loc_rib
                    .contains(&P_C)
        }),
        "the withdrawal did not reach both routers"
    );

    // Nothing stale is left: not in any Adj-RIB-In, not in any Loc-RIB, not in any FIB.
    for r in ["r1", "r2"] {
        let bgp = lab.router(r).unwrap().bgp().unwrap();
        assert!(
            bgp.adj_rib_in.candidates(P_C).is_empty(),
            "{} kept a path",
            r
        );
        assert!(!bgp.loc_rib.contains(&P_C), "{} kept a best path", r);
        assert!(
            !bgp.installed_prefixes().contains(&P_C),
            "{} still thinks it installed the prefix",
            r
        );
        assert!(
            lab.router(r)
                .unwrap()
                .routing_table
                .find_exact(P_C.address, P_C.length)
                .is_none(),
            "{} left a stale forwarding entry",
            r
        );
    }

    // The data plane agrees: the destination is now unreachable from host A.
    assert!(
        !ping(&mut lab, "host_a", ip(10, 3, 0, 2), 0x3003, 2, 5_000),
        "traffic still flowed after the route was withdrawn"
    );

    // The reverse direction was never touched.
    assert!(has_bgp_fib_route(&lab, "r3", P_A));
}

#[test]
fn test_session_loss_purges_only_that_peers_routes() {
    let mut lab = build_linear_lab();
    converge_linear(&mut lab);

    // Cut R2 <-> R3. R2 must forget everything R3 taught it, and tell R1.
    lab.link_mut("r2r3").unwrap().set_blackhole(true);

    assert!(
        run_until(&mut lab, 120_000, |l| {
            l.router("r2")
                .unwrap()
                .bgp()
                .unwrap()
                .peer_state(ip(10, 23, 0, 3))
                != Some(BgpState::Established)
                && !has_bgp_fib_route(l, "r1", P_C)
        }),
        "the far-side failure never reached R1"
    );

    let r2 = lab.router("r2").unwrap().bgp().unwrap();
    assert_eq!(r2.adj_rib_in.prefix_count(ip(10, 23, 0, 3)), 0);
    assert!(!r2.loc_rib.contains(&P_C));
    // The surviving session with R1 is untouched and still carries R1's prefix.
    assert_eq!(r2.peer_state(ip(10, 12, 0, 1)), Some(BgpState::Established));
    assert!(r2.loc_rib.contains(&P_A));
    assert_eq!(r2.adj_rib_in.prefix_count(ip(10, 12, 0, 1)), 1);

    assert!(
        lab.router("r1")
            .unwrap()
            .routing_table
            .find_exact(P_C.address, P_C.length)
            .is_none(),
        "R1 kept a forwarding entry pointing through a dead path"
    );
    assert!(!ping(&mut lab, "host_a", ip(10, 3, 0, 2), 0x4004, 1, 5_000));
}

// ============================================================================
// AS_PATH loop prevention
// ============================================================================

/// A three-AS ring: R1 - R2 - R3 - R1. Every router originates its own LAN prefix, so
/// each prefix has two possible directions around the ring and the loop rules have to
/// stop it from circulating.
fn build_ring_lab() -> VirtualLab {
    let mut lab = VirtualLab::new();
    for link in ["r1r2", "r2r3", "r3r1"] {
        lab.add_link(link);
    }
    let m = |a: u8, b: u8| MacAddress([0x02, 0, 0, 0, a, b]);

    let mut r1 = LabRouter::new("r1");
    r1.add_interface("eth0", m(1, 0), ip(10, 12, 0, 1), 30, "r1r2");
    r1.add_interface("eth1", m(1, 1), ip(10, 31, 0, 1), 30, "r3r1");
    r1.enable_bgp(AS1, ip(1, 1, 1, 1))
        .set_hold_time(LAB_HOLD_TIME);
    r1.add_bgp_peer(ip(10, 12, 0, 2), AS2, ip(10, 12, 0, 1), BgpPeerMode::Active);
    r1.add_bgp_peer(
        ip(10, 31, 0, 3),
        AS3,
        ip(10, 31, 0, 1),
        BgpPeerMode::Passive,
    );
    r1.originate_bgp_prefix(prefix(172, 16, 1, 0, 24));

    let mut r2 = LabRouter::new("r2");
    r2.add_interface("eth0", m(2, 0), ip(10, 12, 0, 2), 30, "r1r2");
    r2.add_interface("eth1", m(2, 1), ip(10, 23, 0, 2), 30, "r2r3");
    r2.enable_bgp(AS2, ip(2, 2, 2, 2))
        .set_hold_time(LAB_HOLD_TIME);
    r2.add_bgp_peer(
        ip(10, 12, 0, 1),
        AS1,
        ip(10, 12, 0, 2),
        BgpPeerMode::Passive,
    );
    r2.add_bgp_peer(ip(10, 23, 0, 3), AS3, ip(10, 23, 0, 2), BgpPeerMode::Active);
    r2.originate_bgp_prefix(prefix(172, 16, 2, 0, 24));

    let mut r3 = LabRouter::new("r3");
    r3.add_interface("eth0", m(3, 0), ip(10, 23, 0, 3), 30, "r2r3");
    r3.add_interface("eth1", m(3, 1), ip(10, 31, 0, 3), 30, "r3r1");
    r3.enable_bgp(AS3, ip(3, 3, 3, 3))
        .set_hold_time(LAB_HOLD_TIME);
    r3.add_bgp_peer(
        ip(10, 23, 0, 2),
        AS2,
        ip(10, 23, 0, 3),
        BgpPeerMode::Passive,
    );
    r3.add_bgp_peer(ip(10, 31, 0, 1), AS1, ip(10, 31, 0, 3), BgpPeerMode::Active);
    r3.originate_bgp_prefix(prefix(172, 16, 3, 0, 24));

    lab.add_router(r1);
    lab.add_router(r2);
    lab.add_router(r3);
    lab
}

#[test]
fn test_a_route_never_loops_back_into_the_as_that_originated_it() {
    let mut lab = build_ring_lab();
    assert!(converge_sessions(&mut lab, 60_000));

    let own = prefix(172, 16, 1, 0, 24);
    assert!(
        run_until(&mut lab, 60_000, |l| {
            l.router("r2")
                .unwrap()
                .bgp()
                .unwrap()
                .loc_rib
                .contains(&prefix(172, 16, 3, 0, 24))
        }),
        "the ring never converged"
    );
    // Let the ring settle so any looping advertisement would have had time to arrive.
    for _ in 0..8 {
        lab.advance_time(1_000);
        lab.run_pumped(30);
    }

    // R1 originated 172.16.1.0/24. No matter how far it travelled round the ring, no
    // path back to R1 may be accepted, because every one carries AS65001.
    let r1 = lab.router("r1").unwrap().bgp().unwrap();
    assert!(
        r1.adj_rib_in
            .candidates(own)
            .iter()
            .all(|p| p.source == PathSource::Local),
        "R1 accepted its own prefix back from a neighbour"
    );
    assert!(r1.loc_rib.get(&own).unwrap().is_local());
    assert!(
        !r1.installed_prefixes().contains(&own),
        "R1 installed a looped route to its own prefix"
    );

    // The other prefixes converged on the shortest way round.
    assert_eq!(
        best_as_path(&lab, "r1", prefix(172, 16, 2, 0, 24)),
        Some(vec![AS2])
    );
    assert_eq!(
        best_as_path(&lab, "r1", prefix(172, 16, 3, 0, 24)),
        Some(vec![AS3])
    );

    // No AS_PATH anywhere in the ring grew beyond two hops, which is what would happen
    // if an advertisement were circulating.
    for r in ["r1", "r2", "r3"] {
        let bgp = lab.router(r).unwrap().bgp().unwrap();
        for path in bgp.adj_rib_in.iter_paths() {
            assert!(
                path.as_path.length() <= 2,
                "{} holds a {}-hop AS_PATH [{}] for {}, which suggests a loop",
                r,
                path.as_path.length(),
                path.as_path,
                path.prefix
            );
        }
    }
}

#[test]
fn test_an_update_carrying_our_own_asn_is_discarded() {
    // A neighbour that advertises a path already containing our ASN must be ignored,
    // and the session must survive: a loop is not a protocol violation.
    use toy_tcpip::socket::SocketError;
    use toy_tcpip::tcp::{SocketAddrV4, TcpState};

    let mut lab = VirtualLab::new();
    lab.add_link("wire");
    lab.add_host(
        "peer",
        "wire",
        NetStackConfig {
            mac: MacAddress([0x02, 0, 0, 0, 0xAA, 0x02]),
            ip: ip(10, 60, 0, 2),
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        },
    );
    let mut victim = LabRouter::new("victim");
    victim.add_interface(
        "eth0",
        MacAddress([0x02, 0, 0, 0, 0xBB, 0x02]),
        ip(10, 60, 0, 1),
        24,
        "wire",
    );
    victim
        .enable_bgp(AS1, ip(7, 7, 7, 7))
        .set_hold_time(LAB_HOLD_TIME);
    victim.add_bgp_peer(
        ip(10, 60, 0, 2),
        AS2,
        ip(10, 60, 0, 1),
        BgpPeerMode::Passive,
    );
    lab.add_router(victim);
    lab.run_pumped(5);

    let stream = lab
        .host_mut("peer")
        .unwrap()
        .stack
        .tcp_connect(SocketAddrV4 {
            ip: ip(10, 60, 0, 1),
            port: 179,
        })
        .unwrap();
    assert!(lab.run_until(
        250,
        30_000,
        |l| l.host("peer").unwrap().stack.tcp_state(stream) == Ok(TcpState::Established)
    ));

    let write = |lab: &mut VirtualLab, bytes: Vec<u8>| {
        let n = lab
            .host_mut("peer")
            .unwrap()
            .stack
            .tcp_write(stream, &bytes)
            .unwrap();
        assert_eq!(n, bytes.len());
        lab.run_pumped(30);
    };

    write(
        &mut lab,
        BgpPdu::Open(toy_tcpip::bgp::BgpOpenMessage::new(
            AS2,
            LAB_HOLD_TIME,
            ip(6, 6, 6, 6),
        ))
        .serialize(),
    );
    write(&mut lab, BgpPdu::Keepalive.serialize());
    assert!(lab.run_until(250, 30_000, |l| {
        l.router("victim")
            .unwrap()
            .bgp()
            .unwrap()
            .peer_state(ip(10, 60, 0, 2))
            == Some(BgpState::Established)
    }));

    // Two prefixes: one clean, one whose AS_PATH already contains AS65001.
    let clean = prefix(198, 51, 100, 0, 24);
    let looped = prefix(203, 0, 113, 0, 24);
    write(
        &mut lab,
        BgpPdu::Update(BgpUpdateMessage::announce(
            BgpPathAttributes::new(
                BgpOrigin::Igp,
                AsPath::sequence(vec![AS2, AS3]),
                ip(10, 60, 0, 2),
            ),
            vec![clean],
        ))
        .serialize(),
    );
    write(
        &mut lab,
        BgpPdu::Update(BgpUpdateMessage::announce(
            BgpPathAttributes::new(
                BgpOrigin::Igp,
                AsPath::sequence(vec![AS2, AS1, AS3]),
                ip(10, 60, 0, 2),
            ),
            vec![looped],
        ))
        .serialize(),
    );
    lab.run_pumped(40);

    let bgp = lab.router("victim").unwrap().bgp().unwrap();
    assert!(
        bgp.loc_rib.contains(&clean),
        "the clean prefix should have been accepted"
    );
    assert!(
        !bgp.loc_rib.contains(&looped),
        "a path containing our own ASN was accepted"
    );
    assert!(bgp.adj_rib_in.candidates(looped).is_empty());
    assert_eq!(
        bgp.peer(ip(10, 60, 0, 2))
            .unwrap()
            .counters
            .as_loops_rejected,
        1
    );
    // The session survives; a loop is discarded, not escalated.
    assert_eq!(
        bgp.peer_state(ip(10, 60, 0, 2)),
        Some(BgpState::Established)
    );

    // Silence the unused-variable warning from the final closure capture.
    let _ = SocketError::WouldBlock;
}

// ============================================================================
// Diagnostics
// ============================================================================

#[test]
fn test_diagnostics_report_real_session_and_rib_state() {
    let mut lab = build_linear_lab();
    converge_linear(&mut lab);
    let now = lab.current_time_ms;

    let bgp = lab.router("r2").unwrap().bgp().unwrap();

    let summary = bgp.format_summary(now);
    assert!(summary.contains("local AS number 65002"));
    assert!(summary.contains("10.12.0.1"));
    assert!(summary.contains("10.23.0.3"));
    assert!(summary.contains("Established"));

    let peers = bgp.format_peers(now);
    assert!(peers.contains("remote-as 65001"));
    assert!(peers.contains("remote-as 65003"));
    assert!(peers.contains("router-id 1.1.1.1"));

    let routes = bgp.format_routes();
    assert!(routes.contains("10.1.0.0/24"));
    assert!(routes.contains("10.3.0.0/24"));

    let rib = bgp.format_rib();
    assert!(rib.contains("10.3.0.0/24"));
    assert!(rib.contains("10.23.0.3"));

    // Uptime is measured in simulated time and only exists while the session is up.
    let s = bgp.summaries(now);
    assert!(s.iter().all(|p| p.uptime_ms.is_some()));
    assert!(s.iter().all(|p| p.hold_ms == LAB_HOLD_TIME as u64 * 1_000));
    assert!(s.iter().all(|p| p.prefixes_received == 1));
}

// ============================================================================
// Stability
// ============================================================================

/// Snapshot of every Loc-RIB in the lab: prefix and the AS_PATH of its best path.
fn rib_snapshot(lab: &VirtualLab) -> Vec<(String, String, Vec<u16>)> {
    let mut out = Vec::new();
    let mut names: Vec<&String> = lab.routers.keys().collect();
    names.sort();
    for name in names {
        let router = &lab.routers[name];
        if let Some(bgp) = router.bgp() {
            for p in bgp.loc_rib.prefixes() {
                let path = bgp.loc_rib.get(&p).unwrap();
                out.push((name.clone(), p.to_string(), path.as_path.flatten()));
            }
        }
    }
    out
}

fn total_updates_sent(lab: &VirtualLab) -> u64 {
    lab.routers
        .values()
        .filter_map(|r| r.bgp())
        .flat_map(|b| b.peers())
        .map(|p| p.counters.updates_sent)
        .sum()
}

fn total_decision_runs(lab: &VirtualLab) -> u64 {
    lab.routers
        .values()
        .filter_map(|r| r.bgp())
        .map(|b| b.decision_runs)
        .sum()
}

#[test]
fn test_a_converged_network_stops_sending_updates() {
    // Route oscillation and duplicate advertisement both look the same from outside: a
    // converged network that keeps talking. It must go quiet and stay quiet.
    let mut lab = build_linear_lab();
    converge_linear(&mut lab);

    // Settle first - convergence itself legitimately produces a few UPDATEs.
    for _ in 0..6 {
        lab.advance_time(1_000);
        lab.run_pumped(30);
    }
    let updates_after_settle = total_updates_sent(&lab);
    let decisions_after_settle = total_decision_runs(&lab);
    let before = rib_snapshot(&lab);

    // Now run for several hold times with nothing changing.
    for _ in 0..30 {
        lab.advance_time(1_000);
        lab.run_pumped(30);
    }

    assert_eq!(
        total_updates_sent(&lab),
        updates_after_settle,
        "a converged network kept re-advertising"
    );
    assert_eq!(
        total_decision_runs(&lab),
        decisions_after_settle,
        "the decision process kept rerunning with nothing to decide"
    );
    assert_eq!(rib_snapshot(&lab), before, "the RIBs moved on their own");

    // Sessions stayed up throughout, so the silence is quiet convergence and not a
    // network that fell over.
    for r in ["r1", "r2", "r3"] {
        assert!(
            lab.router(r)
                .unwrap()
                .bgp()
                .unwrap()
                .peers()
                .iter()
                .all(|p| p.state == BgpState::Established),
            "{} lost a session while idling",
            r
        );
    }
}

#[test]
fn test_repeated_session_flaps_do_not_leak_connections_or_routes() {
    let mut lab = build_linear_lab();
    converge_linear(&mut lab);

    for round in 0..4 {
        lab.link_mut("r1r2").unwrap().set_blackhole(true);
        assert!(
            run_until(&mut lab, 120_000, |l| {
                l.router("r1")
                    .unwrap()
                    .bgp()
                    .unwrap()
                    .peer_state(ip(10, 12, 0, 2))
                    != Some(BgpState::Established)
            }),
            "round {}: the session never went down",
            round
        );
        // Nothing learned from the dead peer may survive.
        assert_eq!(
            lab.router("r1")
                .unwrap()
                .bgp()
                .unwrap()
                .adj_rib_in
                .prefix_count(ip(10, 12, 0, 2)),
            0
        );
        assert!(!has_bgp_fib_route(&lab, "r1", P_C));

        lab.link_mut("r1r2").unwrap().set_blackhole(false);
        assert!(
            run_until(&mut lab, 180_000, |l| {
                l.router("r1")
                    .unwrap()
                    .bgp()
                    .unwrap()
                    .peer_state(ip(10, 12, 0, 2))
                    == Some(BgpState::Established)
                    && has_bgp_fib_route(l, "r1", P_C)
            }),
            "round {}: the session never came back",
            round
        );
    }

    // The session really did flap several times, and each recovery reinstalled the
    // route rather than leaving a duplicate behind.
    let bgp = lab.router("r1").unwrap().bgp().unwrap();
    assert!(
        bgp.peer(ip(10, 12, 0, 2)).unwrap().establishment_count >= 3,
        "expected several establishments, got {}",
        bgp.peer(ip(10, 12, 0, 2)).unwrap().establishment_count
    );
    assert_eq!(
        lab.router("r1")
            .unwrap()
            .routing_table
            .routes_from(RouteSource::Bgp)
            .len(),
        1
    );

    // Transport state does not accumulate: abandoned connections are released, and the
    // socket runtime keeps only what is still live.
    let sockets = lab.router("r1").unwrap().sockets.as_ref().unwrap();
    assert!(
        sockets.connection_count() <= 2,
        "{} live connections after four flaps",
        sockets.connection_count()
    );
    assert!(
        sockets.closed_stream_count() <= toy_tcpip::socket::MAX_CLOSED_STREAM_HISTORY,
        "closed-stream history grew past its cap"
    );
}

#[test]
fn test_every_route_in_the_converged_lab_is_connected_or_bgp() {
    // A guard against a test or a helper quietly injecting a route: after convergence
    // every entry on every router must be either a connected subnet or a BGP route.
    let mut lab = build_linear_lab();
    converge_linear(&mut lab);

    for name in ["r1", "r2", "r3"] {
        let router = lab.router(name).unwrap();
        let connected = router
            .routing_table
            .routes_from(RouteSource::Connected)
            .len();
        let bgp_routes = router.routing_table.routes_from(RouteSource::Bgp).len();
        assert_eq!(
            connected,
            router.interfaces.len(),
            "{} has an unexpected number of connected routes",
            name
        );
        assert_eq!(
            connected + bgp_routes,
            router.routing_table.len(),
            "{} holds a route from a source nothing in this phase installs: {:?}",
            name,
            router
                .routing_table
                .all_routes()
                .iter()
                .map(|r| r.to_string())
                .collect::<Vec<_>>()
        );

        // Every BGP route matches the speaker's own record of what it installed.
        let mut from_table: Vec<String> = router
            .routing_table
            .routes_from(RouteSource::Bgp)
            .iter()
            .map(|r| format!("{}/{}", r.destination, r.prefix_len))
            .collect();
        from_table.sort();
        let mut from_speaker: Vec<String> = router
            .bgp()
            .unwrap()
            .installed_prefixes()
            .iter()
            .map(|p| p.to_string())
            .collect();
        from_speaker.sort();
        assert_eq!(
            from_table, from_speaker,
            "{} FIB and Loc-RIB disagree",
            name
        );
    }
}

#[test]
fn test_a_peer_exceeding_its_prefix_limit_is_cut_off() {
    use common::bgp_lab::RawBgpPeer;

    let mut peer = RawBgpPeer::connect(AS1, AS2, ip(9, 9, 9, 9));
    peer.establish();
    peer.lab
        .router_mut("victim")
        .unwrap()
        .bgp_mut()
        .unwrap()
        .set_max_prefixes(peer.peer, 3);

    // Six distinct prefixes in one UPDATE: well past the limit of three.
    let nlri: Vec<_> = (0..6u8).map(|i| prefix(203, 0, 113, i * 8, 29)).collect();
    peer.write(
        &BgpPdu::Update(BgpUpdateMessage::announce(
            BgpPathAttributes::new(
                BgpOrigin::Igp,
                AsPath::sequence(vec![AS2]),
                ip(10, 50, 0, 2),
            ),
            nlri,
        ))
        .serialize(),
    );

    let note = peer
        .notification()
        .expect("no NOTIFICATION when the prefix limit was blown");
    assert_eq!(note.error_code, 6); // Cease
    assert_eq!(note.error_subcode, 1); // Maximum Number of Prefixes Reached
    assert_ne!(peer.state(), BgpState::Established);
    // Tearing the session down purged everything it had managed to install.
    assert_eq!(peer.victim_bgp().adj_rib_in.path_count(), 0);
    assert!(peer.victim_bgp().loc_rib.is_empty());
}

#[test]
fn test_an_identical_re_advertisement_does_not_rerun_the_decision_process() {
    use common::bgp_lab::RawBgpPeer;

    let mut peer = RawBgpPeer::connect(AS1, AS2, ip(9, 9, 9, 9));
    peer.establish();

    let announce = || {
        BgpPdu::Update(BgpUpdateMessage::announce(
            BgpPathAttributes::new(
                BgpOrigin::Igp,
                AsPath::sequence(vec![AS2]),
                ip(10, 50, 0, 2),
            ),
            vec![prefix(10, 99, 0, 0, 24)],
        ))
        .serialize()
    };

    peer.write(&announce());
    let after_first = peer.victim_bgp().decision_runs;
    assert_eq!(peer.victim_bgp().adj_rib_in.path_count(), 1);

    // Send the very same route again, at a different simulated time. A peer doing this
    // is ordinary - a refresh, a duplicate, a neighbour that repeats itself - and it
    // must cost nothing. The arrival timestamp is a diagnostic, so if it were allowed
    // into the comparison every duplicate would look like a change and the whole
    // decision process would run again for a table that did not move.
    peer.run_until(4_000, |_| false);
    peer.write(&announce());

    assert_eq!(
        peer.victim_bgp().decision_runs,
        after_first,
        "an identical re-advertisement reran the decision process"
    );
    assert_eq!(peer.victim_bgp().adj_rib_in.path_count(), 1);
    assert_eq!(peer.state(), BgpState::Established);

    // A real change still gets through: same prefix, different next hop.
    peer.write(
        &BgpPdu::Update(BgpUpdateMessage::announce(
            BgpPathAttributes::new(
                BgpOrigin::Igp,
                AsPath::sequence(vec![AS2]),
                ip(10, 50, 0, 3),
            ),
            vec![prefix(10, 99, 0, 0, 24)],
        ))
        .serialize(),
    );
    assert!(
        peer.victim_bgp().decision_runs > after_first,
        "a changed next hop was ignored"
    );
}

#[test]
fn test_a_flapping_origin_reconverges_every_time_and_settles_afterwards() {
    // Repeated withdrawal and re-announcement is where oscillation and stale state
    // show up: a FIB entry that never comes back, one that never leaves, or a network
    // that keeps chattering once the flapping stops.
    let mut lab = build_linear_lab();
    converge_linear(&mut lab);

    for round in 0..5 {
        lab.router_mut("r3")
            .unwrap()
            .bgp_mut()
            .unwrap()
            .withdraw_originated(P_C);
        assert!(
            run_until(&mut lab, 60_000, |l| !has_bgp_fib_route(l, "r1", P_C)),
            "round {}: the withdrawal never reached R1's FIB",
            round
        );

        lab.router_mut("r3").unwrap().originate_bgp_prefix(P_C);
        assert!(
            run_until(&mut lab, 60_000, |l| has_bgp_fib_route(l, "r1", P_C)),
            "round {}: the re-announcement never reached R1's FIB",
            round
        );
    }

    // The data plane still works after all that.
    assert!(ping(&mut lab, "host_a", ip(10, 3, 0, 2), 0x3003, 1, 30_000));

    // And the network goes quiet again rather than oscillating.
    for _ in 0..6 {
        lab.advance_time(1_000);
        lab.run_pumped(30);
    }
    let settled_updates = total_updates_sent(&lab);
    let settled_decisions = total_decision_runs(&lab);
    for _ in 0..20 {
        lab.advance_time(1_000);
        lab.run_pumped(30);
    }
    assert_eq!(total_updates_sent(&lab), settled_updates);
    assert_eq!(total_decision_runs(&lab), settled_decisions);
    assert!(has_bgp_fib_route(&lab, "r1", P_C));
}
