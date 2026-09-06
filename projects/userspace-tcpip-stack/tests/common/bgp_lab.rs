//! Multi-autonomous-system BGP topologies for the control-plane integration tests.
//!
//! Everything here builds a real lab: routers with real interfaces, a real socket
//! runtime, and a real BGP speaker listening on TCP port 179. No helper injects a
//! route, hands a BGP message to a peer object, or touches a `RoutingTable` on a
//! router's behalf. The only clock is the lab's simulated one.

#![allow(dead_code)]

use toy_tcpip::bgp::Ipv4Prefix;
use toy_tcpip::bgp_router::{BgpPeerMode, BgpState};
use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::icmp::IcmpPacket;
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::lab::{LabRouter, VirtualLab};
use toy_tcpip::router::RouteSource;
use toy_tcpip::stack::NetStackConfig;

pub const AS1: u32 = 65001;
pub const AS2: u32 = 65002;
pub const AS3: u32 = 65003;
pub const AS4: u32 = 65004;

/// Hold time used across the labs, in seconds. Short enough that a hold-timer
/// expiry is a handful of simulated ticks, long enough to be a legal BGP value.
pub const LAB_HOLD_TIME: u16 = 9;

pub fn prefix(a: u8, b: u8, c: u8, d: u8, len: u8) -> Ipv4Prefix {
    Ipv4Prefix::new(Ipv4Address::new(a, b, c, d), len)
}

pub fn ip(a: u8, b: u8, c: u8, d: u8) -> Ipv4Address {
    Ipv4Address::new(a, b, c, d)
}

fn mac(n: u8, k: u8) -> MacAddress {
    MacAddress([0x02, 0x00, 0x00, 0x00, n, k])
}

fn host_config(m: MacAddress, addr: Ipv4Address, gw: Ipv4Address) -> NetStackConfig {
    NetStackConfig {
        mac: m,
        ip: addr,
        ipv6: None,
        subnet_mask: 24,
        gateway: Some(gw),
    }
}

/// The linear lab:
///
/// ```text
/// host_a 10.1.0.2 -- [lan1] -- r1 (AS65001) -- [r1r2] -- r2 (AS65002)
///                                                          |
///                                                        [r2r3]
///                                                          |
/// host_c 10.3.0.2 -- [lan3] -- r3 (AS65003) ---------------+
/// ```
///
/// R1 originates 10.1.0.0/24 and R3 originates 10.3.0.0/24. Nothing else is
/// configured: every route either router ends up with must have arrived over BGP.
pub fn build_linear_lab() -> VirtualLab {
    let mut lab = VirtualLab::new();
    for link in ["lan1", "r1r2", "r2r3", "lan3"] {
        lab.add_link(link);
    }

    lab.add_host(
        "host_a",
        "lan1",
        host_config(mac(0x0A, 0x02), ip(10, 1, 0, 2), ip(10, 1, 0, 1)),
    );
    lab.add_host(
        "host_c",
        "lan3",
        host_config(mac(0x0C, 0x02), ip(10, 3, 0, 2), ip(10, 3, 0, 1)),
    );

    let mut r1 = LabRouter::new("r1");
    r1.add_interface("eth0", mac(0x01, 0x00), ip(10, 1, 0, 1), 24, "lan1");
    r1.add_interface("eth1", mac(0x01, 0x01), ip(10, 12, 0, 1), 30, "r1r2");
    r1.enable_bgp(AS1, ip(1, 1, 1, 1))
        .set_hold_time(LAB_HOLD_TIME);
    r1.add_bgp_peer(ip(10, 12, 0, 2), AS2, ip(10, 12, 0, 1), BgpPeerMode::Active);
    r1.originate_bgp_prefix(prefix(10, 1, 0, 0, 24));

    let mut r2 = LabRouter::new("r2");
    r2.add_interface("eth0", mac(0x02, 0x00), ip(10, 12, 0, 2), 30, "r1r2");
    r2.add_interface("eth1", mac(0x02, 0x01), ip(10, 23, 0, 2), 30, "r2r3");
    r2.enable_bgp(AS2, ip(2, 2, 2, 2))
        .set_hold_time(LAB_HOLD_TIME);
    r2.add_bgp_peer(
        ip(10, 12, 0, 1),
        AS1,
        ip(10, 12, 0, 2),
        BgpPeerMode::Passive,
    );
    r2.add_bgp_peer(ip(10, 23, 0, 3), AS3, ip(10, 23, 0, 2), BgpPeerMode::Active);

    let mut r3 = LabRouter::new("r3");
    r3.add_interface("eth0", mac(0x03, 0x00), ip(10, 23, 0, 3), 30, "r2r3");
    r3.add_interface("eth1", mac(0x03, 0x01), ip(10, 3, 0, 1), 24, "lan3");
    r3.enable_bgp(AS3, ip(3, 3, 3, 3))
        .set_hold_time(LAB_HOLD_TIME);
    r3.add_bgp_peer(
        ip(10, 23, 0, 2),
        AS2,
        ip(10, 23, 0, 3),
        BgpPeerMode::Passive,
    );
    r3.originate_bgp_prefix(prefix(10, 3, 0, 0, 24));

    lab.add_router(r1);
    lab.add_router(r2);
    lab.add_router(r3);
    lab
}

