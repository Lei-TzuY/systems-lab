//! Unified NetStack: Dual-Stack IPv4/IPv6 Layer 2 -> Layer 3 -> Layer 4 packet processing pipeline.

use crate::arp::{ArpOpcode, ArpPacket, ArpTable};
use crate::ethernet::{
    ETHERTYPE_ARP, ETHERTYPE_IPV4, ETHERTYPE_IPV6, EtherType, EthernetFrame, MacAddress,
};
use crate::firewall::{Firewall, FirewallAction, FirewallChain};
use crate::icmp::{IcmpPacket, IcmpType};
use crate::icmpv6::{
    ICMPV6_TYPE_ECHO_REPLY, ICMPV6_TYPE_ECHO_REQUEST, ICMPV6_TYPE_NEIGHBOR_ADVERT,
    ICMPV6_TYPE_NEIGHBOR_SOLICIT, Icmpv6Packet, NdpTable,
};
use crate::ipv4::{IP_PROTO_ICMP, IP_PROTO_TCP, IP_PROTO_UDP, IpProtocol, Ipv4Address, Ipv4Packet};
use crate::ipv6::{Ipv6Address, Ipv6Packet, NEXT_HEADER_ICMPV6};
use crate::nat::NatTable;
use crate::router::RoutingTable;
use crate::tcp::{TcpManager, TcpSegment};
use crate::udp::{UdpDatagram, UdpSocketTable};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct NetStackConfig {
    pub mac: MacAddress,
    pub ip: Ipv4Address,
    pub ipv6: Option<Ipv6Address>,
    pub subnet_mask: u8,
    pub gateway: Option<Ipv4Address>,
}

pub struct NetStack {
    pub config: NetStackConfig,
    pub arp_table: ArpTable,
    pub ndp_table: NdpTable,
    pub routing_table: RoutingTable,
    pub firewall: Firewall,
    pub nat: Option<NatTable>,
    pub udp_sockets: UdpSocketTable,
    pub tcp_manager: TcpManager,
    pub ip_id_counter: u16,
    pub pending_arp_packets: HashMap<Ipv4Address, Vec<Vec<u8>>>,
    pub pending_ndp_packets: HashMap<Ipv6Address, Vec<Vec<u8>>>,
    pub received_icmp_replies: Vec<(Ipv4Address, u16, u16)>,
    pub received_icmp_time_exceeded: Vec<(Ipv4Address, u8)>,
    pub received_icmp_unreachable: Vec<(Ipv4Address, u8)>,
    pub received_icmpv6_replies: Vec<(Ipv6Address, u16, u16)>,
    pub received_udp_payloads: Vec<(Ipv4Address, u16, u16, Vec<u8>)>,
}

impl NetStack {
    pub fn new(config: NetStackConfig) -> Self {
        let mut routing_table = RoutingTable::new();

        // Local subnet route
        let subnet_net = config.ip.mask(config.subnet_mask);
        routing_table.add_route(subnet_net, config.subnet_mask, None, "eth0");

        // Default gateway route
        if let Some(gw) = config.gateway {
            routing_table.add_route(Ipv4Address::UNSPECIFIED, 0, Some(gw), "eth0");
        }

        NetStack {
            config,
            arp_table: ArpTable::new(),
            ndp_table: NdpTable::new(),
            routing_table,
            firewall: Firewall::new(),
            nat: None,
            udp_sockets: UdpSocketTable::new(),
            tcp_manager: TcpManager::new(),
            ip_id_counter: 1,
            pending_arp_packets: HashMap::new(),
            pending_ndp_packets: HashMap::new(),
            received_icmp_replies: Vec::new(),
            received_icmp_time_exceeded: Vec::new(),
            received_icmp_unreachable: Vec::new(),
            received_icmpv6_replies: Vec::new(),
            received_udp_payloads: Vec::new(),
        }
    }

    pub fn enable_nat(&mut self, public_ip: Ipv4Address) {
        self.nat = Some(NatTable::new(public_ip));
    }

    pub fn next_ip_id(&mut self) -> u16 {
        let id = self.ip_id_counter;
        self.ip_id_counter = self.ip_id_counter.wrapping_add(1);
        id
    }

