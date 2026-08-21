//! Integrated Deterministic Virtual Network Lab.
//!
//! Provides a complete in-process virtual networking testbed supporting:
//! - Multi-node, multi-subnet topologies with virtual links, switches, and routers
//! - Deterministic link fault injection (MTU limits, packet drops, byte corruption)
//! - Multi-interface IPv4 routing, TTL decrementing, and ICMP Time Exceeded generation
//! - Full dual-stack protocol operation (Ethernet, ARP, IPv4, IPv6, ICMP, ICMPv6, NDP, UDP, TCP)
//! - Integrated PCAP capture tap per link with Wireshark compatibility
//! - Discrete event stepping and run-to-quiescence simulation

use crate::arp::{ArpOpcode, ArpPacket, ArpTable};
use crate::ethernet::{
    ETHERTYPE_ARP, ETHERTYPE_IPV4, ETHERTYPE_MPLS, EtherType, EthernetFrame, MacAddress,
};
use crate::firewall::{Firewall, FirewallAction, FirewallChain};
use crate::icmp::{IcmpPacket, IcmpType};
use crate::ipv4::{IP_PROTO_ICMP, IP_PROTO_UDP, Ipv4Address, Ipv4Packet};
use crate::mpls::{LfibAction, LfibTable, MplsHeader, MplsPacket};
use crate::nat::NatTable;
use crate::ospf::OspfLsdb;
use crate::pcap::{LINKTYPE_ETHERNET, PcapWriter};
use crate::rip::{RIP_PORT, RipEngine, RipPacket};
use crate::router::RoutingTable;
use crate::stack::{NetStack, NetStackConfig};
use crate::udp::UdpDatagram;
use crate::vxlan::{VXLAN_UDP_PORT, VxlanPacket};
use std::collections::HashMap;

/// Fault injection configuration and frame accounting for a virtual point-to-point or broadcast link.
#[derive(Debug)]
pub struct VirtualLink {
    pub name: String,
    pub mtu: usize,
    pub drop_packet_indices: Vec<usize>,
    pub corrupt_packet_indices: Vec<usize>,
    pub total_packets_seen: usize,
    pub frames_forwarded: usize,
    pub frames_dropped: usize,
    pub frames_corrupted: usize,
    pub in_flight_frames: Vec<Vec<u8>>,
    pub pcap_writer: Option<PcapWriter<Vec<u8>>>,
}

impl VirtualLink {
    pub fn new(name: &str) -> Self {
        VirtualLink {
            name: name.to_string(),
            mtu: 1500,
            drop_packet_indices: Vec::new(),
            corrupt_packet_indices: Vec::new(),
            total_packets_seen: 0,
            frames_forwarded: 0,
            frames_dropped: 0,
            frames_corrupted: 0,
            in_flight_frames: Vec::new(),
            pcap_writer: None,
        }
    }

    pub fn with_mtu(mut self, mtu: usize) -> Self {
        self.mtu = mtu;
        self
    }

    pub fn with_drop_indices(mut self, indices: &[usize]) -> Self {
        self.drop_packet_indices.extend_from_slice(indices);
        self
    }

    pub fn with_corrupt_indices(mut self, indices: &[usize]) -> Self {
        self.corrupt_packet_indices.extend_from_slice(indices);
        self
    }

    /// Enables continuous PCAP capture tap on this link.
    pub fn enable_pcap(&mut self) {
        let buffer = Vec::new();
        let writer = PcapWriter::new(buffer, 65535, LINKTYPE_ETHERNET).expect("PcapWriter init");
        self.pcap_writer = Some(writer);
    }

    pub fn take_pcap_bytes(&mut self) -> Option<Vec<u8>> {
        self.pcap_writer.as_ref().map(|w| w.get_ref().clone())
    }

    /// Configures deterministic link MTU limit in bytes.
    pub fn set_mtu(&mut self, mtu: usize) {
        self.mtu = mtu;
    }

    /// Adds zero-indexed packet numbers that must be dropped.
    pub fn drop_packet_indices(&mut self, indices: &[usize]) {
        self.drop_packet_indices.extend_from_slice(indices);
    }

    /// Adds zero-indexed packet numbers whose payloads must be corrupted with bit inversion.
    pub fn corrupt_packet_indices(&mut self, indices: &[usize]) {
        self.corrupt_packet_indices.extend_from_slice(indices);
    }