/// The diamond lab, used for best-path selection and failover:
///
/// ```text
///                    r2 (AS65002)
///                  /              \
/// host_a -- r1 (AS65001)           r4 (AS65004) -- host_d 10.4.0.2
///                  \              /
///                    r3 (AS65003)
/// ```
///
/// R4 originates 10.4.0.0/24 and R1 originates 10.1.0.0/24, so R1 sees two equal-length
/// paths to 10.4.0.0/24 and R4 sees two to 10.1.0.0/24.
pub fn build_diamond_lab() -> VirtualLab {
    let mut lab = VirtualLab::new();
    for link in ["lan1", "l12", "l13", "l24", "l34", "lan4"] {
        lab.add_link(link);
    }

    lab.add_host(
        "host_a",
        "lan1",
        host_config(mac(0x0A, 0x02), ip(10, 1, 0, 2), ip(10, 1, 0, 1)),
    );
    lab.add_host(
        "host_d",
        "lan4",
        host_config(mac(0x0D, 0x02), ip(10, 4, 0, 2), ip(10, 4, 0, 1)),
    );

    let mut r1 = LabRouter::new("r1");
    r1.add_interface("eth0", mac(0x01, 0x00), ip(10, 1, 0, 1), 24, "lan1");
    r1.add_interface("eth1", mac(0x01, 0x01), ip(10, 12, 0, 1), 24, "l12");
    r1.add_interface("eth2", mac(0x01, 0x02), ip(10, 13, 0, 1), 24, "l13");
    r1.enable_bgp(AS1, ip(1, 1, 1, 1))
        .set_hold_time(LAB_HOLD_TIME);
    r1.add_bgp_peer(ip(10, 12, 0, 2), AS2, ip(10, 12, 0, 1), BgpPeerMode::Active);
    r1.add_bgp_peer(ip(10, 13, 0, 3), AS3, ip(10, 13, 0, 1), BgpPeerMode::Active);
    r1.originate_bgp_prefix(prefix(10, 1, 0, 0, 24));

    let mut r2 = LabRouter::new("r2");
    r2.add_interface("eth0", mac(0x02, 0x00), ip(10, 12, 0, 2), 24, "l12");
    r2.add_interface("eth1", mac(0x02, 0x01), ip(10, 24, 0, 2), 24, "l24");
    r2.enable_bgp(AS2, ip(2, 2, 2, 2))
        .set_hold_time(LAB_HOLD_TIME);
    r2.add_bgp_peer(
        ip(10, 12, 0, 1),
        AS1,
        ip(10, 12, 0, 2),
        BgpPeerMode::Passive,
    );
    r2.add_bgp_peer(ip(10, 24, 0, 4), AS4, ip(10, 24, 0, 2), BgpPeerMode::Active);

    let mut r3 = LabRouter::new("r3");
    r3.add_interface("eth0", mac(0x03, 0x00), ip(10, 13, 0, 3), 24, "l13");
    r3.add_interface("eth1", mac(0x03, 0x01), ip(10, 34, 0, 3), 24, "l34");
    r3.enable_bgp(AS3, ip(3, 3, 3, 3))
        .set_hold_time(LAB_HOLD_TIME);
    r3.add_bgp_peer(
        ip(10, 13, 0, 1),
        AS1,
        ip(10, 13, 0, 3),
        BgpPeerMode::Passive,
    );
    r3.add_bgp_peer(ip(10, 34, 0, 4), AS4, ip(10, 34, 0, 3), BgpPeerMode::Active);

    let mut r4 = LabRouter::new("r4");
    r4.add_interface("eth0", mac(0x04, 0x00), ip(10, 24, 0, 4), 24, "l24");
    r4.add_interface("eth1", mac(0x04, 0x01), ip(10, 34, 0, 4), 24, "l34");
    r4.add_interface("eth2", mac(0x04, 0x02), ip(10, 4, 0, 1), 24, "lan4");
    r4.enable_bgp(AS4, ip(4, 4, 4, 4))
        .set_hold_time(LAB_HOLD_TIME);
    r4.add_bgp_peer(
        ip(10, 24, 0, 2),
        AS2,
        ip(10, 24, 0, 4),
        BgpPeerMode::Passive,
    );
    r4.add_bgp_peer(
        ip(10, 34, 0, 3),
        AS3,
        ip(10, 34, 0, 4),
        BgpPeerMode::Passive,
    );
    r4.originate_bgp_prefix(prefix(10, 4, 0, 0, 24));

    lab.add_router(r1);
    lab.add_router(r2);
    lab.add_router(r3);
    lab.add_router(r4);
    lab
}

