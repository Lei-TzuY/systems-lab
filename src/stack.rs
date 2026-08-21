//! Unified NetStack: Dual-Stack IPv4/IPv6 Layer 2 -> Layer 3 -> Layer 4 packet processing pipeline.

use crate::arp::{ArpOpcode, ArpPacket, ArpTable};
use crate::ethernet::{EtherType, EthernetFrame, MacAddress, ETHERTYPE_ARP, ETHERTYPE_IPV4, ETHERTYPE_IPV6};
use crate::firewall::{Firewall, FirewallAction, FirewallChain};
use crate::icmp::{IcmpPacket, IcmpType};
use crate::icmpv6::{Icmpv6Packet, NdpTable, ICMPV6_TYPE_ECHO_REQUEST, ICMPV6_TYPE_NEIGHBOR_SOLICIT};
use crate::ipv4::{IpProtocol, Ipv4Address, Ipv4Packet, IP_PROTO_ICMP, IP_PROTO_TCP, IP_PROTO_UDP};
use crate::ipv6::{Ipv6Address, Ipv6Packet, NEXT_HEADER_ICMPV6};
use crate::nat::NatTable;
use crate::router::RoutingTable;
use crate::tcp::{TcpManager, TcpSegment};
use crate::udp::{UdpDatagram, UdpSocketTable};

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
    ip_id_counter: u16,
}

impl NetStack {
    pub fn new(config: NetStackConfig) -> Self {
        let mut routing_table = RoutingTable::new();

        // Local subnet route
        let subnet_net = Ipv4Address([
            config.ip.0[0] & (0xFF << (8 - config.subnet_mask.min(8))),
            0,
            0,
            0,
        ]);
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
        }
    }

    pub fn enable_nat(&mut self, public_ip: Ipv4Address) {
        self.nat = Some(NatTable::new(public_ip));
    }

    fn next_ip_id(&mut self) -> u16 {
        let id = self.ip_id_counter;
        self.ip_id_counter = self.ip_id_counter.wrapping_add(1);
        id
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
        if !eth.dst_mac.is_broadcast() && !eth.dst_mac.is_multicast() && eth.dst_mac != self.config.mac {
            return out_frames;
        }

        match eth.ethertype {
            EtherType::Arp => {
                if let Ok(arp) = ArpPacket::parse(eth.payload) {
                    // Update ARP cache with sender
                    self.arp_table.insert(arp.sender_ip, arp.sender_mac);

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
                    if self.firewall.evaluate(FirewallChain::Input, &ip_pkt) != FirewallAction::Accept {
                        return out_frames; // Dropped by firewall!
                    }

                    // Cache sender MAC for source IP
                    self.arp_table.insert(ip_pkt.header.src_ip.0, eth.src_mac);

                    // Verify destination IP
                    let dst = ip_pkt.header.dst_ip;
                    if dst != self.config.ip && !dst.is_broadcast() && dst != Ipv4Address::BROADCAST {
                        return out_frames;
                    }

                    match ip_pkt.header.protocol {
                        IpProtocol::Icmp => {
                            if let Ok(icmp) = IcmpPacket::parse(ip_pkt.payload, true) {
                                if icmp.icmp_type == IcmpType::EchoRequest {
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
                            }
                        }

                        IpProtocol::Udp => {
                            if let Ok(udp) = UdpDatagram::parse(
                                ip_pkt.header.src_ip,
                                ip_pkt.header.dst_ip,
                                ip_pkt.payload,
                                true,
                            ) {
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
                            ) {
                                if let Some(resp_seg) = self.tcp_manager.process_segment(
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

                    let is_for_me = dst6 == my_ip6 || dst6.is_multicast() || dst6 == Ipv6Address::LINK_LOCAL_ALL_NODES;
                    if !is_for_me {
                        return out_frames;
                    }

                    if ip6_pkt.header.next_header == NEXT_HEADER_ICMPV6 {
                        if let Ok(icmp6) = Icmpv6Packet::parse(ip6_pkt.header.src_ip, ip6_pkt.header.dst_ip, ip6_pkt.payload, true) {
                            match icmp6.msg_type {
                                ICMPV6_TYPE_ECHO_REQUEST => {
                                    if icmp6.payload.len() >= 4 {
                                        let id = u16::from_be_bytes([icmp6.payload[0], icmp6.payload[1]]);
                                        let seq = u16::from_be_bytes([icmp6.payload[2], icmp6.payload[3]]);
                                        let echo_reply = Icmpv6Packet::build_echo_reply(my_ip6, ip6_pkt.header.src_ip, id, seq, &icmp6.payload[4..]);
                                        let ip6_out = Ipv6Packet::serialize(my_ip6, ip6_pkt.header.src_ip, NEXT_HEADER_ICMPV6, 64, &echo_reply);
                                        let eth_out = EthernetFrame::serialize(eth.src_mac, self.config.mac, ETHERTYPE_IPV6, &ip6_out);
                                        out_frames.push(eth_out);
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
                                            let ip6_out = Ipv6Packet::serialize(my_ip6, ip6_pkt.header.src_ip, NEXT_HEADER_ICMPV6, 64, &na);
                                            let eth_out = EthernetFrame::serialize(eth.src_mac, self.config.mac, ETHERTYPE_IPV6, &ip6_out);
                                            out_frames.push(eth_out);
                                        }
                                    }
                                }

                                _ => {}
                            }
                        }
                    }
                }
            }

            _ => {}
        }

        out_frames
    }
}