    /// Processes a frame attempting to cross this link.
    /// Returns Some(frame) if delivered, or None if dropped/clipped.
    pub fn process_frame_transit(&mut self, mut raw_frame: Vec<u8>) -> Option<Vec<u8>> {
        self.total_packets_seen += 1;
        let pkt_index = self.total_packets_seen;

        // 1. Check MTU
        if raw_frame.len() > self.mtu + 14 {
            // Frame payload + Ethernet header exceeds link capacity
            self.frames_dropped += 1;
            return None;
        }

        // 2. Check deterministic drop rule
        if self.drop_packet_indices.contains(&pkt_index) {
            self.frames_dropped += 1;
            return None;
        }

        // 3. Check deterministic corruption rule
        if self.corrupt_packet_indices.contains(&pkt_index) && raw_frame.len() > 20 {
            let corrupt_pos = raw_frame.len() - 1;
            raw_frame[corrupt_pos] ^= 0xFF;
            self.frames_corrupted += 1;
        }

        // 4. Capture in PCAP tap if enabled
        if let Some(ref mut writer) = self.pcap_writer {
            let ts_sec = (pkt_index as u32) / 10;
            let ts_usec = ((pkt_index as u32) % 10) * 100_000;
            let _ = writer.write_packet(ts_sec, ts_usec, &raw_frame);
        }

        self.frames_forwarded += 1;
        Some(raw_frame)
    }

    /// Enqueues a raw frame onto the virtual link for propagation.
    pub fn push_frame(&mut self, frame: Vec<u8>) {
        if let Some(delivered) = self.process_frame_transit(frame) {
            self.in_flight_frames.push(delivered);
        }
    }

    /// Drains all currently queued frames on the link.
    pub fn drain_frames(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.in_flight_frames)
    }
}

/// Simulated Host endpoint running a NetStack attached to a virtual link.
pub struct LabHost {
    pub name: String,
    pub link_name: String,
    pub stack: NetStack,
}

impl LabHost {
    pub fn new(name: &str, link_name: &str, config: NetStackConfig) -> Self {
        LabHost {
            name: name.to_string(),
            link_name: link_name.to_string(),
            stack: NetStack::new(config),
        }
    }
}

/// Single network interface on a virtual router.
#[derive(Debug, Clone)]
pub struct RouterInterface {
    pub name: String,
    pub mac: MacAddress,
    pub ip: Ipv4Address,
    pub subnet_mask: u8,
    pub link_name: String,
}

/// A multi-interface Router node with hardware-like packet forwarding, TTL decrementing,
/// NAT translation (SNAT & DNAT), dynamic routing (RIPv2 & OSPFv2), stateful firewalling,
/// VXLAN overlay bridging, MPLS label switching, and ICMP error generation.
pub struct LabRouter {
    pub name: String,
    pub interfaces: Vec<RouterInterface>,
    pub routing_table: RoutingTable,
    pub arp_tables: HashMap<String, ArpTable>,
    pub pending_transit_packets: HashMap<(String, Ipv4Address), Vec<Vec<u8>>>,
    pub nat_table: Option<NatTable>,
    pub nat_lan_iface: Option<String>,
    pub nat_wan_iface: Option<String>,
    pub rip_engine: Option<RipEngine>,
    pub firewall: Option<Firewall>,
    pub vxlan_tunnels: HashMap<String, (u32, Ipv4Address, String)>,
    pub vxlan_vni_to_access: HashMap<u32, String>,
    pub ospf_lsdb: Option<OspfLsdb>,
    pub lfib: Option<LfibTable>,
    pub mpls_push_routes: HashMap<Ipv4Address, (u32, String)>,
    pub ip_id_counter: u16,
}

impl LabRouter {
    pub fn new(name: &str) -> Self {
        LabRouter {
            name: name.to_string(),
            interfaces: Vec::new(),
            routing_table: RoutingTable::new(),
            arp_tables: HashMap::new(),
            pending_transit_packets: HashMap::new(),
            nat_table: None,
            nat_lan_iface: None,
            nat_wan_iface: None,
            rip_engine: None,
            firewall: None,
            vxlan_tunnels: HashMap::new(),
            vxlan_vni_to_access: HashMap::new(),
            ospf_lsdb: None,
            lfib: None,
            mpls_push_routes: HashMap::new(),
            ip_id_counter: 100,
        }
    }

    pub fn set_firewall(&mut self, fw: Firewall) {
        self.firewall = Some(fw);
    }

    pub fn add_vxlan_tunnel(
        &mut self,
        access_iface: &str,
        vni: u32,
        remote_vtep_ip: Ipv4Address,
        underlay_iface: &str,
    ) {
        self.vxlan_tunnels.insert(
            access_iface.to_string(),
            (vni, remote_vtep_ip, underlay_iface.to_string()),
        );
        self.vxlan_vni_to_access
            .insert(vni, access_iface.to_string());
    }

    pub fn enable_ospf(&mut self) {
        self.ospf_lsdb = Some(OspfLsdb::new());
    }

