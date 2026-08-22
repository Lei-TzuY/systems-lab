//! MP-BGP EVPN over VXLAN, end to end on the leaf-spine-leaf fabric.
//!
//! The bar every test here has to clear: no test may write a remote MAC, a
//! remote VTEP, or a tunnel destination into either leaf. Everything the overlay
//! forwards on has to have arrived as an EVPN route over the real BGP session on
//! TCP port 179, which itself crosses the real IP underlay through the spine.
//!
//! The fabric is built by `build_evpn_fabric`, which configures each leaf with
//! nothing but its own VNI, RD, Route Targets and access port.

use toy_tcpip::bgp_caps::AfiSafi;
use toy_tcpip::ethernet::{ETHERTYPE_IPV4, EtherType, EthernetFrame, MacAddress};
use toy_tcpip::evpn::EvpnNlri;
use toy_tcpip::icmp::{IcmpPacket, IcmpType};
use toy_tcpip::ipv4::{IP_PROTO_UDP, Ipv4Address, Ipv4Packet};
use toy_tcpip::lab::{VirtualLab, build_evpn_fabric};
use toy_tcpip::udp::UdpDatagram;
use toy_tcpip::vxlan::{VXLAN_UDP_PORT, VxlanPacket};

const VNI: u32 = 5001;
const AS1: u32 = 65001;
const AS2: u32 = 65002;

fn ip(a: u8, b: u8, c: u8, d: u8) -> Ipv4Address {
    Ipv4Address::new(a, b, c, d)
}

const HOST_A: Ipv4Address = Ipv4Address([192, 168, 10, 11]);
const HOST_B: Ipv4Address = Ipv4Address([192, 168, 10, 22]);
const MAC_A: MacAddress = MacAddress([0x02, 0x00, 0x00, 0x00, 0x0A, 0x0A]);
const MAC_B: MacAddress = MacAddress([0x02, 0x00, 0x00, 0x00, 0x0B, 0x0B]);
const VTEP1: Ipv4Address = Ipv4Address([10, 0, 0, 1]);
const VTEP2: Ipv4Address = Ipv4Address([10, 0, 0, 2]);

/// Builds the fabric and gets both hosts talking, which is what makes each leaf
/// learn its own local MAC and advertise it.
fn converged_fabric() -> VirtualLab {
    let mut lab = build_evpn_fabric(AS1, AS2);
    // Let the BGP session come up over the underlay first.
    lab.run_until(250, 60_000, |l| {
        l.routers
            .values()
            .filter_map(|r| r.bgp())
            .all(|b| b.peers().iter().all(|p| p.carries_evpn()))
    });

    // Host A pings host B. The first thing that produces is an ARP broadcast,
    // which is the only way either leaf can learn anything at all.
    let frame = lab
        .host_mut("host_a")
        .unwrap()
        .stack
        .ping4(HOST_B, 0x1234, 1, b"evpn")
        .expect("host_a produced no frame");
    lab.send_from_host("host_a", frame);
    lab.run_until(250, 60_000, |_| false);
    lab
}

fn remote_mac(lab: &VirtualLab, leaf: &str, mac: MacAddress) -> Option<Ipv4Address> {
    lab.router(leaf)?.vtep()?.lookup_remote(VNI, &mac)
}

// ============================================================================
// Capability negotiation and session
// ============================================================================

#[test]
fn test_the_leaves_negotiate_evpn_over_a_multihop_session_through_the_spine() {
    let mut lab = build_evpn_fabric(AS1, AS2);
    assert!(
        lab.run_until(250, 60_000, |l| l
            .routers
            .values()
            .filter_map(|r| r.bgp())
            .all(|b| b.peers().iter().all(|p| p.carries_evpn()))),
        "the leaves never negotiated L2VPN EVPN"
    );

    for (leaf, remote_as) in [("leaf1", AS2), ("leaf2", AS1)] {
        let bgp = lab.router(leaf).unwrap().bgp().unwrap();
        let peer = &bgp.peers()[0];
        assert!(peer.negotiated.supports(AfiSafi::IPV4_UNICAST));
        assert!(peer.negotiated.supports(AfiSafi::L2VPN_EVPN));
        assert_eq!(peer.remote_as, remote_as);
        // The session is between the loopbacks, so every BGP segment was
        // forwarded by the spine rather than sent over a shared wire.
        assert!(peer.local_addr == VTEP1 || peer.local_addr == VTEP2);
    }

    // The spine carried it all without speaking BGP or knowing any VNI.
    let spine = lab.router("spine").unwrap();
    assert!(spine.bgp().is_none());
    assert!(spine.vtep().is_none());
}

// ============================================================================
// The acceptance chain
// ============================================================================

