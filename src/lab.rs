//! Integrated Deterministic Virtual Network Lab.
//!
//! Provides a complete in-process virtual networking testbed supporting:
//! - Multi-node, multi-subnet topologies with virtual links, switches, and routers
//! - Deterministic link fault injection (MTU limits, packet drops, byte corruption, packet reordering)
//! - Multi-interface IPv4 routing, TTL decrementing, and ICMP Time Exceeded generation
//! - Full dual-stack protocol operation (Ethernet, ARP, IPv4, IPv6, ICMP, ICMPv6, NDP, UDP, TCP)
//! - Integrated PCAP capture tap per link with Wireshark compatibility
//! - Discrete event stepping, simulated logical clock advancement, and run-to-quiescence simulation

use crate::arp::{ArpOpcode, ArpPacket, ArpTable};
use crate::bgp::Ipv4Prefix;
use crate::bgp_caps::AfiSafi;
use crate::bgp_evpn::RouteTarget;
use crate::bgp_router::{BgpPeerMode, BgpRouter};
use crate::ethernet::{
    ETHERTYPE_ARP, ETHERTYPE_IPV4, ETHERTYPE_MPLS, EtherType, EthernetFrame, MacAddress,
};
use crate::evpn::RouteDistinguisher;
use crate::evpn_vtep::{OverlayDecision, Vtep};
use crate::firewall::{Firewall, FirewallAction, FirewallChain};
use crate::icmp::{IcmpPacket, IcmpType};
use crate::ipv4::{IP_PROTO_ICMP, IP_PROTO_TCP, IP_PROTO_UDP, Ipv4Address, Ipv4Packet};
use crate::mpls::{LfibAction, LfibTable, MplsHeader, MplsPacket};
use crate::nat::NatTable;
use crate::ospf::OspfLsdb;
use crate::pcap::{LINKTYPE_ETHERNET, PcapWriter};
use crate::rip::{RIP_PORT, RipEngine, RipPacket};
use crate::router::{RouteSource, RoutingTable};
use crate::socket::SocketRuntime;
use crate::stack::{NetStack, NetStackConfig};
use crate::tcp::TcpSegment;
use crate::udp::UdpDatagram;
use crate::vxlan::{VXLAN_UDP_PORT, VxlanPacket};
use std::collections::{HashMap, HashSet};

/// Fault injection configuration and frame accounting for a virtual point-to-point or broadcast link.
#[derive(Debug)]
pub struct VirtualLink {
    pub name: String,
    pub mtu: usize,
    pub drop_packet_indices: Vec<usize>,
    pub corrupt_packet_indices: Vec<usize>,
    pub reorder_packet_indices: Vec<(usize, usize)>, // (hold_index, release_after_index)
    pub held_frames: Vec<(usize, Vec<u8>)>,
    pub total_packets_seen: usize,
    pub frames_forwarded: usize,
    pub frames_dropped: usize,
    pub frames_corrupted: usize,
    pub in_flight_frames: Vec<Vec<u8>>,
    pub pcap_writer: Option<PcapWriter<Vec<u8>>>,
    /// When set, every frame entering the link is discarded. Models a cable cut or a
    /// far-side outage, which is how a live protocol session is made to fail without
    /// anyone reaching into the protocol state.
    pub blackhole: bool,
}

impl VirtualLink {
    pub fn new(name: &str) -> Self {
        VirtualLink {
            name: name.to_string(),
            mtu: 1500,
            drop_packet_indices: Vec::new(),
            corrupt_packet_indices: Vec::new(),
            reorder_packet_indices: Vec::new(),
            held_frames: Vec::new(),
            total_packets_seen: 0,
            frames_forwarded: 0,
            frames_dropped: 0,
            frames_corrupted: 0,
            in_flight_frames: Vec::new(),
            pcap_writer: None,
            blackhole: false,
        }
    }