    pub fn add_ospf_link(&mut self, from: Ipv4Address, to: Ipv4Address, cost: u32) {
        if let Some(ref mut lsdb) = self.ospf_lsdb {
            lsdb.add_link(from, to, cost);
        }
    }

    pub fn run_ospf_spf(
        &mut self,
        router_id: Ipv4Address,
        neighbor_subnets: &HashMap<Ipv4Address, (Ipv4Address, u8, String, Ipv4Address)>,
    ) {
        if let Some(ref lsdb) = self.ospf_lsdb {
            let shortest_paths = lsdb.compute_shortest_paths(router_id);
            for (dest_router, (_cost, next_hop_opt)) in shortest_paths {
                if let Some(next_hop_router) = next_hop_opt
                    && let Some((dest_net, mask, iface_name, next_hop_ip)) =
                        neighbor_subnets.get(&dest_router)
                {
                    let n_hop = if next_hop_router == dest_router {
                        *next_hop_ip
                    } else if let Some((_, _, _, nh_ip)) = neighbor_subnets.get(&next_hop_router) {
                        *nh_ip
                    } else {
                        *next_hop_ip
                    };
                    self.routing_table
                        .add_route(*dest_net, *mask, Some(n_hop), iface_name);
                }
            }
        }
    }

    pub fn enable_mpls(&mut self) {
        self.lfib = Some(LfibTable::new());
    }

    pub fn add_mpls_push_route(&mut self, dst_ip: Ipv4Address, label: u32, egress_iface: &str) {
        self.mpls_push_routes
            .insert(dst_ip, (label, egress_iface.to_string()));
    }

    pub fn add_mpls_swap_route(&mut self, in_label: u32, out_label: u32, egress_iface: &str) {
        let lfib = self.lfib.get_or_insert_with(LfibTable::new);
        lfib.insert(
            in_label,
            LfibAction::Swap(out_label, egress_iface.to_string()),
        );
    }

    pub fn add_mpls_pop_route(&mut self, in_label: u32) {
        let lfib = self.lfib.get_or_insert_with(LfibTable::new);
        lfib.insert(in_label, LfibAction::Pop);
    }

    pub fn enable_nat(&mut self, lan_iface: &str, wan_iface: &str, public_ip: Ipv4Address) {
        self.nat_table = Some(NatTable::new(public_ip));
        self.nat_lan_iface = Some(lan_iface.to_string());
        self.nat_wan_iface = Some(wan_iface.to_string());
    }

    pub fn add_port_forward(
        &mut self,
        ext_port: u16,
        int_ip: Ipv4Address,
        int_port: u16,
        proto: u8,
    ) {
        if let Some(ref mut nat) = self.nat_table {
            nat.add_port_forward(ext_port, int_ip, int_port, proto);
        }
    }

    pub fn enable_rip(&mut self) {
        let mut rip = RipEngine::new();
        for iface in &self.interfaces {
            let subnet_net = iface.ip.mask(iface.subnet_mask);
            rip.add_local_network(subnet_net, iface.subnet_mask, &iface.name);
        }
        self.routing_table = rip.routes.clone();
        self.rip_engine = Some(rip);
    }

