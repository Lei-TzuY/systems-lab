//! Shared helpers for the BGP route reflector and EVPN route reflector suites.
//!
//! Nothing here injects control-plane state. The builders live in `toy_tcpip::lab`
//! and are real labs; what this module adds is the reading of what came out:
//! capture decoding, convergence predicates, and the small amount of arithmetic
//! several suites would otherwise each restate.

#![allow(dead_code)]

use toy_tcpip::bgp::{BGP_PORT, BgpFramer, BgpPdu};
use toy_tcpip::bgp_evpn::EvpnRouteKey;
use toy_tcpip::ethernet::{EtherType, EthernetFrame, MacAddress};
use toy_tcpip::ipv4::{IpProtocol, Ipv4Address, Ipv4Packet};
use toy_tcpip::lab::VirtualLab;
use toy_tcpip::tcp::TcpSegment;
use toy_tcpip::udp::UdpDatagram;
use toy_tcpip::vxlan::{VXLAN_UDP_PORT, VxlanPacket};

/// The tenant hosts used by every route reflector fabric.
pub const HOST_A: Ipv4Address = Ipv4Address([192, 168, 10, 11]);
pub const HOST_B: Ipv4Address = Ipv4Address([192, 168, 10, 22]);
pub const MAC_A: MacAddress = MacAddress([0x02, 0x00, 0x00, 0x00, 0x0A, 0x0A]);
pub const MAC_B: MacAddress = MacAddress([0x02, 0x00, 0x00, 0x00, 0x0B, 0x0B]);

/// Runs the lab until every configured session on every speaker is ESTABLISHED
/// and carries the EVPN family.
pub fn converge_sessions_evpn(lab: &mut VirtualLab, max_sim_ms: u64) -> bool {
    lab.run_until(250, max_sim_ms, |l| {
        l.routers
            .values()
            .filter_map(|r| r.bgp())
            .all(|b| !b.peers().is_empty() && b.peers().iter().all(|p| p.carries_evpn()))
    })
}

/// Makes host A ping host B, which is what makes leaf1 learn a local MAC and so
/// the only thing that puts an EVPN route into the fabric at all.
pub fn ping_a_to_b(lab: &mut VirtualLab, ident: u16, seq: u16) {
    let frame = lab
        .host_mut("host_a")
        .unwrap()
        .stack
        .ping4(HOST_B, ident, seq, b"rr")
        .expect("host_a produced no frame");
    lab.send_from_host("host_a", frame);
    lab.run_until(250, 60_000, |_| false);
}

/// Makes host B ping host A.
pub fn ping_b_to_a(lab: &mut VirtualLab, ident: u16, seq: u16) {
    let frame = lab
        .host_mut("host_b")
        .unwrap()
        .stack
        .ping4(HOST_A, ident, seq, b"rr")
        .expect("host_b produced no frame");
    lab.send_from_host("host_b", frame);
    lab.run_until(250, 60_000, |_| false);
}

/// True when `host_a` has received at least one ICMP echo reply from host B.
pub fn host_a_heard_back(lab: &VirtualLab) -> bool {
    lab.host("host_a")
        .unwrap()
        .stack
        .received_icmp_replies
        .iter()
        .any(|(src, _, _)| *src == HOST_B)
}

/// True when `host_b` has received at least one ICMP echo reply from host A.
pub fn host_b_heard_back(lab: &VirtualLab) -> bool {
    lab.host("host_b")
        .unwrap()
        .stack
        .received_icmp_replies
        .iter()
        .any(|(src, _, _)| *src == HOST_A)
}

/// The remote MAC entry a leaf holds for `mac` in `vni`, if any.
pub fn remote_mac(lab: &VirtualLab, leaf: &str, vni: u32, mac: MacAddress) -> Option<Ipv4Address> {
    lab.router(leaf)?.vtep()?.lookup_remote(vni, &mac)
}

/// Every EVPN route key a speaker currently holds in its Adj-RIB-In, per peer.
pub fn adj_rib_in_keys(lab: &VirtualLab, router: &str, peer: Ipv4Address) -> Vec<EvpnRouteKey> {
    lab.router(router)
        .and_then(|r| r.bgp())
        .and_then(|b| b.evpn_adj_rib_in.peer_table(peer))
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default()
}

/// The total number of UPDATE messages every speaker in the lab has sent.
///
/// This is the churn measure the quiescence tests use: after convergence it must
/// stop moving, and a reflection loop is exactly the thing that would keep it
/// climbing forever.
pub fn total_updates_sent(lab: &VirtualLab) -> u64 {
    lab.routers
        .values()
        .filter_map(|r| r.bgp())
        .flat_map(|b| b.peers())
        .map(|p| p.counters.updates_sent)
        .sum()
}

/// The total number of UPDATE messages every speaker has received.
pub fn total_updates_received(lab: &VirtualLab) -> u64 {
    lab.routers
        .values()
        .filter_map(|r| r.bgp())
        .flat_map(|b| b.peers())
        .map(|p| p.counters.updates_received)
        .sum()
}

/// How many times any speaker has run either decision process.
pub fn total_decision_runs(lab: &VirtualLab) -> u64 {
    lab.routers
        .values()
        .filter_map(|r| r.bgp())
        .map(|b| b.decision_runs + b.evpn_decision_runs)
        .sum()
}