    pub fn send_ip_packet(&mut self, dst_ip: Ipv4Address, ip_bytes: Vec<u8>) -> Option<Vec<u8>> {
        let next_hop = if let Some(route) = self.routing_table.lookup(dst_ip) {
            route.next_hop(dst_ip)
        } else {
            dst_ip
        };

        if let Some(dst_mac) = self.arp_table.lookup(&next_hop.0) {
            Some(EthernetFrame::serialize(
                dst_mac,
                self.config.mac,
                ETHERTYPE_IPV4,
                &ip_bytes,
            ))
        } else {
            self.pending_arp_packets
                .entry(next_hop)
                .or_default()
                .push(ip_bytes);
            let arp_req = ArpPacket::build_request(self.config.mac, self.config.ip.0, next_hop.0);
            Some(EthernetFrame::serialize(
                MacAddress::BROADCAST,
                self.config.mac,
                ETHERTYPE_ARP,
                &arp_req.serialize(),
            ))
        }
    }

    pub fn send_ip6_packet(&mut self, dst_ip: Ipv6Address, ip6_bytes: Vec<u8>) -> Option<Vec<u8>> {
        if let Some(dst_mac) = self.ndp_table.lookup(&dst_ip) {
            Some(EthernetFrame::serialize(
                dst_mac,
                self.config.mac,
                ETHERTYPE_IPV6,
                &ip6_bytes,
            ))
        } else {
            self.pending_ndp_packets
                .entry(dst_ip)
                .or_default()
                .push(ip6_bytes);
            let my_ip6 = self.config.ipv6.unwrap_or(Ipv6Address::LOOPBACK);
            let ns =
                Icmpv6Packet::build_neighbor_solicitation(my_ip6, dst_ip, dst_ip, self.config.mac);
            let ip6_ns = Ipv6Packet::serialize(my_ip6, dst_ip, NEXT_HEADER_ICMPV6, 255, &ns);
            Some(EthernetFrame::serialize(
                MacAddress::BROADCAST,
                self.config.mac,
                ETHERTYPE_IPV6,
                &ip6_ns,
            ))
        }
    }

    pub fn ping4(
        &mut self,
        dst_ip: Ipv4Address,
        id: u16,
        seq: u16,
        payload: &[u8],
    ) -> Option<Vec<u8>> {
        let icmp = IcmpPacket::build_echo_request(id, seq, payload);
        let ip_id = self.next_ip_id();
        let ip_bytes =
            Ipv4Packet::serialize(self.config.ip, dst_ip, IP_PROTO_ICMP, ip_id, 64, &icmp);
        self.send_ip_packet(dst_ip, ip_bytes)
    }

    pub fn ping6(
        &mut self,
        dst_ip: Ipv6Address,
        id: u16,
        seq: u16,
        payload: &[u8],
    ) -> Option<Vec<u8>> {
        let my_ip6 = self.config.ipv6.unwrap_or(Ipv6Address::LOOPBACK);
        let icmp = Icmpv6Packet::build_echo_request(my_ip6, dst_ip, id, seq, payload);
        let ip6_bytes = Ipv6Packet::serialize(my_ip6, dst_ip, NEXT_HEADER_ICMPV6, 64, &icmp);
        self.send_ip6_packet(dst_ip, ip6_bytes)
    }

    pub fn send_udp(
        &mut self,
        dst_ip: Ipv4Address,
        src_port: u16,
        dst_port: u16,
        payload: &[u8],
    ) -> Option<Vec<u8>> {
        let udp = UdpDatagram::serialize(self.config.ip, dst_ip, src_port, dst_port, payload);
        let ip_id = self.next_ip_id();
        let ip_bytes = Ipv4Packet::serialize(self.config.ip, dst_ip, IP_PROTO_UDP, ip_id, 64, &udp);
        self.send_ip_packet(dst_ip, ip_bytes)
    }

    pub fn tcp_connect(
        &mut self,
        dst_ip: Ipv4Address,
        src_port: u16,
        dst_port: u16,
        isn: u32,
    ) -> Option<Vec<u8>> {
        use crate::tcp::SocketAddrV4;
        let local = SocketAddrV4 {
            ip: self.config.ip,
            port: src_port,
        };
        let remote = SocketAddrV4 {
            ip: dst_ip,
            port: dst_port,
        };
        let tcp_seg_bytes = self.tcp_manager.connect(local, remote, isn);
        let ip_id = self.next_ip_id();
        let ip_bytes = Ipv4Packet::serialize(
            self.config.ip,
            dst_ip,
            IP_PROTO_TCP,
            ip_id,
            64,
            &tcp_seg_bytes,
        );
        self.send_ip_packet(dst_ip, ip_bytes)
    }