#[test]
fn test_a_remote_mac_learned_over_bgp_programs_vxlan_forwarding() {
    let lab = converged_fabric();

    // Leaf1 advertised host A, leaf2 advertised host B, and each imported the
    // other by Route Target. Nothing in this test wrote either entry.
    assert_eq!(
        remote_mac(&lab, "leaf1", MAC_B),
        Some(VTEP2),
        "leaf1 never learned host B through EVPN"
    );
    assert_eq!(
        remote_mac(&lab, "leaf2", MAC_A),
        Some(VTEP1),
        "leaf2 never learned host A through EVPN"
    );

    // A leaf must not learn its own local host as a remote one.
    assert_eq!(remote_mac(&lab, "leaf1", MAC_A), None);
    assert_eq!(remote_mac(&lab, "leaf2", MAC_B), None);

    // The remote entry carries the host IP the Type 2 route advertised.
    let inst = lab
        .router("leaf2")
        .unwrap()
        .vtep()
        .unwrap()
        .instance(VNI)
        .unwrap();
    let entry = inst.remote_macs.get(&MAC_A).unwrap();
    assert_eq!(entry.ip, Some(HOST_A));
    assert_eq!(entry.vtep, VTEP1);
}

#[test]
fn test_the_evpn_route_really_travelled_as_mp_reach_nlri() {
    let lab = converged_fabric();
    let bgp = lab.router("leaf2").unwrap().bgp().unwrap();

    let path = bgp
        .evpn_adj_rib_in
        .iter_paths()
        .find(|p| p.route.mac() == Some(MAC_A))
        .expect("no EVPN path for host A in leaf2's Adj-RIB-In");

    // It came from the peer, not from local configuration.
    assert!(!path.local);
    assert_eq!(path.peer_addr, VTEP1);
    assert_eq!(path.peer_as, AS1);
    assert_eq!(path.route.next_hop, VTEP1);
    assert_eq!(path.route.vni(), VNI);
    assert_eq!(path.route.host_ip(), Some(HOST_A));
    // eBGP prepended the originating AS.
    assert_eq!(path.as_path.flatten(), vec![AS1]);

    match &path.route.nlri {
        EvpnNlri::MacIpAdv(m) => {
            assert_eq!(m.mac, MAC_A);
            assert_eq!(m.rd.to_string(), "10.0.0.1:5001");
        }
        other => panic!("expected a Type 2 route, got {:?}", other),
    }

    // And a Type 3 route from the same leaf built the flood list.
    assert!(
        bgp.evpn_adj_rib_in
            .iter_paths()
            .any(|p| matches!(p.route.nlri, EvpnNlri::InclusiveMulticast(_))),
        "no Type 3 route arrived"
    );
    assert!(
        lab.router("leaf2")
            .unwrap()
            .vtep()
            .unwrap()
            .instance(VNI)
            .unwrap()
            .remote_vteps
            .contains(&VTEP1)
    );

    assert!(bgp.peers()[0].counters.evpn_received >= 2);
    assert!(bgp.peers()[0].counters.evpn_advertised >= 2);
}

#[test]
fn test_tenant_traffic_crosses_the_overlay_in_both_directions() {
    let mut lab = converged_fabric();

    // The ping in `converged_fabric` had to ARP first, so by now host A knows
    // host B's MAC. Send a fresh echo request and watch it complete.
    let frame = lab
        .host_mut("host_a")
        .unwrap()
        .stack
        .ping4(HOST_B, 0x4242, 7, b"overlay")
        .expect("host_a produced no frame");
    // A frame that still needed ARP would be an ARP request, not an IP packet.
    let eth = EthernetFrame::parse(&frame).unwrap();
    assert_eq!(
        eth.ethertype,
        EtherType::IPv4,
        "host A still had to ARP: the first exchange never completed"
    );
    assert_eq!(eth.dst_mac, MAC_B);

    lab.send_from_host("host_a", frame);
    lab.run_until(250, 60_000, |_| false);

    // Host B answered, and the answer got back.
    let replies = &lab.host("host_a").unwrap().stack.received_icmp_replies;
    assert!(
        replies
            .iter()
            .any(|(src, id, seq)| *src == HOST_B && *id == 0x4242 && *seq == 7),
        "no echo reply came back across the overlay; got {:?}",
        replies
    );
}

#[test]
fn test_the_underlay_carried_real_vxlan_on_udp_4789() {
    let mut lab = build_evpn_fabric(AS1, AS2);
    lab.enable_pcap("leaf1spine");
    lab.run_until(250, 60_000, |l| {
        l.routers
            .values()
            .filter_map(|r| r.bgp())
            .all(|b| b.peers().iter().all(|p| p.carries_evpn()))
    });
    let frame = lab
        .host_mut("host_a")
        .unwrap()
        .stack
        .ping4(HOST_B, 0x1234, 1, b"evpn")
        .unwrap();
    lab.send_from_host("host_a", frame);
    lab.run_until(250, 60_000, |_| false);

    let pcap = lab.export_pcap("leaf1spine").expect("no capture");
    let vxlan = captured_vxlan(&pcap);
    assert!(
        !vxlan.is_empty(),
        "nothing on the underlay was a VXLAN packet"
    );

    // Every VXLAN packet went between the two loopbacks and carried the tenant VNI.
    for (src, dst, vni, inner) in &vxlan {
        assert!(
            (*src == VTEP1 && *dst == VTEP2) || (*src == VTEP2 && *dst == VTEP1),
            "VXLAN between unexpected endpoints {} -> {}",
            src,
            dst
        );
        assert_eq!(*vni, VNI);
        let eth = EthernetFrame::parse(inner).expect("inner frame is not Ethernet");
        assert!(
            eth.src_mac == MAC_A || eth.src_mac == MAC_B,
            "inner frame from an unexpected MAC {}",
            eth.src_mac
        );
    }

    // At least one carried the tenant ICMP echo request between the tenant IPs.
    let carried_echo = vxlan.iter().any(|(_, _, _, inner)| {
        let Ok(eth) = EthernetFrame::parse(inner) else {
            return false;
        };
        let Ok(pkt) = Ipv4Packet::parse(eth.payload, false) else {
            return false;
        };
        pkt.header.src_ip == HOST_A
            && pkt.header.dst_ip == HOST_B
            && IcmpPacket::parse(pkt.payload, false)
                .is_ok_and(|i| i.icmp_type == IcmpType::EchoRequest)
    });
    assert!(
        carried_echo,
        "no VXLAN packet carried the tenant echo request"
    );
}