    pub fn generate_rip_advertisements(&self) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        if let Some(ref rip) = self.rip_engine {
            let rip_pkt = rip.build_advertisement();
            let rip_bytes = rip_pkt.serialize();
            for iface in &self.interfaces {
                let udp_bytes = UdpDatagram::serialize(
                    iface.ip,
                    Ipv4Address([224, 0, 0, 9]),
                    RIP_PORT,
                    RIP_PORT,
                    &rip_bytes,
                );
                let ip_bytes = Ipv4Packet::serialize(
                    iface.ip,
                    Ipv4Address([224, 0, 0, 9]),
                    IP_PROTO_UDP,
                    100,
                    1,
                    &udp_bytes,
                );
                let eth_bytes = EthernetFrame::serialize(
                    MacAddress([0x01, 0x00, 0x5E, 0x00, 0x00, 0x09]),
                    iface.mac,
                    ETHERTYPE_IPV4,
                    &ip_bytes,
                );
                out.push((iface.link_name.clone(), eth_bytes));
            }
        }
        out
    }

    pub fn add_interface(
        &mut self,
        name: &str,
        mac: MacAddress,
        ip: Ipv4Address,
        subnet_mask: u8,
        link_name: &str,
    ) {
        let iface = RouterInterface {
            name: name.to_string(),
            mac,
            ip,
            subnet_mask,
            link_name: link_name.to_string(),
        };
        self.arp_tables.insert(name.to_string(), ArpTable::new());

        // Add local connected subnet route
        let subnet_net = ip.mask(subnet_mask);
        self.routing_table
            .add_route(subnet_net, subnet_mask, None, name);
        self.interfaces.push(iface);
    }

    fn next_ip_id(&mut self) -> u16 {
        let id = self.ip_id_counter;
        self.ip_id_counter = self.ip_id_counter.wrapping_add(1);
        id
    }

    /// Processes an incoming frame arriving on a specific virtual link.
    /// Returns a list of `(egress_link_name, frame_bytes)` to transmit.
    pub fn process_incoming_frame(
        &mut self,
        ingress_link: &str,
        raw_frame: &[u8],
    ) -> Vec<(String, Vec<u8>)> {
        let mut out_transmissions = Vec::new();

        let ingress_iface = match self.interfaces.iter().find(|i| i.link_name == ingress_link) {
            Some(i) => i.clone(),
            None => return out_transmissions,
        };

        // Check if this ingress interface is a VXLAN Access port
        if let Some(&(vni, remote_vtep_ip, ref underlay_iface_name)) =
            self.vxlan_tunnels.get(&ingress_iface.name)
        {
            if let Some(underlay_iface) = self
                .interfaces
                .iter()
                .find(|i| i.name == *underlay_iface_name)
                .cloned()
                && let Ok(vxlan_bytes) = VxlanPacket::encapsulate(vni, raw_frame)
            {
                let udp_bytes = UdpDatagram::serialize(
                    underlay_iface.ip,
                    remote_vtep_ip,
                    VXLAN_UDP_PORT,
                    VXLAN_UDP_PORT,
                    &vxlan_bytes,
                );
                let ip_id = self.next_ip_id();
                let ip_bytes = Ipv4Packet::serialize(
                    underlay_iface.ip,
                    remote_vtep_ip,
                    IP_PROTO_UDP,
                    ip_id,
                    64,
                    &udp_bytes,
                );

                if let Some(route) = self.routing_table.lookup(remote_vtep_ip)
                    && let Some(egress) = self.interfaces.iter().find(|i| i.name == route.interface)
                {
                    let next_hop = route.next_hop(remote_vtep_ip);
                    let egress_arp = self.arp_tables.entry(egress.name.clone()).or_default();
                    if let Some(dst_mac) = egress_arp.lookup(&next_hop.0) {
                        let eth_out = EthernetFrame::serialize(
                            dst_mac,
                            egress.mac,
                            ETHERTYPE_IPV4,
                            &ip_bytes,
                        );
                        out_transmissions.push((egress.link_name.clone(), eth_out));
                    } else {
                        let pending_key = (egress.name.clone(), next_hop);
                        self.pending_transit_packets
                            .entry(pending_key)
                            .or_default()
                            .push(ip_bytes);
                        let arp_req = ArpPacket::build_request(egress.mac, egress.ip.0, next_hop.0);
                        let eth_arp = EthernetFrame::serialize(
                            MacAddress::BROADCAST,
                            egress.mac,
                            ETHERTYPE_ARP,
                            &arp_req.serialize(),
                        );
                        out_transmissions.push((egress.link_name.clone(), eth_arp));
                    }
                }
            }
            return out_transmissions;
        }

        let eth = match EthernetFrame::parse(raw_frame) {
            Ok(f) => f,
            Err(_) => return out_transmissions,
        };

        // Filter: only accept if destination is ingress interface MAC, broadcast, or multicast
        if !eth.dst_mac.is_broadcast()
            && !eth.dst_mac.is_multicast()
            && eth.dst_mac != ingress_iface.mac
        {
            return out_transmissions;
        }

        match eth.ethertype {
            EtherType::Arp => {
                if let Ok(arp) = ArpPacket::parse(eth.payload) {
                    let arp_table = self
                        .arp_tables
                        .entry(ingress_iface.name.clone())
                        .or_default();
                    arp_table.insert(arp.sender_ip, arp.sender_mac);
                    let sender_ipv4 = Ipv4Address(arp.sender_ip);

                    // Check pending transit packets waiting for this ARP on this interface
                    let pending_key = (ingress_iface.name.clone(), sender_ipv4);
                    if let Some(queued) = self.pending_transit_packets.remove(&pending_key) {
                        for ip_data in queued {
                            let eth_out = EthernetFrame::serialize(
                                arp.sender_mac,
                                ingress_iface.mac,
                                ETHERTYPE_IPV4,
                                &ip_data,
                            );
                            out_transmissions.push((ingress_link.to_string(), eth_out));
                        }
                    }

                    if arp.opcode == ArpOpcode::Request && arp.target_ip == ingress_iface.ip.0 {
                        // Generate ARP reply
                        let reply = ArpPacket::build_reply(
                            ingress_iface.mac,
                            ingress_iface.ip.0,
                            arp.sender_mac,
                            arp.sender_ip,
                        );
                        let eth_out = EthernetFrame::serialize(
                            arp.sender_mac,
                            ingress_iface.mac,
                            ETHERTYPE_ARP,
                            &reply.serialize(),
                        );
                        out_transmissions.push((ingress_link.to_string(), eth_out));
                    }
                }
            }

            EtherType::Mpls => {
                if let Ok(mpls_pkt) = MplsPacket::parse(eth.payload)
                    && let Some(top_hdr) = mpls_pkt.labels.first()
                    && let Some(ref lfib) = self.lfib
                {
                    match lfib.lookup(top_hdr.label) {
                        Some(LfibAction::Swap(out_label, egress_name)) => {
                            let mut new_labels = mpls_pkt.labels.clone();
                            new_labels[0].label = *out_label;
                            if new_labels[0].ttl > 1 {
                                new_labels[0].ttl -= 1;
                            }
                            let new_mpls = MplsPacket::new(new_labels, mpls_pkt.payload);
                            let mpls_bytes = new_mpls.serialize();

                            if let Some(egress_iface) =
                                self.interfaces.iter().find(|i| i.name == *egress_name)
                            {
                                let eth_out = EthernetFrame::serialize(
                                    MacAddress::BROADCAST,
                                    egress_iface.mac,
                                    ETHERTYPE_MPLS,
                                    &mpls_bytes,
                                );
                                out_transmissions.push((egress_iface.link_name.clone(), eth_out));
                            }
                        }
                        Some(LfibAction::Pop) => {
                            if top_hdr.bottom_of_stack
                                && let Ok(inner_ip) = Ipv4Packet::parse(&mpls_pkt.payload, false)
                                && let Some(route) =
                                    self.routing_table.lookup(inner_ip.header.dst_ip)
                                && let Some(egress_iface) =
                                    self.interfaces.iter().find(|i| i.name == route.interface)
                            {
                                let next_hop = route.next_hop(inner_ip.header.dst_ip);
                                let egress_arp = self
                                    .arp_tables
                                    .entry(egress_iface.name.clone())
                                    .or_default();
                                if let Some(dst_mac) = egress_arp.lookup(&next_hop.0) {
                                    let eth_out = EthernetFrame::serialize(
                                        dst_mac,
                                        egress_iface.mac,
                                        ETHERTYPE_IPV4,
                                        &mpls_pkt.payload,
                                    );
                                    out_transmissions
                                        .push((egress_iface.link_name.clone(), eth_out));
                                } else {
                                    let pending_key = (egress_iface.name.clone(), next_hop);
                                    self.pending_transit_packets
                                        .entry(pending_key)
                                        .or_default()
                                        .push(mpls_pkt.payload);
                                    let arp_req = ArpPacket::build_request(
                                        egress_iface.mac,
                                        egress_iface.ip.0,
                                        next_hop.0,
                                    );
                                    let eth_arp = EthernetFrame::serialize(
                                        MacAddress::BROADCAST,
                                        egress_iface.mac,
                                        ETHERTYPE_ARP,
                                        &arp_req.serialize(),
                                    );
                                    out_transmissions
                                        .push((egress_iface.link_name.clone(), eth_arp));
                                }
                            }
                        }
                        None | Some(LfibAction::Push(_)) => {}
                    }
                }
            }

            EtherType::IPv4 => {
                if let Ok(ip_pkt) = Ipv4Packet::parse(eth.payload, true) {
                    let is_for_router =
                        self.interfaces.iter().any(|i| i.ip == ip_pkt.header.dst_ip);

                    // 1. Evaluate Firewall Input or Forward Chain
                    if let Some(ref fw) = self.firewall {
                        let chain = if is_for_router {
                            FirewallChain::Input
                        } else {
                            FirewallChain::Forward
                        };
                        if fw.evaluate(chain, &ip_pkt) != FirewallAction::Accept {
                            return out_transmissions; // Dropped by firewall!
                        }
                    }

                    // Update ARP table on ingress interface with sender
                    let arp_table = self
                        .arp_tables
                        .entry(ingress_iface.name.clone())
                        .or_default();
                    arp_table.insert(ip_pkt.header.src_ip.0, eth.src_mac);

                    // Check for RIPv2 multicast or direct UDP packets
                    if ip_pkt.header.protocol == crate::ipv4::IpProtocol::Udp
                        && let Ok(udp) = UdpDatagram::parse(
                            ip_pkt.header.src_ip,
                            ip_pkt.header.dst_ip,
                            ip_pkt.payload,
                            false,
                        )
                    {
                        // RIPv2
                        if udp.dst_port == RIP_PORT {
                            if let Some(ref mut rip) = self.rip_engine
                                && let Ok(rip_pkt) = RipPacket::parse(udp.payload)
                            {
                                rip.process_advertisement(
                                    ip_pkt.header.src_ip,
                                    &rip_pkt,
                                    &ingress_iface.name,
                                );
                                self.routing_table = rip.routes.clone();
                            }
                            return out_transmissions;
                        }

                        // VXLAN Decapsulation (UDP 4789)
                        if udp.dst_port == VXLAN_UDP_PORT
                            && is_for_router
                            && let Ok(vxlan) = VxlanPacket::parse(udp.payload)
                            && let Some(access_name) =
                                self.vxlan_vni_to_access.get(&vxlan.header.vni)
                            && let Some(access_iface) =
                                self.interfaces.iter().find(|i| i.name == *access_name)
                        {
                            out_transmissions
                                .push((access_iface.link_name.clone(), vxlan.inner_frame));
                            return out_transmissions;
                        }
                    }

                    if is_for_router {
                        // Check if Inbound NAT (DNAT) translates this WAN packet for a LAN host
                        if let Some(ref mut nat) = self.nat_table
                            && self.nat_wan_iface.as_deref() == Some(&ingress_iface.name)
                        {
                            let mut ip_buf = eth.payload.to_vec();
                            if nat.translate_inbound(&mut ip_buf)
                                && let Ok(trans_ip) = Ipv4Packet::parse(&ip_buf, true)
                                && let Some(route) =
                                    self.routing_table.lookup(trans_ip.header.dst_ip)
                                && let Some(egress_iface) =
                                    self.interfaces.iter().find(|i| i.name == route.interface)
                            {
                                let egress_link = egress_iface.link_name.clone();
                                let next_hop = route.next_hop(trans_ip.header.dst_ip);
                                let egress_arp = self
                                    .arp_tables
                                    .entry(egress_iface.name.clone())
                                    .or_default();
                                if let Some(dst_mac) = egress_arp.lookup(&next_hop.0) {
                                    let eth_out = EthernetFrame::serialize(
                                        dst_mac,
                                        egress_iface.mac,
                                        ETHERTYPE_IPV4,
                                        &ip_buf,
                                    );
                                    out_transmissions.push((egress_link, eth_out));
                                    return out_transmissions;
                                } else {
                                    let pending_key = (egress_iface.name.clone(), next_hop);
                                    self.pending_transit_packets
                                        .entry(pending_key)
                                        .or_default()
                                        .push(ip_buf);
                                    let arp_req = ArpPacket::build_request(
                                        egress_iface.mac,
                                        egress_iface.ip.0,
                                        next_hop.0,
                                    );
                                    let eth_arp = EthernetFrame::serialize(
                                        MacAddress::BROADCAST,
                                        egress_iface.mac,
                                        ETHERTYPE_ARP,
                                        &arp_req.serialize(),
                                    );
                                    out_transmissions.push((egress_link, eth_arp));
                                    return out_transmissions;
                                }
                            }
                        }

                        // Direct packet to router's own IP (e.g. pinging the router)
                        if ip_pkt.header.protocol == crate::ipv4::IpProtocol::Icmp
                            && let Ok(icmp) = IcmpPacket::parse(ip_pkt.payload, true)
                            && icmp.icmp_type == IcmpType::EchoRequest
                        {
                            let echo_reply = IcmpPacket::build_echo_reply(&icmp);
                            let ip_id = self.next_ip_id();
                            let ip_out = Ipv4Packet::serialize(
                                ip_pkt.header.dst_ip,
                                ip_pkt.header.src_ip,
                                IP_PROTO_ICMP,
                                ip_id,
                                64,
                                &echo_reply,
                            );
                            let eth_out = EthernetFrame::serialize(
                                eth.src_mac,
                                ingress_iface.mac,
                                ETHERTYPE_IPV4,
                                &ip_out,
                            );
                            out_transmissions.push((ingress_link.to_string(), eth_out));
                        }
                    } else {
                        // Forwarding data plane path
                        // 1. Check TTL
                        if ip_pkt.header.ttl <= 1 {
                            // TTL expired in transit -> Generate ICMP Time Exceeded (Type 11 Code 0)
                            let time_exceeded_payload =
                                IcmpPacket::build_time_exceeded(0, eth.payload);
                            let ip_id = self.next_ip_id();
                            let ip_out = Ipv4Packet::serialize(
                                ingress_iface.ip,
                                ip_pkt.header.src_ip,
                                IP_PROTO_ICMP,
                                ip_id,
                                64,
                                &time_exceeded_payload,
                            );
                            let eth_out = EthernetFrame::serialize(
                                eth.src_mac,
                                ingress_iface.mac,
                                ETHERTYPE_IPV4,
                                &ip_out,
                            );
                            out_transmissions.push((ingress_link.to_string(), eth_out));
                            return out_transmissions;
                        }

                        // Check MPLS Ingress Push route
                        if let Some(&(push_label, ref egress_name)) =
                            self.mpls_push_routes.get(&ip_pkt.header.dst_ip)
                            && let Some(egress_iface) =
                                self.interfaces.iter().find(|i| i.name == *egress_name)
                        {
                            let mpls_hdr = MplsHeader::new(push_label, 0, true, 64);
                            let mpls_pkt = MplsPacket::new(vec![mpls_hdr], eth.payload.to_vec());
                            let mpls_bytes = mpls_pkt.serialize();
                            let eth_out = EthernetFrame::serialize(
                                MacAddress::BROADCAST,
                                egress_iface.mac,
                                ETHERTYPE_MPLS,
                                &mpls_bytes,
                            );
                            out_transmissions.push((egress_iface.link_name.clone(), eth_out));
                            return out_transmissions;
                        }

                        // 2. Decrement TTL and recompute checksum
                        let new_ttl = ip_pkt.header.ttl - 1;

                        // 3. Routing Table Lookup (LPM)
                        if let Some(route) = self.routing_table.lookup(ip_pkt.header.dst_ip) {
                            let egress_iface_name = route.interface.clone();
                            let next_hop = route.next_hop(ip_pkt.header.dst_ip);

                            if let Some(egress_iface) =
                                self.interfaces.iter().find(|i| i.name == egress_iface_name)
                            {
                                let egress_link = egress_iface.link_name.clone();
                                let ip_id = ip_pkt.header.identification;
                                let mut forwarded_ip_bytes = Ipv4Packet::serialize(
                                    ip_pkt.header.src_ip,
                                    ip_pkt.header.dst_ip,
                                    ip_pkt.header.protocol.to_u8(),
                                    ip_id,
                                    new_ttl,
                                    ip_pkt.payload,
                                );

                                // Check if Outbound NAT (SNAT) applies for LAN -> WAN
                                if let Some(ref mut nat) = self.nat_table
                                    && self.nat_lan_iface.as_deref() == Some(&ingress_iface.name)
                                    && self.nat_wan_iface.as_deref() == Some(&egress_iface.name)
                                {
                                    nat.translate_outbound(&mut forwarded_ip_bytes);
                                }

                                let egress_arp = self
                                    .arp_tables
                                    .entry(egress_iface.name.clone())
                                    .or_default();
                                if let Some(dst_mac) = egress_arp.lookup(&next_hop.0) {
                                    let eth_out = EthernetFrame::serialize(
                                        dst_mac,
                                        egress_iface.mac,
                                        ETHERTYPE_IPV4,
                                        &forwarded_ip_bytes,
                                    );
                                    out_transmissions.push((egress_link, eth_out));
                                } else {
                                    // Queue transit packet and broadcast ARP Request on egress link
                                    let pending_key = (egress_iface.name.clone(), next_hop);
                                    self.pending_transit_packets
                                        .entry(pending_key)
                                        .or_default()
                                        .push(forwarded_ip_bytes);

                                    let arp_req = ArpPacket::build_request(
                                        egress_iface.mac,
                                        egress_iface.ip.0,
                                        next_hop.0,
                                    );
                                    let eth_arp = EthernetFrame::serialize(
                                        MacAddress::BROADCAST,
                                        egress_iface.mac,
                                        ETHERTYPE_ARP,
                                        &arp_req.serialize(),
                                    );
                                    out_transmissions.push((egress_link, eth_arp));
                                }
                            }
                        }
                    }
                }
            }

            _ => {}
        }

        out_transmissions
    }
}