    pub fn tcp_send_data(
        &mut self,
        dst_ip: Ipv4Address,
        src_port: u16,
        dst_port: u16,
        data: &[u8],
    ) -> Option<Vec<u8>> {
        use crate::tcp::SocketAddrV4;
        let local = SocketAddrV4 {
            ip: self.config.ip,
            port: src_port,
        };
        let remote = SocketAddrV4 {
            ip: dst_ip,
            port: dst_port,
        };
        let tcp_seg_bytes = self.tcp_manager.send_data(local, remote, data)?;
        let ip_id = self.next_ip_id();
        let ip_bytes = Ipv4Packet::serialize(
            self.config.ip,
            dst_ip,
            IP_PROTO_TCP,
            ip_id,
            64,
            &tcp_seg_bytes,
        );
        self.send_ip_packet(dst_ip, ip_bytes)
    }

    pub fn tcp_close(
        &mut self,
        dst_ip: Ipv4Address,
        src_port: u16,
        dst_port: u16,
    ) -> Option<Vec<u8>> {
        use crate::tcp::SocketAddrV4;
        let local = SocketAddrV4 {
            ip: self.config.ip,
            port: src_port,
        };
        let remote = SocketAddrV4 {
            ip: dst_ip,
            port: dst_port,
        };
        let tcp_seg_bytes = self.tcp_manager.close(local, remote)?;
        let ip_id = self.next_ip_id();
        let ip_bytes = Ipv4Packet::serialize(
            self.config.ip,
            dst_ip,
            IP_PROTO_TCP,
            ip_id,
            64,
            &tcp_seg_bytes,
        );
        self.send_ip_packet(dst_ip, ip_bytes)
    }