    /// Cuts (or restores) the link. A blackholed link silently drops everything, so
    /// peers on it stop hearing each other and their hold timers eventually expire.
    pub fn set_blackhole(&mut self, down: bool) {
        self.blackhole = down;
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

    /// Processes a frame attempting to cross this link, returning all delivered frames.
    pub fn process_frames_transit(&mut self, mut raw_frame: Vec<u8>) -> Vec<Vec<u8>> {
        self.total_packets_seen += 1;
        let pkt_index = self.total_packets_seen;
        let mut delivered = Vec::new();

        // Check hold/reorder rule: (hold_idx, release_after_idx)
        for &(hold_idx, release_after) in &self.reorder_packet_indices {
            if pkt_index == hold_idx {
                self.held_frames.push((release_after, raw_frame));
                return delivered; // Held for reordering
            }
        }

        // 0. A cut link swallows everything.
        if self.blackhole {
            self.frames_dropped += 1;
            return delivered;
        }

        // 1. Check MTU
        if raw_frame.len() > self.mtu + 14 {
            // Frame payload + Ethernet header exceeds link capacity
            self.frames_dropped += 1;
            return delivered;
        }

        // 2. Check deterministic drop rule
        if self.drop_packet_indices.contains(&pkt_index) {
            self.frames_dropped += 1;
            return delivered;
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
        delivered.push(raw_frame);

        // Check if any held frames should now be released
        let mut remaining_held = Vec::new();
        for (release_after, held_frame) in std::mem::take(&mut self.held_frames) {
            if pkt_index == release_after {
                if let Some(ref mut writer) = self.pcap_writer {
                    let ts_sec = (pkt_index as u32) / 10;
                    let ts_usec = ((pkt_index as u32) % 10) * 100_000 + 50_000;
                    let _ = writer.write_packet(ts_sec, ts_usec, &held_frame);
                }
                self.frames_forwarded += 1;
                delivered.push(held_frame);
            } else {
                remaining_held.push((release_after, held_frame));
            }
        }
        self.held_frames = remaining_held;

        delivered
    }

    /// Single frame transit helper for legacy compatibility.
    pub fn process_frame_transit(&mut self, raw_frame: Vec<u8>) -> Option<Vec<u8>> {
        self.process_frames_transit(raw_frame).into_iter().next()
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
    /// Transport endpoint table for traffic the router itself terminates. Present only
    /// once a control-plane protocol that needs sockets (currently BGP) is enabled.
    pub sockets: Option<SocketRuntime>,
    /// The BGP-4 speaker running on this router, if configured.
    pub bgp: Option<BgpRouter>,
    /// The VXLAN tunnel endpoint driven by MP-BGP EVPN, if this router is a leaf.
    /// Distinct from `vxlan_tunnels`, which is the older statically configured
    /// point-to-point overlay and stays available for topologies that use it.
    pub vtep: Option<Vtep>,
    /// Simulated clock, advanced by the lab.
    pub current_time_ms: u64,
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
            sockets: None,
            bgp: None,
            vtep: None,
            current_time_ms: 0,
        }
    }

    /// Gives this router a transport endpoint table so it can terminate TCP and UDP
    /// addressed to its own interfaces, not merely forward through them.
    pub fn enable_sockets(&mut self) -> &mut SocketRuntime {
        if self.sockets.is_none() {
            let default_ip = self
                .interfaces
                .first()
                .map(|i| i.ip)
                .unwrap_or(Ipv4Address::UNSPECIFIED);
            self.sockets = Some(SocketRuntime::new(default_ip));
        }
        self.sockets.as_mut().unwrap()
    }

    /// Starts a BGP-4 speaker on this router. It listens on TCP port 179 across every
    /// interface and installs its selected routes into this router's real routing table.
    pub fn enable_bgp(&mut self, local_as: u32, router_id: Ipv4Address) -> &mut BgpRouter {
        self.enable_sockets();
        self.bgp = Some(BgpRouter::new(local_as, router_id));
        self.bgp.as_mut().unwrap()
    }

    /// Configures a BGP neighbour reachable through `local_addr`, one of this router's
    /// own interface addresses.
    pub fn add_bgp_peer(
        &mut self,
        peer_ip: Ipv4Address,
        peer_as: u32,
        local_addr: Ipv4Address,
        mode: BgpPeerMode,
    ) {
        if let Some(ref mut bgp) = self.bgp {
            bgp.add_peer(peer_ip, peer_as, local_addr, mode);
        }
    }

    /// Originates a prefix into BGP from this router. The advertised next hop defaults
    /// to the interface address inside the prefix, falling back to the first interface.
    pub fn originate_bgp_prefix(&mut self, prefix: Ipv4Prefix) {
        let next_hop = self
            .interfaces
            .iter()
            .find(|i| prefix.contains(i.ip))
            .map(|i| i.ip)
            .or_else(|| self.interfaces.first().map(|i| i.ip))
            .unwrap_or(Ipv4Address::UNSPECIFIED);
        if let Some(ref mut bgp) = self.bgp {
            bgp.originate(prefix, next_hop);
        }
    }

    /// Administratively shuts a BGP neighbour down: NOTIFICATION, TCP teardown, and
    /// removal of every route learned from it.
    pub fn bgp_shutdown_peer(&mut self, peer_ip: Ipv4Address) {
        let now = self.current_time_ms;
        if let (Some(bgp), Some(sockets)) = (self.bgp.as_mut(), self.sockets.as_mut()) {
            bgp.shutdown_peer(peer_ip, now, sockets);
        }
    }

    /// Re-enables a neighbour that was administratively shut down.
    pub fn bgp_enable_peer(&mut self, peer_ip: Ipv4Address) {
        if let Some(ref mut bgp) = self.bgp {
            bgp.enable_peer(peer_ip);
        }
    }

    /// Stops originating a prefix, which propagates as a withdrawal.
    pub fn withdraw_bgp_prefix(&mut self, prefix: Ipv4Prefix) -> bool {
        self.bgp
            .as_mut()
            .map(|b| b.withdraw_originated(prefix))
            .unwrap_or(false)
    }

    pub fn bgp(&self) -> Option<&BgpRouter> {
        self.bgp.as_ref()
    }

    pub fn bgp_mut(&mut self) -> Option<&mut BgpRouter> {
        self.bgp.as_mut()
    }

    // ========================================================================
    // EVPN / VXLAN tunnel endpoint
    // ========================================================================

    /// Turns this router into a VXLAN tunnel endpoint driven by MP-BGP EVPN.
    ///
    /// `source_ip` is the address every VXLAN packet is sent from and the next
    /// hop this leaf advertises in its own EVPN routes, so it has to be an
    /// address the other leaves can route to. Enabling a VTEP also puts L2VPN
    /// EVPN into the capability set the BGP speaker offers, because a leaf with
    /// no EVPN capability could never learn a remote MAC.
    pub fn enable_vtep(&mut self, source_ip: Ipv4Address, underlay_iface: &str) -> &mut Vtep {
        self.vtep = Some(Vtep::new(source_ip, underlay_iface));
        if let Some(bgp) = self.bgp.as_mut() {
            bgp.enable_family(AfiSafi::L2VPN_EVPN);
        }
        self.vtep.as_mut().unwrap()
    }

    /// Configures a tenant EVPN instance on this VTEP.
    ///
    /// The import Route Targets are registered with the BGP speaker at the same
    /// time. That is what makes the Adj-RIB-In filter and the instance agree:
    /// a route no instance here asked for is dropped before it is even stored.
    pub fn add_evpn_instance(
        &mut self,
        vni: u32,
        rd: RouteDistinguisher,
        import_rts: &[RouteTarget],
        export_rts: &[RouteTarget],
    ) -> bool {
        let added = match self.vtep.as_mut() {
            Some(vtep) => vtep.add_instance(vni, rd, import_rts, export_rts),
            None => false,
        };
        // Only register the import targets if the instance actually exists.
        // Importing on behalf of an instance that was refused would fill the
        // Adj-RIB-In with routes nothing could ever program.
        if added && let Some(bgp) = self.bgp.as_mut() {
            for rt in import_rts {
                bgp.add_import_route_target(*rt);
            }
        }
        added
    }

    /// Puts one of this router's interfaces into a tenant instance as an access
    /// port. Frames arriving there are tenant traffic, not underlay traffic.
    pub fn attach_evpn_access_port(&mut self, vni: u32, iface: &str) {
        if let Some(vtep) = self.vtep.as_mut() {
            vtep.attach_access_port(vni, iface);
        }
    }

    pub fn vtep(&self) -> Option<&Vtep> {
        self.vtep.as_ref()
    }

    pub fn vtep_mut(&mut self) -> Option<&mut Vtep> {
        self.vtep.as_mut()
    }

    /// Pushes what the VTEP has learned locally into the BGP speaker as EVPN
    /// routes, and stops originating anything it no longer knows about.
    ///
    /// The originated set is made to equal the VTEP's view rather than being
    /// appended to, so a host that disappears withdraws itself.
    fn sync_evpn_origination(&mut self) {
        let Some(vtep) = self.vtep.as_ref() else {
            return;
        };
        let desired = vtep.routes_to_originate();
        let Some(bgp) = self.bgp.as_mut() else {
            return;
        };

        let desired_keys: HashSet<_> = desired.iter().map(|r| r.key()).collect();
        let stale: Vec<_> = bgp
            .evpn_originated_routes()
            .iter()
            .map(|r| r.key())
            .filter(|k| !desired_keys.contains(k))
            .collect();
        for key in stale {
            bgp.withdraw_evpn(&key);
        }
        for route in desired {
            bgp.originate_evpn(route);
        }
    }

    /// Rebuilds the VTEP's remote forwarding state from the EVPN Loc-RIB.
    ///
    /// The VTEP is moved out for the duration so the speaker can be borrowed
    /// immutably at the same time; cloning the Loc-RIB on every poll instead
    /// would make a steady state cost work proportional to its size.
    fn program_vtep_from_bgp(&mut self) {
        let Some(mut vtep) = self.vtep.take() else {
            return;
        };
        let withdraw = match self.bgp.as_ref() {
            Some(bgp) => vtep.program_from_rib(&bgp.evpn_loc_rib),
            None => Vec::new(),
        };
        self.vtep = Some(vtep);

        // A host that turned up behind another VTEP with a higher mobility
        // sequence is no longer ours to advertise.
        if !withdraw.is_empty()
            && let Some(bgp) = self.bgp.as_mut()
        {
            for key in withdraw {
                bgp.withdraw_evpn(&key);
            }
        }
    }

    /// Runs this router's control plane and transport timers at simulated time `now_ms`
    /// and returns `(egress_link, frame)` pairs for everything it wants to transmit.
    ///
    /// Order matters: the BGP speaker runs first so anything it decides to send is
    /// queued before the socket runtime drains its transmit path in the same step.
    pub fn step_timers(&mut self, now_ms: u64) -> Vec<(String, Vec<u8>)> {
        self.current_time_ms = now_ms;
        let mut out = Vec::new();
        if self.sockets.is_none() {
            return out;
        }

        // Local MAC learning becomes EVPN origination before the speaker runs, so
        // a host that appeared since the last step is advertised in this poll
        // rather than the next one.
        self.sync_evpn_origination();

        if let (Some(bgp), Some(sockets)) = (self.bgp.as_mut(), self.sockets.as_mut()) {
            bgp.poll(now_ms, sockets, &mut self.routing_table);
        }

        // ...and whatever the speaker decided is programmed into the data plane
        // immediately afterwards, so the overlay never lags the control plane by
        // a whole simulation step.
        self.program_vtep_from_bgp();

        let pending = match self.sockets.as_mut() {
            Some(s) => s.step_timers(now_ms),
            None => Vec::new(),
        };
        for tx in pending {
            let frames =
                self.emit_from_local_stack(tx.local.ip, tx.remote.ip, tx.protocol, &tx.payload);
            out.extend(frames);
        }
        out
    }

    /// Encapsulates a transport PDU this router originated in IPv4 and Ethernet,
    /// resolving the egress interface through its own routing table and ARP cache.
    /// Unresolved next hops queue the packet and emit an ARP request, exactly as the
    /// transit forwarding path does.
    fn emit_from_local_stack(
        &mut self,
        src_ip: Ipv4Address,
        dst_ip: Ipv4Address,
        protocol: u8,
        payload: &[u8],
    ) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        let Some(route) = self.routing_table.lookup(dst_ip).cloned() else {
            return out;
        };
        let Some(egress) = self
            .interfaces
            .iter()
            .find(|i| i.name == route.interface)
            .cloned()
        else {
            return out;
        };
        let next_hop = route.next_hop(dst_ip);
        let ip_id = self.next_ip_id();
        let ip_bytes = Ipv4Packet::serialize(src_ip, dst_ip, protocol, ip_id, 64, payload);

        let arp = self.arp_tables.entry(egress.name.clone()).or_default();
        if let Some(dst_mac) = arp.lookup(&next_hop.0) {
            out.push((
                egress.link_name.clone(),
                EthernetFrame::serialize(dst_mac, egress.mac, ETHERTYPE_IPV4, &ip_bytes),
            ));
        } else {
            self.pending_transit_packets
                .entry((egress.name.clone(), next_hop))
                .or_default()
                .push(ip_bytes);
            let arp_req = ArpPacket::build_request(egress.mac, egress.ip.0, next_hop.0);
            out.push((
                egress.link_name.clone(),
                EthernetFrame::serialize(
                    MacAddress::BROADCAST,
                    egress.mac,
                    ETHERTYPE_ARP,
                    &arp_req.serialize(),
                ),
            ));
        }
        out
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
        self.routing_table.add_route_from(
            subnet_net,
            subnet_mask,
            None,
            name,
            RouteSource::Connected,
        );
        self.interfaces.push(iface);
    }