/// Deterministic Virtual Network Lab orchestrator.
#[derive(Default)]
pub struct VirtualLab {
    pub links: HashMap<String, VirtualLink>,
    pub hosts: HashMap<String, LabHost>,
    pub routers: HashMap<String, LabRouter>,
    pub in_flight_frames: Vec<(String, String, Vec<u8>)>, // (sender_node, link_name, frame)
    pub total_steps_executed: usize,
    pub total_frames_delivered: usize,
}

impl VirtualLab {
    pub fn new() -> Self {
        VirtualLab {
            links: HashMap::new(),
            hosts: HashMap::new(),
            routers: HashMap::new(),
            in_flight_frames: Vec::new(),
            total_steps_executed: 0,
            total_frames_delivered: 0,
        }
    }

    pub fn add_link(&mut self, name: &str) {
        self.links.insert(name.to_string(), VirtualLink::new(name));
    }

    pub fn add_link_with_mtu(&mut self, name: &str, mtu: usize) {
        self.links
            .insert(name.to_string(), VirtualLink::new(name).with_mtu(mtu));
    }

    pub fn add_host(&mut self, name: &str, link_name: &str, config: NetStackConfig) {
        if !self.links.contains_key(link_name) {
            self.add_link(link_name);
        }
        let host = LabHost::new(name, link_name, config);
        self.hosts.insert(name.to_string(), host);
    }