/// The iBGP lab, used to check the rules that only apply inside one autonomous system:
///
/// ```text
/// r1 (AS65001) --eBGP-- r2 (AS65002) --iBGP-- r3 (AS65002) --eBGP-- r4 (AS65004)
///                          |
///                        iBGP
///                          |
///                       r5 (AS65002)
/// ```
///
/// r4 originates 10.4.0.0/24 and r1 originates 10.1.0.0/24. Both iBGP sessions use
/// next-hop-self, which is what makes the next hop resolvable without an IGP.
pub fn build_ibgp_lab() -> VirtualLab {
    let mut lab = VirtualLab::new();
    for link in ["l12", "l23", "l25", "l34"] {
        lab.add_link(link);
    }

    let mut r1 = LabRouter::new("r1");
    r1.add_interface("eth0", mac(0x01, 0x00), ip(10, 12, 0, 1), 24, "l12");
    r1.enable_bgp(AS1, ip(1, 1, 1, 1))
        .set_hold_time(LAB_HOLD_TIME);
    r1.add_bgp_peer(ip(10, 12, 0, 2), AS2, ip(10, 12, 0, 1), BgpPeerMode::Active);
    r1.originate_bgp_prefix(prefix(10, 1, 0, 0, 24));

    let mut r2 = LabRouter::new("r2");
    r2.add_interface("eth0", mac(0x02, 0x00), ip(10, 12, 0, 2), 24, "l12");
    r2.add_interface("eth1", mac(0x02, 0x01), ip(10, 23, 0, 2), 24, "l23");
    r2.add_interface("eth2", mac(0x02, 0x02), ip(10, 25, 0, 2), 24, "l25");
    r2.enable_bgp(AS2, ip(2, 2, 2, 2))
        .set_hold_time(LAB_HOLD_TIME);
    r2.add_bgp_peer(
        ip(10, 12, 0, 1),
        AS1,
        ip(10, 12, 0, 2),
        BgpPeerMode::Passive,
    );
    r2.add_bgp_peer(ip(10, 23, 0, 3), AS2, ip(10, 23, 0, 2), BgpPeerMode::Active);
    r2.add_bgp_peer(ip(10, 25, 0, 5), AS2, ip(10, 25, 0, 2), BgpPeerMode::Active);
    if let Some(b) = r2.bgp_mut() {
        b.set_next_hop_self(ip(10, 23, 0, 3), true);
        b.set_next_hop_self(ip(10, 25, 0, 5), true);
    }

    let mut r3 = LabRouter::new("r3");
    r3.add_interface("eth0", mac(0x03, 0x00), ip(10, 23, 0, 3), 24, "l23");
    r3.add_interface("eth1", mac(0x03, 0x01), ip(10, 34, 0, 3), 24, "l34");
    r3.enable_bgp(AS2, ip(3, 3, 3, 3))
        .set_hold_time(LAB_HOLD_TIME);
    r3.add_bgp_peer(
        ip(10, 23, 0, 2),
        AS2,
        ip(10, 23, 0, 3),
        BgpPeerMode::Passive,
    );
    r3.add_bgp_peer(ip(10, 34, 0, 4), AS4, ip(10, 34, 0, 3), BgpPeerMode::Active);
    if let Some(b) = r3.bgp_mut() {
        b.set_next_hop_self(ip(10, 23, 0, 2), true);
    }

    let mut r4 = LabRouter::new("r4");
    r4.add_interface("eth0", mac(0x04, 0x00), ip(10, 34, 0, 4), 24, "l34");
    r4.enable_bgp(AS4, ip(4, 4, 4, 4))
        .set_hold_time(LAB_HOLD_TIME);
    r4.add_bgp_peer(
        ip(10, 34, 0, 3),
        AS2,
        ip(10, 34, 0, 4),
        BgpPeerMode::Passive,
    );
    r4.originate_bgp_prefix(prefix(10, 4, 0, 0, 24));

    let mut r5 = LabRouter::new("r5");
    r5.add_interface("eth0", mac(0x05, 0x00), ip(10, 25, 0, 5), 24, "l25");
    r5.enable_bgp(AS2, ip(5, 5, 5, 5))
        .set_hold_time(LAB_HOLD_TIME);
    r5.add_bgp_peer(
        ip(10, 25, 0, 2),
        AS2,
        ip(10, 25, 0, 5),
        BgpPeerMode::Passive,
    );
    if let Some(b) = r5.bgp_mut() {
        b.set_next_hop_self(ip(10, 25, 0, 2), true);
    }

    lab.add_router(r1);
    lab.add_router(r2);
    lab.add_router(r3);
    lab.add_router(r5);
    lab.add_router(r4);
    lab
}