/// The longest CLUSTER_LIST any speaker is currently storing.
pub fn longest_cluster_list(lab: &VirtualLab) -> usize {
    lab.routers
        .values()
        .filter_map(|r| r.bgp())
        .flat_map(|b| {
            let evpn = b
                .evpn_adj_rib_in
                .iter_paths()
                .map(|p| p.cluster_list.len())
                .collect::<Vec<_>>();
            let ipv4 = b
                .adj_rib_in
                .iter_paths()
                .map(|p| p.cluster_list.len())
                .collect::<Vec<_>>();
            evpn.into_iter().chain(ipv4)
        })
        .max()
        .unwrap_or(0)
}

/// One decoded packet from a capture.
#[derive(Debug, Clone)]
pub struct Captured {
    pub ethertype: EtherType,
    pub src: Ipv4Address,
    pub dst: Ipv4Address,
    pub protocol: Option<IpProtocol>,
    /// The IPv4 payload, or the Ethernet payload for a non-IPv4 frame.
    pub payload: Vec<u8>,
}

/// Reads a capture with this repository's own PCAP reader.
pub fn read_capture(pcap: &[u8]) -> Vec<Captured> {
    use toy_tcpip::pcap::PcapReader;

    let mut reader = PcapReader::new(std::io::Cursor::new(pcap)).expect("PcapReader");
    let mut out = Vec::new();
    while let Ok(Some(pkt)) = reader.next_packet() {
        let Ok(eth) = EthernetFrame::parse(&pkt.data) else {
            continue;
        };
        match eth.ethertype {
            EtherType::IPv4 => {
                if let Ok(ipv4) = Ipv4Packet::parse(eth.payload, false) {
                    out.push(Captured {
                        ethertype: EtherType::IPv4,
                        src: ipv4.header.src_ip,
                        dst: ipv4.header.dst_ip,
                        protocol: Some(ipv4.header.protocol),
                        payload: ipv4.payload.to_vec(),
                    });
                }
            }
            other => out.push(Captured {
                ethertype: other,
                src: Ipv4Address::UNSPECIFIED,
                dst: Ipv4Address::UNSPECIFIED,
                protocol: None,
                payload: eth.payload.to_vec(),
            }),
        }
    }
    out
}

/// Reassembles the BGP messages one speaker sent to another, from captured TCP
/// segments alone.
///
/// This is the same problem the speaker's own framer solves, with two extra
/// complications a capture brings: a retransmission appears twice, and a segment
/// may be missing entirely. Keying by sequence number handles the first, and
/// stopping at the first gap handles the second, because a stream reassembled
/// across a hole decodes nonsense.
pub fn bgp_stream(packets: &[Captured], from: Ipv4Address, to: Ipv4Address) -> Vec<BgpPdu> {
    use std::collections::BTreeMap;

    let mut by_seq: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    for p in packets {
        if p.src != from || p.dst != to || p.protocol != Some(IpProtocol::Tcp) {
            continue;
        }
        let Ok(seg) = TcpSegment::parse(p.src, p.dst, &p.payload, false) else {
            continue;
        };
        if seg.src_port != BGP_PORT && seg.dst_port != BGP_PORT {
            continue;
        }
        if seg.payload.is_empty() {
            continue;
        }
        by_seq.insert(seg.seq_num, seg.payload.to_vec());
    }

    let mut framer = BgpFramer::new();
    let mut out = Vec::new();
    let mut expect: Option<u32> = None;
    for (seq, payload) in by_seq {
        if expect.is_some_and(|e| e != seq) {
            break;
        }
        if framer.push(&payload).is_err() {
            break;
        }
        expect = Some(seq.wrapping_add(payload.len() as u32));
        while let Ok(Some(frame)) = framer.next_frame() {
            // Every fabric here negotiates 4-octet ASNs, so that is how the
            // AS_PATH on these sessions is written.
            if let Ok(pdu) = BgpPdu::parse_width(&frame, true) {
                out.push(pdu);
            }
        }
    }
    out
}

/// `(outer src, outer dst, VNI, inner frame)` for every VXLAN packet captured.
pub fn captured_vxlan(pcap: &[u8]) -> Vec<(Ipv4Address, Ipv4Address, u32, Vec<u8>)> {
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
        if ipv4.header.protocol != IpProtocol::Udp {
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

/// The inner `(src MAC, dst MAC)` of every VXLAN packet carrying `vni`.
pub fn vxlan_inner_macs(pcap: &[u8], vni: u32) -> Vec<(MacAddress, MacAddress)> {
    captured_vxlan(pcap)
        .into_iter()
        .filter(|(_, _, v, _)| *v == vni)
        .filter_map(|(_, _, _, inner)| {
            EthernetFrame::parse(&inner)
                .ok()
                .map(|e| (e.src_mac, e.dst_mac))
        })
        .collect()
}

/// The distinct outer destinations VXLAN was sent to, in first-seen order.
pub fn vxlan_destinations(pcap: &[u8]) -> Vec<Ipv4Address> {
    let mut out: Vec<Ipv4Address> = Vec::new();
    for (_, dst, _, _) in captured_vxlan(pcap) {
        if !out.contains(&dst) {
            out.push(dst);
        }
    }
    out
}