    pub fn add_router(&mut self, router: LabRouter) {
        for iface in &router.interfaces {
            if !self.links.contains_key(&iface.link_name) {
                self.add_link(&iface.link_name);
            }
        }
        self.routers.insert(router.name.clone(), router);
    }

    pub fn host(&self, name: &str) -> Option<&LabHost> {
        self.hosts.get(name)
    }

    pub fn host_mut(&mut self, name: &str) -> Option<&mut LabHost> {
        self.hosts.get_mut(name)
    }

    pub fn router(&self, name: &str) -> Option<&LabRouter> {
        self.routers.get(name)
    }

    pub fn router_mut(&mut self, name: &str) -> Option<&mut LabRouter> {
        self.routers.get_mut(name)
    }

    pub fn link(&self, name: &str) -> Option<&VirtualLink> {
        self.links.get(name)
    }

    pub fn link_mut(&mut self, name: &str) -> Option<&mut VirtualLink> {
        self.links.get_mut(name)
    }

    pub fn enable_pcap(&mut self, link_name: &str) {
        if let Some(link) = self.links.get_mut(link_name) {
            link.enable_pcap();
        }
    }

    pub fn export_pcap(&mut self, link_name: &str) -> Option<Vec<u8>> {
        self.links
            .get_mut(link_name)
            .and_then(|l| l.take_pcap_bytes())
    }