/// The dual-homed lab, used for MULTI_EXIT_DISC:
///
/// ```text
/// r1 (AS65001) === two separate links === AS65002, entered at r2 and at r3
/// ```
///
/// Both r2 and r3 are in AS65002 and both originate 10.9.0.0/24, so the two paths r1
/// sees are identical in every attribute except the MED each one attaches. Because both
/// paths start with the same neighbouring AS, comparing their MEDs is meaningful.
pub fn build_med_lab(med_via_r2: u32, med_via_r3: u32) -> VirtualLab {
    use toy_tcpip::bgp_rib::{PolicyRule, PrefixMatch, RoutePolicy};

    let mut lab = VirtualLab::new();
    for link in ["l12", "l13"] {
        lab.add_link(link);
    }
    let target = prefix(10, 9, 0, 0, 24);

    let mut r1 = LabRouter::new("r1");
    r1.add_interface("eth0", mac(0x01, 0x00), ip(10, 12, 0, 1), 24, "l12");
    r1.add_interface("eth1", mac(0x01, 0x01), ip(10, 13, 0, 1), 24, "l13");
    r1.enable_bgp(AS1, ip(1, 1, 1, 1))
        .set_hold_time(LAB_HOLD_TIME);
    r1.add_bgp_peer(ip(10, 12, 0, 2), AS2, ip(10, 12, 0, 1), BgpPeerMode::Active);
    r1.add_bgp_peer(ip(10, 13, 0, 3), AS2, ip(10, 13, 0, 1), BgpPeerMode::Active);

    let mut r2 = LabRouter::new("r2");
    r2.add_interface("eth0", mac(0x02, 0x00), ip(10, 12, 0, 2), 24, "l12");
    r2.enable_bgp(AS2, ip(2, 2, 2, 2))
        .set_hold_time(LAB_HOLD_TIME);
    r2.add_bgp_peer(
        ip(10, 12, 0, 1),
        AS1,
        ip(10, 12, 0, 2),
        BgpPeerMode::Passive,
    );
    r2.originate_bgp_prefix(target);
    let mut p2 = RoutePolicy::new();
    p2.add_rule(PolicyRule::permit(10, PrefixMatch::Any).with_med(med_via_r2));
    if let Some(b) = r2.bgp_mut() {
        b.set_export_policy(ip(10, 12, 0, 1), p2);
    }

    let mut r3 = LabRouter::new("r3");
    r3.add_interface("eth0", mac(0x03, 0x00), ip(10, 13, 0, 3), 24, "l13");
    r3.enable_bgp(AS2, ip(3, 3, 3, 3))
        .set_hold_time(LAB_HOLD_TIME);
    r3.add_bgp_peer(
        ip(10, 13, 0, 1),
        AS1,
        ip(10, 13, 0, 3),
        BgpPeerMode::Passive,
    );
    r3.originate_bgp_prefix(target);
    let mut p3 = RoutePolicy::new();
    p3.add_rule(PolicyRule::permit(10, PrefixMatch::Any).with_med(med_via_r3));
    if let Some(b) = r3.bgp_mut() {
        b.set_export_policy(ip(10, 13, 0, 1), p3);
    }

    lab.add_router(r1);
    lab.add_router(r2);
    lab.add_router(r3);
    lab
}

/// Drives the lab until every configured session on every router is ESTABLISHED.
pub fn converge_sessions(lab: &mut VirtualLab, max_sim_ms: u64) -> bool {
    lab.run_until(250, max_sim_ms, |l| {
        l.routers.values().all(|r| match r.bgp() {
            Some(b) => b.peers().iter().all(|p| p.state == BgpState::Established),
            None => true,
        })
    })
}

/// Drives the lab until `predicate` holds, with the tick size the BGP labs use.
pub fn run_until<F>(lab: &mut VirtualLab, max_sim_ms: u64, predicate: F) -> bool
where
    F: FnMut(&VirtualLab) -> bool,
{
    lab.run_until(250, max_sim_ms, predicate)
}

/// True when `router` has a BGP-sourced FIB entry for `p`.
pub fn has_bgp_fib_route(lab: &VirtualLab, router: &str, p: Ipv4Prefix) -> bool {
    lab.router(router)
        .and_then(|r| r.routing_table.find_exact(p.address, p.length))
        .is_some_and(|r| r.source == RouteSource::Bgp)
}