// ============================================================================
// Unknown unicast and Type 3
// ============================================================================

#[test]
fn test_a_known_unicast_is_sent_to_one_vtep_rather_than_flooded() {
    use toy_tcpip::evpn_vtep::OverlayDecision;

    let lab = converged_fabric();
    let vtep = lab.router("leaf1").unwrap().vtep().unwrap();

    assert_eq!(
        vtep.forward("eth0", MAC_B),
        OverlayDecision::Unicast {
            vni: VNI,
            vtep: VTEP2
        },
        "a MAC with a Type 2 route was flooded instead of sent to its VTEP"
    );

    // A MAC nothing advertised is flooded to the Type 3 list, which is the only
    // thing that should ever produce replication.
    let unknown = MacAddress([0x02, 0x00, 0x00, 0x00, 0xEE, 0xEE]);
    assert_eq!(
        vtep.forward("eth0", unknown),
        OverlayDecision::Flood {
            vni: VNI,
            vteps: vec![VTEP2]
        }
    );
    assert_eq!(
        vtep.forward("eth0", MacAddress::BROADCAST),
        OverlayDecision::Flood {
            vni: VNI,
            vteps: vec![VTEP2]
        }
    );
}

#[test]
fn test_a_leaf_with_no_type_3_route_has_nowhere_to_flood() {
    // Before any EVPN route arrives, a broadcast has no ingress-replication list
    // and must be dropped rather than sent somewhere invented.
    use toy_tcpip::evpn_vtep::OverlayDecision;

    let lab = build_evpn_fabric(AS1, AS2);
    let vtep = lab.router("leaf1").unwrap().vtep().unwrap();
    assert_eq!(
        vtep.forward("eth0", MacAddress::BROADCAST),
        OverlayDecision::Drop
    );
}

// ============================================================================
// Helpers
// ============================================================================

/// Pulls `(outer src, outer dst, VNI, inner frame)` out of every VXLAN packet in
/// a capture, using this repository's own PCAP reader.
fn captured_vxlan(pcap: &[u8]) -> Vec<(Ipv4Address, Ipv4Address, u32, Vec<u8>)> {
    use toy_tcpip::pcap::PcapReader;

    let mut reader = PcapReader::new(std::io::Cursor::new(pcap)).expect("PcapReader");
    let mut out = Vec::new();
    while let Ok(Some(pkt)) = reader.next_packet() {
        let Ok(eth) = EthernetFrame::parse(&pkt.data) else {
            continue;
        };
        if eth.ethertype != EtherType::IPv4 {
            continue;
        }
        let Ok(ipv4) = Ipv4Packet::parse(eth.payload, false) else {
            continue;
        };
        if ipv4.header.protocol != toy_tcpip::ipv4::IpProtocol::Udp {
            continue;
        }
        let Ok(udp) =
            UdpDatagram::parse(ipv4.header.src_ip, ipv4.header.dst_ip, ipv4.payload, false)
        else {
            continue;
        };
        if udp.dst_port != VXLAN_UDP_PORT {
            continue;
        }
        let Ok(vx) = VxlanPacket::parse(udp.payload) else {
            continue;
        };
        out.push((
            ipv4.header.src_ip,
            ipv4.header.dst_ip,
            vx.header.vni,
            vx.inner_frame,
        ));
    }
    out
}

#[test]
fn test_the_helpers_agree_with_the_constants() {
    // A guard on the fabric builder: if the addressing ever changes, these
    // tests should fail here rather than mysteriously somewhere else.
    let lab = build_evpn_fabric(AS1, AS2);
    assert_eq!(
        lab.router("leaf1").unwrap().vtep().unwrap().source_ip,
        VTEP1
    );
    assert_eq!(
        lab.router("leaf2").unwrap().vtep().unwrap().source_ip,
        VTEP2
    );
    assert_eq!(lab.host("host_a").unwrap().stack.config.ip, HOST_A);
    assert_eq!(lab.host("host_b").unwrap().stack.config.mac, MAC_B);
    assert_eq!(ip(10, 0, 0, 1), VTEP1);
    let _ = (ETHERTYPE_IPV4, IP_PROTO_UDP);
}