    /// Queues a raw frame transmission originating from a host.
    pub fn send_from_host(&mut self, host_name: &str, frame: Vec<u8>) {
        if let Some(host) = self.hosts.get(host_name) {
            self.in_flight_frames
                .push((host_name.to_string(), host.link_name.clone(), frame));
        }
    }

    /// Executes one discrete simulation step:
    /// Drains current in-flight frames, passes each through the corresponding link fault model,
    /// delivers ready frames to connected hosts and routers on that link, and collects newly
    /// generated reply or transit frames into the in-flight queue.
    pub fn step(&mut self) -> usize {
        if self.in_flight_frames.is_empty() {
            return 0;
        }

        self.total_steps_executed += 1;
        let current_batch = std::mem::take(&mut self.in_flight_frames);
        let mut next_batch = Vec::new();
        let mut frames_processed = 0;

        for (sender, link_name, raw_frame) in current_batch {
            frames_processed += 1;

            // 1. Traverse Link (applies MTU, Drops, Corruption, PCAP Tap)
            let delivered_frame = match self.links.get_mut(&link_name) {
                Some(link) => match link.process_frame_transit(raw_frame) {
                    Some(f) => f,
                    None => continue, // Dropped by link!
                },
                None => raw_frame,
            };

            self.total_frames_delivered += 1;

            // 2. Deliver to all other Hosts on this link
            let host_names: Vec<String> = self.hosts.keys().cloned().collect();
            for h_name in host_names {
                if h_name == sender {
                    continue;
                }
                let host = self.hosts.get_mut(&h_name).unwrap();
                if host.link_name == link_name {
                    let replies = host.stack.process_frame(&delivered_frame);
                    for reply in replies {
                        next_batch.push((h_name.clone(), link_name.clone(), reply));
                    }
                }
            }

            // 3. Deliver to all Routers with an interface attached to this link
            let router_names: Vec<String> = self.routers.keys().cloned().collect();
            for r_name in router_names {
                if r_name == sender {
                    continue;
                }
                let router = self.routers.get_mut(&r_name).unwrap();
                let outgoing = router.process_incoming_frame(&link_name, &delivered_frame);
                for (egress_link, frame) in outgoing {
                    next_batch.push((r_name.clone(), egress_link, frame));
                }
            }
        }

        self.in_flight_frames = next_batch;
        frames_processed
    }

    /// Runs simulation steps until no frames remain in-flight or `max_steps` is reached.
    /// Returns the total number of steps executed.
    pub fn run_until_quiescent(&mut self, max_steps: usize) -> usize {
        let mut steps = 0;
        while !self.in_flight_frames.is_empty() && steps < max_steps {
            self.step();
            steps += 1;
        }
        steps
    }

    /// Triggers all RIPv2-enabled routers in the lab to generate and transmit periodic routing updates.
    pub fn broadcast_rip_advertisements(&mut self) {
        let router_names: Vec<String> = self.routers.keys().cloned().collect();
        for r_name in router_names {
            let router = self.routers.get(&r_name).unwrap();
            let updates = router.generate_rip_advertisements();
            for (link_name, frame) in updates {
                self.in_flight_frames
                    .push((r_name.clone(), link_name, frame));
            }
        }
    }
}