/// The forwarding next hop `router` would use for `p`, if it is a BGP route.
pub fn bgp_fib_next_hop(lab: &VirtualLab, router: &str, p: Ipv4Prefix) -> Option<Ipv4Address> {
    lab.router(router)
        .and_then(|r| r.routing_table.find_exact(p.address, p.length))
        .filter(|r| r.source == RouteSource::Bgp)
        .and_then(|r| r.gateway)
}

/// AS_PATH of `router`'s best path to `p`, flattened left to right.
pub fn best_as_path(lab: &VirtualLab, router: &str, p: Ipv4Prefix) -> Option<Vec<u32>> {
    lab.router(router)
        .and_then(|r| r.bgp())
        .and_then(|b| b.loc_rib.get(&p))
        .map(|path| path.as_path.flatten())
}

/// Sends one ICMP echo from `from_host` to `to_ip` and runs the lab until the reply
/// arrives (or the deadline passes). Returns true if the reply came back.
///
/// This is the data-plane probe: it proves packets really traverse the routers using
/// whatever the routing tables currently say.
pub fn ping(
    lab: &mut VirtualLab,
    from_host: &str,
    to_ip: Ipv4Address,
    id: u16,
    seq: u16,
    max_sim_ms: u64,
) -> bool {
    let before = lab
        .host(from_host)
        .map(|h| h.stack.received_icmp_replies.len())
        .unwrap_or(0);

    let Some(frame) = lab
        .host_mut(from_host)
        .and_then(|h| h.stack.ping4(to_ip, id, seq, b"BGP_DATA_PLANE_PROBE"))
    else {
        return false;
    };
    lab.send_from_host(from_host, frame);

    lab.run_until(250, max_sim_ms, |l| {
        l.host(from_host)
            .map(|h| {
                h.stack.received_icmp_replies.len() > before
                    && h.stack
                        .received_icmp_replies
                        .iter()
                        .any(|(src, i, s)| *src == to_ip && *i == id && *s == seq)
            })
            .unwrap_or(false)
    })
}

/// Extracts every ICMP echo request seen on `link`'s capture, as `(src, dst, seq)`.
/// Used to prove which physical path the data plane actually took.
pub fn captured_echo_requests(pcap: &[u8]) -> Vec<(Ipv4Address, Ipv4Address, u16)> {
    use toy_tcpip::ethernet::{EtherType, EthernetFrame};
    use toy_tcpip::ipv4::{IpProtocol, Ipv4Packet};
    use toy_tcpip::pcap::PcapReader;

    let mut out = Vec::new();
    let Ok(mut reader) = PcapReader::new(std::io::Cursor::new(pcap.to_vec())) else {
        return out;
    };
    let Ok(packets) = reader.read_all_packets() else {
        return out;
    };
    for pkt in packets {
        let Ok(eth) = EthernetFrame::parse(&pkt.data) else {
            continue;
        };
        if eth.ethertype != EtherType::IPv4 {
            continue;
        }
        let Ok(ipv4) = Ipv4Packet::parse(eth.payload, false) else {
            continue;
        };
        if ipv4.header.protocol != IpProtocol::Icmp {
            continue;
        }
        if let Ok(icmp) = IcmpPacket::parse(ipv4.payload, false)
            && icmp.icmp_type == toy_tcpip::icmp::IcmpType::EchoRequest
        {
            out.push((ipv4.header.src_ip, ipv4.header.dst_ip, icmp.sequence_number));
        }
    }
    out
}

// ============================================================================
// Raw peer harness
// ============================================================================

/// A plain TCP client wired to a router's port 179, used to drive the receive path
/// with bytes the test chooses - including bytes no correct speaker would ever send.
///
/// The router under test ("victim") is a real `LabRouter` running a real BGP speaker;
/// the only thing unusual is that the other end is a host writing raw bytes instead of
/// a second speaker.
pub struct RawBgpPeer {
    pub lab: VirtualLab,
    pub stream: toy_tcpip::socket::TcpStreamHandle,
    /// Address of the router under test.
    pub victim: Ipv4Address,
    /// Address this harness speaks from.
    pub peer: Ipv4Address,
    /// ASN the router expects this harness to claim.
    pub peer_as: u32,
    /// Whether the OPEN this harness sent advertised 4-octet ASN support. The
    /// router encodes AS_PATH accordingly, so the harness has to decode the same
    /// way or it would read the router's own UPDATEs wrong.
    pub four_octet_as: bool,
}

impl RawBgpPeer {
    /// Builds the lab and completes the TCP three-way handshake to port 179.
    /// The router is passive, so it waits for this connection.
    pub fn connect(victim_as: u32, expected_peer_as: u32, victim_router_id: Ipv4Address) -> Self {
        Self::connect_configured(victim_as, expected_peer_as, victim_router_id, |_| {})
    }