    fn next_ip_id(&mut self) -> u16 {
        let id = self.ip_id_counter;
        self.ip_id_counter = self.ip_id_counter.wrapping_add(1);
        id
    }

    /// Processes an incoming frame arriving on a specific virtual link.
    /// Returns a list of `(egress_link_name, frame_bytes)` to transmit.
    /// Handles a tenant frame arriving on an EVPN access port.
    ///
    /// Two things happen, in this order and for different reasons. The source
    /// MAC is learned locally, which is what turns a host plugging in into an
    /// EVPN Type 2 advertisement. Then the *destination* is looked up in the
    /// state MP-BGP built, which is what decides whether the frame is bridged
    /// locally, encapsulated to exactly one remote VTEP, or replicated to the
    /// VTEPs that signalled participation with a Type 3 route.
    fn evpn_access_ingress(
        &mut self,
        ingress: &RouterInterface,
        raw_frame: &[u8],
    ) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        let Ok(eth) = EthernetFrame::parse(raw_frame) else {
            return out;
        };

        let host_ip = Self::tenant_source_ip(&eth);
        if let Some(vtep) = self.vtep.as_mut() {
            vtep.learn_local(&ingress.name, eth.src_mac, host_ip);
        }

        let decision = match self.vtep.as_ref() {
            Some(v) => v.forward(&ingress.name, eth.dst_mac),
            None => OverlayDecision::Drop,
        };