    /// Primary entry point: process incoming raw Ethernet frame bytes,
    /// demultiplex through all protocol layers, and return any outgoing reply frames.
    pub fn process_frame(&mut self, raw_frame: &[u8]) -> Vec<Vec<u8>> {
        let mut out_frames = Vec::new();

        let eth = match EthernetFrame::parse(raw_frame) {
            Ok(f) => f,
            Err(_) => return out_frames,
        };

        // Filter packets: accept if destination is our MAC or Broadcast / Multicast
        if !eth.dst_mac.is_broadcast()
            && !eth.dst_mac.is_multicast()
            && eth.dst_mac != self.config.mac
        {
            return out_frames;
        }

        match eth.ethertype {
            EtherType::Arp => {
                if let Ok(arp) = ArpPacket::parse(eth.payload) {
                    // Update ARP cache with sender
                    self.arp_table.insert(arp.sender_ip, arp.sender_mac);
                    let sender_ipv4 = Ipv4Address(arp.sender_ip);

                    // Drain any pending IP packets waiting for this ARP resolution
                    if let Some(queued_packets) = self.pending_arp_packets.remove(&sender_ipv4) {
                        for ip_pkt in queued_packets {
                            let eth_out = EthernetFrame::serialize(
                                arp.sender_mac,
                                self.config.mac,
                                ETHERTYPE_IPV4,
                                &ip_pkt,
                            );
                            out_frames.push(eth_out);
                        }
                    }

                    if arp.opcode == ArpOpcode::Request && arp.target_ip == self.config.ip.0 {
                        // Generate ARP Reply
                        let reply = ArpPacket::build_reply(
                            self.config.mac,
                            self.config.ip.0,
                            arp.sender_mac,
                            arp.sender_ip,
                        );
                        let eth_out = EthernetFrame::serialize(
                            arp.sender_mac,
                            self.config.mac,
                            ETHERTYPE_ARP,
                            &reply.serialize(),
                        );
                        out_frames.push(eth_out);
                    }
                }
            }

            EtherType::IPv4 => {
                if let Ok(ip_pkt) = Ipv4Packet::parse(eth.payload, true) {
                    // 1. Evaluate packet against INPUT firewall chain
                    if self.firewall.evaluate(FirewallChain::Input, &ip_pkt)
                        != FirewallAction::Accept
                    {
                        return out_frames; // Dropped by firewall!
                    }

                    // Cache sender MAC for source IP
                    self.arp_table.insert(ip_pkt.header.src_ip.0, eth.src_mac);

                    // Verify destination IP
                    let dst = ip_pkt.header.dst_ip;
                    if dst != self.config.ip && !dst.is_broadcast() && dst != Ipv4Address::BROADCAST
                    {
                        return out_frames;
                    }

                    match ip_pkt.header.protocol {
                        IpProtocol::Icmp => {
                            if let Ok(icmp) = IcmpPacket::parse(ip_pkt.payload, true) {
                                match icmp.icmp_type {
                                    IcmpType::EchoRequest => {
                                        let echo_reply = IcmpPacket::build_echo_reply(&icmp);
                                        let ip_id = self.next_ip_id();
                                        let ip_out = Ipv4Packet::serialize(
                                            self.config.ip,
                                            ip_pkt.header.src_ip,
                                            IP_PROTO_ICMP,
                                            ip_id,
                                            64,
                                            &echo_reply,
                                        );
                                        let eth_out = EthernetFrame::serialize(
                                            eth.src_mac,
                                            self.config.mac,
                                            ETHERTYPE_IPV4,
                                            &ip_out,
                                        );
                                        out_frames.push(eth_out);
                                    }
                                    IcmpType::EchoReply => {
                                        self.received_icmp_replies.push((
                                            ip_pkt.header.src_ip,
                                            icmp.identifier,
                                            icmp.sequence_number,
                                        ));
                                    }
                                    IcmpType::TimeExceeded => {
                                        self.received_icmp_time_exceeded
                                            .push((ip_pkt.header.src_ip, icmp.code));
                                    }
                                    IcmpType::DestinationUnreachable => {
                                        self.received_icmp_unreachable
                                            .push((ip_pkt.header.src_ip, icmp.code));
                                    }
                                    _ => {}
                                }
                            }
                        }

                        IpProtocol::Udp => {
                            if let Ok(udp) = UdpDatagram::parse(
                                ip_pkt.header.src_ip,
                                ip_pkt.header.dst_ip,
                                ip_pkt.payload,
                                true,
                            ) {
                                self.received_udp_payloads.push((
                                    ip_pkt.header.src_ip,
                                    udp.src_port,
                                    udp.dst_port,
                                    udp.payload.to_vec(),
                                ));

                                if let Some(resp_payload) = self.udp_sockets.dispatch(
                                    ip_pkt.header.src_ip,
                                    udp.src_port,
                                    udp.dst_port,
                                    udp.payload,
                                ) {
                                    let udp_out = UdpDatagram::serialize(
                                        self.config.ip,
                                        ip_pkt.header.src_ip,
                                        udp.dst_port,
                                        udp.src_port,
                                        &resp_payload,
                                    );
                                    let ip_id = self.next_ip_id();
                                    let ip_out = Ipv4Packet::serialize(
                                        self.config.ip,
                                        ip_pkt.header.src_ip,
                                        IP_PROTO_UDP,
                                        ip_id,
                                        64,
                                        &udp_out,
                                    );
                                    let eth_out = EthernetFrame::serialize(
                                        eth.src_mac,
                                        self.config.mac,
                                        ETHERTYPE_IPV4,
                                        &ip_out,
                                    );
                                    out_frames.push(eth_out);
                                }
                            }
                        }

                        IpProtocol::Tcp => {
                            if let Ok(tcp) = TcpSegment::parse(
                                ip_pkt.header.src_ip,
                                ip_pkt.header.dst_ip,
                                ip_pkt.payload,
                                true,
                            ) && let Some(resp_seg) = self.tcp_manager.process_segment(
                                ip_pkt.header.src_ip,
                                ip_pkt.header.dst_ip,
                                &tcp,
                            ) {
                                let ip_id = self.next_ip_id();
                                let ip_out = Ipv4Packet::serialize(
                                    self.config.ip,
                                    ip_pkt.header.src_ip,
                                    IP_PROTO_TCP,
                                    ip_id,
                                    64,
                                    &resp_seg,
                                );
                                let eth_out = EthernetFrame::serialize(
                                    eth.src_mac,
                                    self.config.mac,
                                    ETHERTYPE_IPV4,
                                    &ip_out,
                                );
                                out_frames.push(eth_out);
                            }
                        }

                        _ => {}
                    }
                }
            }

            EtherType::IPv6 => {
                if let Ok(ip6_pkt) = Ipv6Packet::parse(eth.payload) {
                    // Update NDP Cache with sender
                    self.ndp_table.insert(ip6_pkt.header.src_ip, eth.src_mac);

                    let my_ip6 = self.config.ipv6.unwrap_or(Ipv6Address::LOOPBACK);
                    let dst6 = ip6_pkt.header.dst_ip;

                    let is_for_me = dst6 == my_ip6
                        || dst6.is_multicast()
                        || dst6 == Ipv6Address::LINK_LOCAL_ALL_NODES;
                    if !is_for_me {
                        return out_frames;
                    }

                    if ip6_pkt.header.next_header == NEXT_HEADER_ICMPV6
                        && let Ok(icmp6) = Icmpv6Packet::parse(
                            ip6_pkt.header.src_ip,
                            ip6_pkt.header.dst_ip,
                            ip6_pkt.payload,
                            true,
                        )
                    {
                        match icmp6.msg_type {
                            ICMPV6_TYPE_ECHO_REQUEST => {
                                if icmp6.payload.len() >= 4 {
                                    let id =
                                        u16::from_be_bytes([icmp6.payload[0], icmp6.payload[1]]);
                                    let seq =
                                        u16::from_be_bytes([icmp6.payload[2], icmp6.payload[3]]);
                                    let echo_reply = Icmpv6Packet::build_echo_reply(
                                        my_ip6,
                                        ip6_pkt.header.src_ip,
                                        id,
                                        seq,
                                        &icmp6.payload[4..],
                                    );
                                    let ip6_out = Ipv6Packet::serialize(
                                        my_ip6,
                                        ip6_pkt.header.src_ip,
                                        NEXT_HEADER_ICMPV6,
                                        64,
                                        &echo_reply,
                                    );
                                    let eth_out = EthernetFrame::serialize(
                                        eth.src_mac,
                                        self.config.mac,
                                        ETHERTYPE_IPV6,
                                        &ip6_out,
                                    );
                                    out_frames.push(eth_out);
                                }
                            }

                            ICMPV6_TYPE_ECHO_REPLY => {
                                if icmp6.payload.len() >= 4 {
                                    let id =
                                        u16::from_be_bytes([icmp6.payload[0], icmp6.payload[1]]);
                                    let seq =
                                        u16::from_be_bytes([icmp6.payload[2], icmp6.payload[3]]);
                                    self.received_icmpv6_replies.push((
                                        ip6_pkt.header.src_ip,
                                        id,
                                        seq,
                                    ));
                                }
                            }

                            ICMPV6_TYPE_NEIGHBOR_SOLICIT => {
                                if icmp6.payload.len() >= 20 {
                                    let mut target_bytes = [0u8; 16];
                                    target_bytes.copy_from_slice(&icmp6.payload[4..20]);
                                    let target_ip6 = Ipv6Address(target_bytes);

                                    if target_ip6 == my_ip6 {
                                        let na = Icmpv6Packet::build_neighbor_advertisement(
                                            my_ip6,
                                            ip6_pkt.header.src_ip,
                                            my_ip6,
                                            self.config.mac,
                                            false,
                                            true,
                                            true,
                                        );
                                        let ip6_out = Ipv6Packet::serialize(
                                            my_ip6,
                                            ip6_pkt.header.src_ip,
                                            NEXT_HEADER_ICMPV6,
                                            64,
                                            &na,
                                        );
                                        let eth_out = EthernetFrame::serialize(
                                            eth.src_mac,
                                            self.config.mac,
                                            ETHERTYPE_IPV6,
                                            &ip6_out,
                                        );
                                        out_frames.push(eth_out);
                                    }
                                }
                            }

                            ICMPV6_TYPE_NEIGHBOR_ADVERT if icmp6.payload.len() >= 20 => {
                                let mut target_bytes = [0u8; 16];
                                target_bytes.copy_from_slice(&icmp6.payload[4..20]);
                                let target_ip6 = Ipv6Address(target_bytes);

                                if let Some(queued_packets) =
                                    self.pending_ndp_packets.remove(&target_ip6)
                                {
                                    for ip6_pkt_data in queued_packets {
                                        let eth_out = EthernetFrame::serialize(
                                            eth.src_mac,
                                            self.config.mac,
                                            ETHERTYPE_IPV6,
                                            &ip6_pkt_data,
                                        );
                                        out_frames.push(eth_out);
                                    }
                                }
                            }

                            _ => {}
                        }
                    }
                }
            }

            _ => {}
        }

        out_frames
    }
}