    /// [`RawBgpPeer::connect`], with a chance to configure the router under test
    /// before it is put in the lab - a VTEP and its EVPN instances, say.
    pub fn connect_configured(
        victim_as: u32,
        expected_peer_as: u32,
        victim_router_id: Ipv4Address,
        configure: impl FnOnce(&mut LabRouter),
    ) -> Self {
        use toy_tcpip::tcp::{SocketAddrV4, TcpState};

        let victim = ip(10, 50, 0, 1);
        let peer = ip(10, 50, 0, 2);

        let mut lab = VirtualLab::new();
        lab.add_link("wire");
        lab.add_host(
            "peer",
            "wire",
            NetStackConfig {
                mac: MacAddress([0x02, 0, 0, 0, 0xAA, 0x01]),
                ip: peer,
                ipv6: None,
                subnet_mask: 24,
                gateway: None,
            },
        );

        let mut router = LabRouter::new("victim");
        router.add_interface(
            "eth0",
            MacAddress([0x02, 0, 0, 0, 0xBB, 0x01]),
            victim,
            24,
            "wire",
        );
        router
            .enable_bgp(victim_as, victim_router_id)
            .set_hold_time(LAB_HOLD_TIME);
        router.add_bgp_peer(peer, expected_peer_as, victim, BgpPeerMode::Passive);
        configure(&mut router);
        lab.add_router(router);

        // Let the router bind its listener before dialling it.
        lab.run_pumped(5);

        let stream = lab
            .host_mut("peer")
            .unwrap()
            .stack
            .tcp_connect(SocketAddrV4 {
                ip: victim,
                port: 179,
            })
            .expect("tcp_connect to port 179");

        let mut harness = RawBgpPeer {
            lab,
            stream,
            victim,
            peer,
            peer_as: expected_peer_as,
            four_octet_as: false,
        };
        let s = harness.stream;
        assert!(
            harness.run_until(30_000, |l| l.host("peer").unwrap().stack.tcp_state(s)
                == Ok(TcpState::Established)),
            "the raw TCP connection to port 179 never established"
        );
        harness
    }

    pub fn run_until<F>(&mut self, max_sim_ms: u64, predicate: F) -> bool
    where
        F: FnMut(&VirtualLab) -> bool,
    {
        self.lab.run_until(250, max_sim_ms, predicate)
    }

    /// Runs the lab far enough for anything just written to be delivered and processed.
    pub fn pump(&mut self) {
        self.lab.run_pumped(30);
    }

    /// Writes bytes onto the stream and lets the router process them.
    pub fn write(&mut self, bytes: &[u8]) {
        self.write_only(bytes);
        self.pump();
    }

    /// Serializes and writes a PDU using the ASN width this session negotiated.
    ///
    /// A test that builds an UPDATE by hand has no way to know which width the
    /// handshake settled on, and encoding a 2-octet AS_PATH onto a session the
    /// router reads as 4-octet would fail for a reason the test is not about.
    pub fn write_pdu(&mut self, pdu: toy_tcpip::bgp::BgpPdu) {
        let bytes = self.encode_pdu(pdu);
        self.write(&bytes);
    }

    /// [`RawBgpPeer::write_pdu`] without running the lab afterwards.
    pub fn write_pdu_only(&mut self, pdu: toy_tcpip::bgp::BgpPdu) {
        let bytes = self.encode_pdu(pdu);
        self.write_only(&bytes);
    }

    fn encode_pdu(&self, pdu: toy_tcpip::bgp::BgpPdu) -> Vec<u8> {
        use toy_tcpip::bgp::BgpPdu;
        match (pdu, self.four_octet_as) {
            (BgpPdu::Update(mut u), true) => {
                if let Some(attrs) = u.attributes.as_mut() {
                    attrs.four_octet_as = true;
                }
                BgpPdu::Update(u).serialize()
            }
            (other, _) => other.serialize(),
        }
    }

    /// Writes bytes without running the lab, so several writes can be batched into
    /// whatever segmentation the transport happens to choose.
    pub fn write_only(&mut self, bytes: &[u8]) {
        let n = self
            .lab
            .host_mut("peer")
            .unwrap()
            .stack
            .tcp_write(self.stream, bytes)
            .expect("tcp_write");
        assert_eq!(n, bytes.len(), "short write in the raw peer harness");
    }

    /// Half-closes the connection, as a peer vanishing mid-message would.
    pub fn disconnect(&mut self) {
        let _ = self
            .lab
            .host_mut("peer")
            .unwrap()
            .stack
            .tcp_close(self.stream);
        self.pump();
    }