        match decision {
            OverlayDecision::Local { access_interface } => {
                if let Some(iface) = self
                    .interfaces
                    .iter()
                    .find(|i| i.name == access_interface)
                    .cloned()
                {
                    out.push((iface.link_name, raw_frame.to_vec()));
                }
            }
            OverlayDecision::Unicast { vni, vtep } => {
                self.encapsulate_vxlan(vni, vtep, raw_frame, &mut out);
            }
            OverlayDecision::Flood { vni, vteps } => {
                // Ingress replication: one copy per participating VTEP, built
                // separately so each carries its own outer IP header.
                for remote in vteps {
                    self.encapsulate_vxlan(vni, remote, raw_frame, &mut out);
                }
            }
            OverlayDecision::Drop => {}
        }
        out
    }

    /// The tenant host address a frame reveals, used to fill in the IP field of
    /// an EVPN Type 2 route. An ARP sender address counts, because that is the
    /// first thing a host says and often the only thing it says about itself.
    fn tenant_source_ip(eth: &EthernetFrame<'_>) -> Option<Ipv4Address> {
        match eth.ethertype {
            EtherType::IPv4 => Ipv4Packet::parse(eth.payload, false)
                .ok()
                .map(|p| p.header.src_ip),
            EtherType::Arp => ArpPacket::parse(eth.payload)
                .ok()
                .map(|a| Ipv4Address(a.sender_ip)),
            _ => None,
        }
    }

    /// Wraps a tenant frame in VXLAN / UDP 4789 / IPv4 and hands it to the real
    /// underlay forwarding path: routing table lookup, ARP resolution, and the
    /// same pending-packet queue every other locally originated packet uses.
    fn encapsulate_vxlan(
        &mut self,
        vni: u32,
        remote_vtep: Ipv4Address,
        inner_frame: &[u8],
        out: &mut Vec<(String, Vec<u8>)>,
    ) {
        let Some(source_ip) = self.vtep.as_ref().map(|v| v.source_ip) else {
            return;
        };
        let Ok(vxlan_bytes) = VxlanPacket::encapsulate(vni, inner_frame) else {
            return;
        };
        let udp_bytes = UdpDatagram::serialize(
            source_ip,
            remote_vtep,
            VXLAN_UDP_PORT,
            VXLAN_UDP_PORT,
            &vxlan_bytes,
        );
        let ip_id = self.next_ip_id();
        let ip_bytes =
            Ipv4Packet::serialize(source_ip, remote_vtep, IP_PROTO_UDP, ip_id, 64, &udp_bytes);

        let Some(route) = self.routing_table.lookup(remote_vtep).cloned() else {
            return;
        };
        let Some(egress) = self
            .interfaces
            .iter()
            .find(|i| i.name == route.interface)
            .cloned()
        else {
            return;
        };
        let next_hop = route.next_hop(remote_vtep);
        let arp = self.arp_tables.entry(egress.name.clone()).or_default();
        if let Some(dst_mac) = arp.lookup(&next_hop.0) {
            out.push((
                egress.link_name.clone(),
                EthernetFrame::serialize(dst_mac, egress.mac, ETHERTYPE_IPV4, &ip_bytes),
            ));
        } else {
            self.pending_transit_packets
                .entry((egress.name.clone(), next_hop))
                .or_default()
                .push(ip_bytes);
            let arp_req = ArpPacket::build_request(egress.mac, egress.ip.0, next_hop.0);
            out.push((
                egress.link_name.clone(),
                EthernetFrame::serialize(
                    MacAddress::BROADCAST,
                    egress.mac,
                    ETHERTYPE_ARP,
                    &arp_req.serialize(),
                ),
            ));
        }
    }

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

        // An EVPN access port carries tenant traffic, and where it goes is
        // decided by what MP-BGP taught this VTEP - never by a configured
        // destination. This is checked before the older static tunnel below, so
        // a router with both configured uses the control-plane path.
        if self
            .vtep
            .as_ref()
            .is_some_and(|v| v.vni_for_access(&ingress_iface.name).is_some())
        {
            return self.evpn_access_ingress(&ingress_iface, raw_frame);
        }

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

                        // VXLAN Decapsulation (UDP 4789), EVPN-driven.
                        //
                        // Nothing is learned from the inner frame. In EVPN the
                        // only way a MAC becomes reachable is a Type 2 route, so
                        // data-plane learning here would quietly reintroduce the
                        // flood-and-learn behaviour the control plane replaces -
                        // and would install state no withdrawal could remove.
                        if udp.dst_port == VXLAN_UDP_PORT
                            && is_for_router
                            && let Ok(vxlan) = VxlanPacket::parse(udp.payload)
                            && self
                                .vtep
                                .as_ref()
                                .is_some_and(|v| v.has_vni(vxlan.header.vni))
                        {
                            let inner_dst = EthernetFrame::parse(&vxlan.inner_frame)
                                .map(|f| f.dst_mac)
                                .unwrap_or(MacAddress::BROADCAST);
                            let ports = self
                                .vtep
                                .as_ref()
                                .map(|v| v.access_ports_for(vxlan.header.vni, inner_dst))
                                .unwrap_or_default();
                            for port in ports {
                                if let Some(access) =
                                    self.interfaces.iter().find(|i| i.name == port)
                                {
                                    out_transmissions.push((
                                        access.link_name.clone(),
                                        vxlan.inner_frame.clone(),
                                    ));
                                }
                            }
                            return out_transmissions;
                        }

                        // VXLAN Decapsulation for the older statically configured
                        // point-to-point overlay.
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

                        // TCP addressed to one of our own interfaces: hand it to the
                        // socket runtime, which owns port 179 when BGP is enabled and
                        // answers anything else with a RST.
                        if ip_pkt.header.protocol == crate::ipv4::IpProtocol::Tcp
                            && self.sockets.is_some()
                        {
                            let src_ip = ip_pkt.header.src_ip;
                            let dst_ip = ip_pkt.header.dst_ip;
                            let now = self.current_time_ms;
                            let responses =
                                match TcpSegment::parse(src_ip, dst_ip, ip_pkt.payload, true) {
                                    Ok(seg) => self
                                        .sockets
                                        .as_mut()
                                        .map(|s| s.dispatch_tcp_segment(src_ip, dst_ip, &seg, now))
                                        .unwrap_or_default(),
                                    Err(_) => Vec::new(),
                                };
                            for resp in responses {
                                let frames =
                                    self.emit_from_local_stack(dst_ip, src_ip, IP_PROTO_TCP, &resp);
                                out_transmissions.extend(frames);
                            }
                            return out_transmissions;
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

/// Builds the canned three-autonomous-system BGP fabric the shell diagnostics run on:
///
/// ```text
/// host_a 10.1.0.2 - r1 (AS65001) - r2 (AS65002) - r3 (AS65003) - host_c 10.3.0.2
/// ```
///
/// R1 originates 10.1.0.0/24 and R3 originates 10.3.0.0/24. Nothing else is configured,
/// so every route the routers end up with was learned over a real BGP session on TCP
/// port 179 and installed by the decision process.
pub fn build_bgp_demo_fabric() -> VirtualLab {
    fn mac(a: u8, b: u8) -> MacAddress {
        MacAddress([0x02, 0x00, 0x00, 0x00, a, b])
    }
    let addr = Ipv4Address::new;

    let mut lab = VirtualLab::new();
    for link in ["lan1", "r1r2", "r2r3", "lan3"] {
        lab.add_link(link);
    }

    lab.add_host(
        "host_a",
        "lan1",
        NetStackConfig {
            mac: mac(0x0A, 0x02),
            ip: addr(10, 1, 0, 2),
            ipv6: None,
            subnet_mask: 24,
            gateway: Some(addr(10, 1, 0, 1)),
        },
    );
    lab.add_host(
        "host_c",
        "lan3",
        NetStackConfig {
            mac: mac(0x0C, 0x02),
            ip: addr(10, 3, 0, 2),
            ipv6: None,
            subnet_mask: 24,
            gateway: Some(addr(10, 3, 0, 1)),
        },
    );

    let mut r1 = LabRouter::new("r1");
    r1.add_interface("eth0", mac(0x01, 0x00), addr(10, 1, 0, 1), 24, "lan1");
    r1.add_interface("eth1", mac(0x01, 0x01), addr(10, 12, 0, 1), 30, "r1r2");
    r1.enable_bgp(65001, addr(1, 1, 1, 1)).set_hold_time(9);
    r1.add_bgp_peer(
        addr(10, 12, 0, 2),
        65002,
        addr(10, 12, 0, 1),
        BgpPeerMode::Active,
    );
    r1.originate_bgp_prefix(Ipv4Prefix::new(addr(10, 1, 0, 0), 24));

    let mut r2 = LabRouter::new("r2");
    r2.add_interface("eth0", mac(0x02, 0x00), addr(10, 12, 0, 2), 30, "r1r2");
    r2.add_interface("eth1", mac(0x02, 0x01), addr(10, 23, 0, 2), 30, "r2r3");
    r2.enable_bgp(65002, addr(2, 2, 2, 2)).set_hold_time(9);
    r2.add_bgp_peer(
        addr(10, 12, 0, 1),
        65001,
        addr(10, 12, 0, 2),
        BgpPeerMode::Passive,
    );
    r2.add_bgp_peer(
        addr(10, 23, 0, 3),
        65003,
        addr(10, 23, 0, 2),
        BgpPeerMode::Active,
    );

    let mut r3 = LabRouter::new("r3");
    r3.add_interface("eth0", mac(0x03, 0x00), addr(10, 23, 0, 3), 30, "r2r3");
    r3.add_interface("eth1", mac(0x03, 0x01), addr(10, 3, 0, 1), 24, "lan3");
    r3.enable_bgp(65003, addr(3, 3, 3, 3)).set_hold_time(9);
    r3.add_bgp_peer(
        addr(10, 23, 0, 2),
        65002,
        addr(10, 23, 0, 3),
        BgpPeerMode::Passive,
    );
    r3.originate_bgp_prefix(Ipv4Prefix::new(addr(10, 3, 0, 0), 24));

    lab.add_router(r1);
    lab.add_router(r2);
    lab.add_router(r3);
    lab
}

/// Route Target for a VNI in the usual `AS:VNI` form.
pub fn evpn_rt(asn: u16, vni: u32) -> RouteTarget {
    RouteTarget::as2(asn, vni)
}

/// Builds the leaf-spine-leaf EVPN/VXLAN fabric:
///
/// ```text
///  host_a 192.168.10.11            host_b 192.168.10.22
///  MAC 02:..:0A                    MAC 02:..:0B
///        |  tenant1                      |  tenant2
///      leaf1                           leaf2
///      VTEP 10.0.0.1                   VTEP 10.0.0.2
///        \  10.1.0.1/30      10.2.0.2/30  /
///         \-------- spine (IP underlay) -/
///                 10.1.0.2      10.2.0.1
/// ```
///
/// The two tenant hosts sit in one /24 with no gateway: as far as they can tell
/// they share a wire, and every packet between them has to cross the overlay for
/// that to be true.
///
/// The spine forwards IP and nothing else - it runs no BGP and knows no VNI. The
/// leaves peer directly, loopback to loopback, so the TCP session carrying the
/// EVPN routes is itself multihop traffic the spine forwards. Nothing about the
/// overlay is configured on either leaf beyond its own instance: no remote MAC,
/// no remote VTEP, no tunnel destination. Every one of those has to arrive as an
/// EVPN route or the fabric does not work at all.
///
/// `leaf_as` gives the two leaves their ASNs, so a caller can run the same fabric
/// on 16-bit or 32-bit autonomous system numbers.
pub fn build_evpn_fabric(leaf1_as: u32, leaf2_as: u32) -> VirtualLab {
    fn mac(a: u8, b: u8) -> MacAddress {
        MacAddress([0x02, 0x00, 0x00, 0x00, a, b])
    }
    let addr = Ipv4Address::new;

    const VNI: u32 = 5001;
    let vtep1 = addr(10, 0, 0, 1);
    let vtep2 = addr(10, 0, 0, 2);

    let mut lab = VirtualLab::new();
    for link in [
        "tenant1",
        "tenant2",
        "leaf1spine",
        "leaf2spine",
        "lo1",
        "lo2",
    ] {
        lab.add_link(link);
    }

    // Tenant hosts: same subnet, no gateway. Anything that reaches the far side
    // did so as a bridged Ethernet frame, not as a routed packet.
    lab.add_host(
        "host_a",
        "tenant1",
        NetStackConfig {
            mac: mac(0x0A, 0x0A),
            ip: addr(192, 168, 10, 11),
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        },
    );
    lab.add_host(
        "host_b",
        "tenant2",
        NetStackConfig {
            mac: mac(0x0B, 0x0B),
            ip: addr(192, 168, 10, 22),
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        },
    );

    let mut leaf1 = LabRouter::new("leaf1");
    leaf1.add_interface(
        "eth0",
        mac(0x01, 0x00),
        addr(192, 168, 10, 1),
        24,
        "tenant1",
    );
    leaf1.add_interface("eth1", mac(0x01, 0x01), addr(10, 1, 0, 1), 30, "leaf1spine");
    // The VTEP address lives on a loopback, exactly as it would on a real leaf:
    // it must stay up when any one underlay link does not.
    leaf1.add_interface("lo0", mac(0x01, 0xFF), vtep1, 32, "lo1");
    leaf1.routing_table.add_route_from(
        vtep2,
        32,
        Some(addr(10, 1, 0, 2)),
        "eth1",
        RouteSource::Static,
    );
    leaf1
        .enable_bgp(leaf1_as, addr(1, 1, 1, 1))
        .set_hold_time(9);
    leaf1.add_bgp_peer(vtep2, leaf2_as, vtep1, BgpPeerMode::Active);
    leaf1.enable_vtep(vtep1, "eth1");
    leaf1.add_evpn_instance(
        VNI,
        RouteDistinguisher::new(vtep1, VNI as u16),
        &[evpn_rt(65001, VNI)],
        &[evpn_rt(65001, VNI)],
    );
    leaf1.attach_evpn_access_port(VNI, "eth0");

    let mut spine = LabRouter::new("spine");
    spine.add_interface("eth0", mac(0x02, 0x00), addr(10, 1, 0, 2), 30, "leaf1spine");
    spine.add_interface("eth1", mac(0x02, 0x01), addr(10, 2, 0, 1), 30, "leaf2spine");
    spine.routing_table.add_route_from(
        vtep1,
        32,
        Some(addr(10, 1, 0, 1)),
        "eth0",
        RouteSource::Static,
    );
    spine.routing_table.add_route_from(
        vtep2,
        32,
        Some(addr(10, 2, 0, 2)),
        "eth1",
        RouteSource::Static,
    );

    let mut leaf2 = LabRouter::new("leaf2");
    leaf2.add_interface(
        "eth0",
        mac(0x03, 0x00),
        addr(192, 168, 10, 2),
        24,
        "tenant2",
    );
    leaf2.add_interface("eth1", mac(0x03, 0x01), addr(10, 2, 0, 2), 30, "leaf2spine");
    leaf2.add_interface("lo0", mac(0x03, 0xFF), vtep2, 32, "lo2");
    leaf2.routing_table.add_route_from(
        vtep1,
        32,
        Some(addr(10, 2, 0, 1)),
        "eth1",
        RouteSource::Static,
    );
    leaf2
        .enable_bgp(leaf2_as, addr(3, 3, 3, 3))
        .set_hold_time(9);
    leaf2.add_bgp_peer(vtep1, leaf1_as, vtep2, BgpPeerMode::Passive);
    leaf2.enable_vtep(vtep2, "eth1");
    leaf2.add_evpn_instance(
        VNI,
        RouteDistinguisher::new(vtep2, VNI as u16),
        &[evpn_rt(65001, VNI)],
        &[evpn_rt(65001, VNI)],
    );
    leaf2.attach_evpn_access_port(VNI, "eth0");

    lab.add_router(leaf1);
    lab.add_router(spine);
    lab.add_router(leaf2);
    lab
}

/// Drives `lab` until every configured BGP session is ESTABLISHED and every VTEP
/// has been told about at least one remote MAC, or the simulated deadline passes.
///
/// The second half of that condition is the point: a session that is up but has
/// exchanged no EVPN route has not converged the overlay.
pub fn converge_evpn(lab: &mut VirtualLab, max_sim_ms: u64) -> bool {
    lab.run_until(250, max_sim_ms, |l| {
        l.routers.values().all(|r| match (r.bgp(), r.vtep()) {
            (Some(b), Some(v)) => {
                b.peers().iter().all(|p| p.carries_evpn()) && v.remote_mac_count() > 0
            }
            _ => true,
        })
    })
}

/// Drives `lab` until every configured BGP session is ESTABLISHED and every speaker has
/// installed at least one learned route, or the simulated deadline passes. Purely
/// simulated time: no thread sleeps.
pub fn converge_bgp(lab: &mut VirtualLab, max_sim_ms: u64) -> bool {
    lab.run_until(250, max_sim_ms, |l| {
        l.routers.values().all(|r| match r.bgp() {
            Some(b) => {
                b.peers()
                    .iter()
                    .all(|p| p.state == crate::bgp_router::BgpState::Established)
                    && !b.loc_rib.is_empty()
            }
            None => true,
        })
    })
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
    pub current_time_ms: u64,
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
            current_time_ms: 0,
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

    /// Collects everything every host's socket runtime wants to transmit at the current
    /// simulated time and queues it on the owning link, without advancing the clock.
    ///
    /// This is what turns an application-level `tcp_write` into frames on the wire: the
    /// application never touches a packet, the lab pumps the stack instead.
    pub fn pump(&mut self) -> usize {
        let mut queued = 0;
        let mut host_names: Vec<String> = self.hosts.keys().cloned().collect();
        host_names.sort();
        for h_name in host_names {
            let host = self.hosts.get_mut(&h_name).unwrap();
            let link_name = host.link_name.clone();
            for f in host.stack.poll_transmit() {
                self.in_flight_frames
                    .push((h_name.clone(), link_name.clone(), f));
                queued += 1;
            }
        }
        queued += self.pump_routers();
        queued
    }

    /// Runs every router's control plane and socket runtime at the current simulated
    /// time and queues whatever they emit. Routers with no socket runtime produce
    /// nothing, so a topology without a routing process is unaffected.
    fn pump_routers(&mut self) -> usize {
        let mut queued = 0;
        let now = self.current_time_ms;
        let mut router_names: Vec<String> = self.routers.keys().cloned().collect();
        router_names.sort();
        for r_name in router_names {
            let router = self.routers.get_mut(&r_name).unwrap();
            if router.sockets.is_none() {
                continue;
            }
            for (link_name, frame) in router.step_timers(now) {
                self.in_flight_frames
                    .push((r_name.clone(), link_name, frame));
                queued += 1;
            }
        }
        queued
    }

    /// Advances simulated logical time by `ms` and runs every host's timers, queueing any
    /// resulting frames (retransmissions, deferred FINs, window probes, new data).
    pub fn advance_time(&mut self, ms: u64) -> usize {
        self.current_time_ms += ms;
        let mut queued = 0;
        let mut host_names: Vec<String> = self.hosts.keys().cloned().collect();
        host_names.sort();
        for h_name in host_names {
            let host = self.hosts.get_mut(&h_name).unwrap();
            let link_name = host.link_name.clone();
            let frames = host.stack.step_timers(self.current_time_ms);
            for f in frames {
                self.in_flight_frames
                    .push((h_name.clone(), link_name.clone(), f));
                queued += 1;
            }
        }
        // Routers run their BGP timers off the same logical clock, so a hold timer
        // expires because simulated time passed, never because a thread slept.
        queued += self.pump_routers();
        queued
    }

    /// Runs the network to quiescence at the current time, pumping host sockets between
    /// steps so application writes turn into frames without needing the clock to move.
    pub fn run_pumped(&mut self, max_rounds: usize) -> usize {
        let mut steps = 0;
        for _ in 0..max_rounds {
            let queued = self.pump();
            if queued == 0 && self.in_flight_frames.is_empty() {
                break;
            }
            steps += self.run_until_quiescent(200);
        }
        steps
    }

    /// Drives the simulation until `predicate` holds or the simulated deadline passes.
    ///
    /// Each round pumps every host, runs the network to quiescence, then advances the
    /// clock by `tick_ms` so retransmission timers can fire. Returns true if the
    /// predicate was satisfied. Purely simulated time: no thread ever sleeps.
    pub fn run_until<F>(&mut self, tick_ms: u64, max_sim_ms: u64, mut predicate: F) -> bool
    where
        F: FnMut(&VirtualLab) -> bool,
    {
        let deadline = self.current_time_ms + max_sim_ms;
        self.run_pumped(50);
        if predicate(self) {
            return true;
        }
        while self.current_time_ms < deadline {
            self.advance_time(tick_ms.max(1));
            self.run_pumped(50);
            if predicate(self) {
                return true;
            }
        }
        false
    }

    /// Executes one discrete simulation step:
    /// Drains current in-flight frames, passes each through the corresponding link fault model,
    /// delivers ready frames to connected hosts and routers on that link, and collects newly
    /// generated reply or transit frames into the in-flight queue.
    pub fn step(&mut self) -> usize {
        // Give every host's socket runtime a chance to emit before draining the wire, so a
        // plain `step()` loop carries application data without an explicit pump.
        self.pump();

        if self.in_flight_frames.is_empty() {
            return 0;
        }

        self.total_steps_executed += 1;
        let current_batch = std::mem::take(&mut self.in_flight_frames);
        let mut next_batch = Vec::new();
        let mut frames_processed = 0;

        for (sender, link_name, raw_frame) in current_batch {
            frames_processed += 1;

            // 1. Traverse Link (applies MTU, Drops, Corruption, Reordering, PCAP Tap)
            let delivered_frames = match self.links.get_mut(&link_name) {
                Some(link) => link.process_frames_transit(raw_frame),
                None => vec![raw_frame],
            };

            for delivered_frame in delivered_frames {
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

    /// Advances simulated time in discrete ticks of `step_dt_ms` up to `max_sim_ms`,
    /// running each step to quiescence until the network is idle.
    pub fn run_until_idle_or_timeout(&mut self, max_sim_ms: u64, step_dt_ms: u64) -> u64 {
        let start = self.current_time_ms;
        let limit = start + max_sim_ms;
        while self.current_time_ms < limit {
            self.run_until_quiescent(50);
            let queued = self.advance_time(step_dt_ms);
            self.run_until_quiescent(50);
            if queued == 0 && self.in_flight_frames.is_empty() {
                // If quiescent and no new timer events triggered, jump ahead or complete
                break;
            }
        }
        self.current_time_ms - start
    }
}