    /// Reads back whatever the router sent and decodes it.
    pub fn drain(&mut self) -> Vec<toy_tcpip::bgp::BgpPdu> {
        use toy_tcpip::bgp::{BgpFramer, BgpPdu};
        use toy_tcpip::socket::SocketError;

        self.pump();
        let mut framer = BgpFramer::new();
        let mut buf = [0u8; 4096];
        loop {
            match self
                .lab
                .host_mut("peer")
                .unwrap()
                .stack
                .tcp_read(self.stream, &mut buf)
            {
                Ok(0) => break,
                Ok(n) => {
                    if framer.push(&buf[..n]).is_err() {
                        break;
                    }
                }
                Err(SocketError::WouldBlock) => break,
                Err(_) => break,
            }
        }
        let mut out = Vec::new();
        let four_octet = self.four_octet_as;
        while let Ok(Some(frame)) = framer.next_frame() {
            if let Ok(pdu) = BgpPdu::parse_width(&frame, four_octet) {
                out.push(pdu);
            }
        }
        out
    }

    /// The first NOTIFICATION the router sent, if any.
    pub fn notification(&mut self) -> Option<toy_tcpip::bgp::BgpNotificationMessage> {
        use toy_tcpip::bgp::BgpPdu;
        self.drain().into_iter().find_map(|m| match m {
            BgpPdu::Notification(n) => Some(n),
            _ => None,
        })
    }

    pub fn state(&self) -> BgpState {
        self.lab
            .router("victim")
            .unwrap()
            .bgp()
            .unwrap()
            .peer_state(self.peer)
            .unwrap()
    }

    pub fn victim_bgp(&self) -> &toy_tcpip::bgp_router::BgpRouter {
        self.lab.router("victim").unwrap().bgp().unwrap()
    }

    /// The capability set a modern neighbour offers: both families and AS4.
    pub fn modern_capabilities() -> toy_tcpip::bgp_caps::BgpCapabilitySet {
        use toy_tcpip::bgp_caps::{AfiSafi, BgpCapability, BgpCapabilitySet};
        let mut caps = BgpCapabilitySet::new();
        caps.advertise(AfiSafi::IPV4_UNICAST);
        caps.advertise(AfiSafi::L2VPN_EVPN);
        caps.push(BgpCapability::FourOctetAs(0));
        caps
    }

    /// Completes a valid OPEN / KEEPALIVE handshake advertising both address
    /// families and 4-octet ASN support, so later writes arrive in ESTABLISHED on
    /// a session that will accept EVPN NLRI.
    pub fn establish(&mut self) {
        use toy_tcpip::bgp_caps::BgpCapability;
        let mut caps = Self::modern_capabilities();
        caps.capabilities
            .retain(|c| !matches!(c, BgpCapability::FourOctetAs(_)));
        caps.push(BgpCapability::FourOctetAs(self.peer_as));
        self.establish_with(&caps);
    }

    /// Completes the handshake as a plain RFC 4271 speaker: no capabilities at
    /// all, which must still negotiate IPv4 Unicast and nothing more.
    pub fn establish_legacy(&mut self) {
        self.establish_with(&toy_tcpip::bgp_caps::BgpCapabilitySet::new());
    }

    /// Completes the handshake offering exactly `caps`.
    pub fn establish_with(&mut self, caps: &toy_tcpip::bgp_caps::BgpCapabilitySet) {
        use toy_tcpip::bgp::{BgpOpenMessage, BgpPdu};
        self.four_octet_as = caps.supports_four_octet_as();
        self.write(
            &BgpPdu::Open(BgpOpenMessage::with_capabilities(
                self.peer_as,
                LAB_HOLD_TIME,
                ip(5, 5, 5, 5),
                caps,
            ))
            .serialize(),
        );
        self.write(&BgpPdu::Keepalive.serialize());
        let peer = self.peer;
        assert!(
            self.run_until(30_000, |l| l
                .router("victim")
                .unwrap()
                .bgp()
                .unwrap()
                .peer_state(peer)
                == Some(BgpState::Established)),
            "the raw peer never reached ESTABLISHED"
        );
    }
}

// ============================================================================
// Route reflector labs (IPv4 unicast)
// ============================================================================

/// The AS every speaker in the route reflector labs belongs to. Route reflection
/// is an iBGP mechanism, so there is only one.
pub const RR_AS: u32 = 65000;

/// The hub-and-spoke route reflector lab, on one shared subnet:
///
/// ```text
///   c1 10.0.0.1    c2 10.0.0.2   n1 10.0.0.11   n2 10.0.0.12
///        \             |              |            /
///         \------- rr 10.0.0.254 (AS65000) -------/
/// ```
///
/// `c1` and `c2` are route reflector clients of `rr`; `n1` and `n2` are ordinary
/// non-client iBGP neighbours. Every spoke peers with `rr` and with nothing else,
/// and each originates one prefix of its own:
///
/// | speaker | prefix          |
/// |---------|-----------------|
/// | c1      | 172.16.1.0/24   |
/// | c2      | 172.16.2.0/24   |
/// | n1      | 172.16.11.0/24  |
/// | n2      | 172.16.12.0/24  |
///
/// The four RFC 4456 outcomes are therefore all observable at once, from which
/// prefixes turn up in which Loc-RIB, with nothing else able to explain the
/// difference: same AS, same subnet, same session type, same everything but role.
///
/// Everything sits on one subnet so that a reflected NEXT_HOP - which a reflector
/// must not rewrite - is directly reachable by whoever receives it. That is what
/// makes the reflected routes usable rather than merely present.
pub fn build_rr_lab() -> VirtualLab {
    let mut lab = VirtualLab::new();
    lab.add_link("core");

    let spokes: [(&str, u8, u8, u8, bool); 4] = [
        ("c1", 1, 1, 1, true),
        ("c2", 2, 2, 2, true),
        ("n1", 11, 11, 11, false),
        ("n2", 12, 12, 12, false),
    ];

    let mut rr = LabRouter::new("rr");
    rr.add_interface("eth0", mac(0x09, 0x00), ip(10, 0, 0, 254), 24, "core");
    rr.enable_bgp(RR_AS, ip(9, 9, 9, 9))
        .set_hold_time(LAB_HOLD_TIME);

    for (name, host, id, net, client) in spokes {
        let mut r = LabRouter::new(name);
        r.add_interface("eth0", mac(host, 0x00), ip(10, 0, 0, host), 24, "core");
        r.enable_bgp(RR_AS, ip(id, id, id, id))
            .set_hold_time(LAB_HOLD_TIME);
        r.add_bgp_peer(
            ip(10, 0, 0, 254),
            RR_AS,
            ip(10, 0, 0, host),
            BgpPeerMode::Active,
        );
        r.originate_bgp_prefix(prefix(172, 16, net, 0, 24));
        lab.add_router(r);

        rr.add_bgp_peer(
            ip(10, 0, 0, host),
            RR_AS,
            ip(10, 0, 0, 254),
            BgpPeerMode::Passive,
        );
        rr.set_bgp_route_reflector_client(ip(10, 0, 0, host), client);
    }

    lab.add_router(rr);
    lab
}

/// The prefix a named spoke of [`build_rr_lab`] originates.
pub fn rr_lab_prefix(spoke: &str) -> Ipv4Prefix {
    match spoke {
        "c1" => prefix(172, 16, 1, 0, 24),
        "c2" => prefix(172, 16, 2, 0, 24),
        "n1" => prefix(172, 16, 11, 0, 24),
        "n2" => prefix(172, 16, 12, 0, 24),
        other => panic!("no such spoke in the route reflector lab: {}", other),
    }
}

/// The session address of a named spoke of [`build_rr_lab`].
pub fn rr_lab_addr(spoke: &str) -> Ipv4Address {
    match spoke {
        "c1" => ip(10, 0, 0, 1),
        "c2" => ip(10, 0, 0, 2),
        "n1" => ip(10, 0, 0, 11),
        "n2" => ip(10, 0, 0, 12),
        "rr" => ip(10, 0, 0, 254),
        other => panic!("no such router in the route reflector lab: {}", other),
    }
}

/// Two iBGP speakers on one wire, *both* configured to open the connection.
///
/// Nothing here is passive, so each dials the other and each accepts the other's
/// call: a genuine connection collision, on real TCP, every single run. RFC 4271
/// section 6.8 says exactly one of the two connections may survive it, and which
/// one is decided by comparing BGP identifiers - `left` is 1.1.1.1 and `right` is
/// 2.2.2.2, so the connection `right` initiated is the one that must remain.
pub fn build_collision_lab() -> VirtualLab {
    let mut lab = VirtualLab::new();
    lab.add_link("wire");

    let mut left = LabRouter::new("left");
    left.add_interface("eth0", mac(0x01, 0x00), ip(10, 9, 0, 1), 24, "wire");
    left.enable_bgp(RR_AS, ip(1, 1, 1, 1))
        .set_hold_time(LAB_HOLD_TIME);
    left.add_bgp_peer(ip(10, 9, 0, 2), RR_AS, ip(10, 9, 0, 1), BgpPeerMode::Active);
    left.originate_bgp_prefix(prefix(172, 20, 1, 0, 24));

    let mut right = LabRouter::new("right");
    right.add_interface("eth0", mac(0x02, 0x00), ip(10, 9, 0, 2), 24, "wire");
    right
        .enable_bgp(RR_AS, ip(2, 2, 2, 2))
        .set_hold_time(LAB_HOLD_TIME);
    right.add_bgp_peer(ip(10, 9, 0, 1), RR_AS, ip(10, 9, 0, 2), BgpPeerMode::Active);
    right.originate_bgp_prefix(prefix(172, 20, 2, 0, 24));

    lab.add_router(left);
    lab.add_router(right);
    lab
}
