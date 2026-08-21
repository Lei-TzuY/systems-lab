//! Network Address Translation (NAT / NAPT / Masquerade) and Connection Tracking.
//!
//! Enables dynamic Port Address Translation (SNAT) for LAN clients sharing a public IP
//! and static Port Forwarding (DNAT) from public ports to private internal servers.

use crate::checksum::{compute_checksum, compute_ipv4_transport_checksum};
use crate::ipv4::{IP_PROTO_ICMP, IP_PROTO_TCP, IP_PROTO_UDP, IPV4_MIN_HEADER_LEN, Ipv4Address};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SocketAddrTuple {
    pub ip: Ipv4Address,
    pub port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NatSessionKey {
    pub src: SocketAddrTuple,
    pub dst: SocketAddrTuple,
    pub protocol: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NatBinding {
    pub internal_src: SocketAddrTuple,
    pub external_src: SocketAddrTuple,
    pub remote_dst: SocketAddrTuple,
    pub protocol: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortForwardRule {
    pub external_port: u16,
    pub internal_ip: Ipv4Address,
    pub internal_port: u16,
    pub protocol: u8,
}

pub struct NatTable {
    pub public_ip: Ipv4Address,
    next_alloc_port: u16,
    // Outbound session: (Internal IP, Internal Port, Remote IP, Remote Port, Proto) -> External Port
    outbound_sessions: HashMap<NatSessionKey, u16>,
    // Inbound lookup: (External Port, Remote IP, Remote Port, Proto) -> (Internal IP, Internal Port)
    inbound_lookup: HashMap<(u16, Ipv4Address, u16, u8), SocketAddrTuple>,
    // Static DNAT port forwarding: (External Port, Proto) -> (Internal IP, Internal Port)
    port_forwards: HashMap<(u16, u8), SocketAddrTuple>,
}

impl NatTable {
    pub fn new(public_ip: Ipv4Address) -> Self {
        NatTable {
            public_ip,
            next_alloc_port: 40000,
            outbound_sessions: HashMap::new(),
            inbound_lookup: HashMap::new(),
            port_forwards: HashMap::new(),
        }
    }

    /// Adds a static DNAT Port Forwarding rule (e.g. 8080 -> 192.168.1.10:80).
    pub fn add_port_forward(
        &mut self,
        external_port: u16,
        internal_ip: Ipv4Address,
        internal_port: u16,
        protocol: u8,
    ) {
        self.port_forwards.insert(
            (external_port, protocol),
            SocketAddrTuple {
                ip: internal_ip,
                port: internal_port,
            },
        );
    }

    /// Allocates an ephemeral public port for SNAT.
    fn allocate_port(&mut self) -> u16 {
        let port = self.next_alloc_port;
        self.next_alloc_port = if self.next_alloc_port >= 65000 {
            40000
        } else {
            self.next_alloc_port + 1
        };
        port
    }

    /// Translates an outbound IPv4 packet from internal private host to external public destination (SNAT).
    /// Rewrites Source IP to public IP and Source Port to allocated NAT port; recalculates checksums.
    pub fn translate_outbound(&mut self, packet: &mut [u8]) -> bool {
        if packet.len() < IPV4_MIN_HEADER_LEN {
            return false;
        }

        let ihl = (packet[0] & 0x0F) as usize * 4;
        if packet.len() < ihl {
            return false;
        }

        let protocol = packet[9];
        let src_ip = Ipv4Address([packet[12], packet[13], packet[14], packet[15]]);
        let dst_ip = Ipv4Address([packet[16], packet[17], packet[18], packet[19]]);

        match protocol {
            IP_PROTO_TCP | IP_PROTO_UDP => {
                if packet.len() < ihl + 4 {
                    return false;
                }
                let src_port = u16::from_be_bytes([packet[ihl], packet[ihl + 1]]);
                let dst_port = u16::from_be_bytes([packet[ihl + 2], packet[ihl + 3]]);

                let session_key = NatSessionKey {
                    src: SocketAddrTuple {
                        ip: src_ip,
                        port: src_port,
                    },
                    dst: SocketAddrTuple {
                        ip: dst_ip,
                        port: dst_port,
                    },
                    protocol,
                };

                let ext_port = if let Some(&p) = self.outbound_sessions.get(&session_key) {
                    p
                } else {
                    let new_p = self.allocate_port();
                    self.outbound_sessions.insert(session_key, new_p);
                    self.inbound_lookup.insert(
                        (new_p, dst_ip, dst_port, protocol),
                        SocketAddrTuple {
                            ip: src_ip,
                            port: src_port,
                        },
                    );
                    new_p
                };

                // Rewrite Source IP -> Public IP
                packet[12..16].copy_from_slice(&self.public_ip.0);
                // Rewrite Source Port -> External NAT Port
                packet[ihl..ihl + 2].copy_from_slice(&ext_port.to_be_bytes());

                // Recompute IPv4 Header Checksum
                packet[10..12].copy_from_slice(&[0x00, 0x00]);
                let ip_csum = compute_checksum(&packet[0..ihl]);
                packet[10..12].copy_from_slice(&ip_csum.to_be_bytes());

                // Recompute Transport Checksum
                if protocol == IP_PROTO_TCP {
                    if packet.len() >= ihl + 20 {
                        packet[ihl + 16..ihl + 18].copy_from_slice(&[0x00, 0x00]);
                        let tcp_csum = compute_ipv4_transport_checksum(
                            self.public_ip.0,
                            dst_ip.0,
                            6,
                            &packet[ihl..],
                        );
                        packet[ihl + 16..ihl + 18].copy_from_slice(&tcp_csum.to_be_bytes());
                    }
                } else if protocol == IP_PROTO_UDP && packet.len() >= ihl + 8 {
                    packet[ihl + 6..ihl + 8].copy_from_slice(&[0x00, 0x00]);
                    let udp_csum = compute_ipv4_transport_checksum(
                        self.public_ip.0,
                        dst_ip.0,
                        17,
                        &packet[ihl..],
                    );
                    packet[ihl + 6..ihl + 8].copy_from_slice(&udp_csum.to_be_bytes());
                }

                true
            }

            IP_PROTO_ICMP => {
                // ICMP SNAT: Rewrite Source IP to Public IP
                packet[12..16].copy_from_slice(&self.public_ip.0);
                packet[10..12].copy_from_slice(&[0x00, 0x00]);
                let ip_csum = compute_checksum(&packet[0..ihl]);
                packet[10..12].copy_from_slice(&ip_csum.to_be_bytes());
                true
            }

            _ => false,
        }
    }

    /// Translates an incoming reply IPv4 packet from public internet to internal LAN destination (Reverse SNAT / DNAT).
    /// Rewrites Destination IP and Port back to private host; recalculates checksums.
    pub fn translate_inbound(&mut self, packet: &mut [u8]) -> bool {
        if packet.len() < IPV4_MIN_HEADER_LEN {
            return false;
        }

        let ihl = (packet[0] & 0x0F) as usize * 4;
        if packet.len() < ihl {
            return false;
        }

        let protocol = packet[9];
        let src_ip = Ipv4Address([packet[12], packet[13], packet[14], packet[15]]);
        let _dst_ip = Ipv4Address([packet[16], packet[17], packet[18], packet[19]]);

        match protocol {
            IP_PROTO_TCP | IP_PROTO_UDP => {
                if packet.len() < ihl + 4 {
                    return false;
                }
                let src_port = u16::from_be_bytes([packet[ihl], packet[ihl + 1]]);
                let dst_port = u16::from_be_bytes([packet[ihl + 2], packet[ihl + 3]]);

                // Check 1: Dynamic SNAT reverse lookup
                let target = if let Some(&internal) = self
                    .inbound_lookup
                    .get(&(dst_port, src_ip, src_port, protocol))
                {
                    Some(internal)
                } else if let Some(&fwd) = self.port_forwards.get(&(dst_port, protocol)) {
                    // Check 2: Static DNAT Port Forwarding
                    Some(fwd)
                } else {
                    None
                };

                if let Some(target_addr) = target {
                    // Rewrite Destination IP -> Internal IP
                    packet[16..20].copy_from_slice(&target_addr.ip.0);
                    // Rewrite Destination Port -> Internal Port
                    packet[ihl + 2..ihl + 4].copy_from_slice(&target_addr.port.to_be_bytes());

                    // Recompute IPv4 Header Checksum
                    packet[10..12].copy_from_slice(&[0x00, 0x00]);
                    let ip_csum = compute_checksum(&packet[0..ihl]);
                    packet[10..12].copy_from_slice(&ip_csum.to_be_bytes());

                    // Recompute Transport Checksum
                    if protocol == IP_PROTO_TCP {
                        if packet.len() >= ihl + 20 {
                            packet[ihl + 16..ihl + 18].copy_from_slice(&[0x00, 0x00]);
                            let tcp_csum = compute_ipv4_transport_checksum(
                                src_ip.0,
                                target_addr.ip.0,
                                6,
                                &packet[ihl..],
                            );
                            packet[ihl + 16..ihl + 18].copy_from_slice(&tcp_csum.to_be_bytes());
                        }
                    } else if protocol == IP_PROTO_UDP && packet.len() >= ihl + 8 {
                        packet[ihl + 6..ihl + 8].copy_from_slice(&[0x00, 0x00]);
                        let udp_csum = compute_ipv4_transport_checksum(
                            src_ip.0,
                            target_addr.ip.0,
                            17,
                            &packet[ihl..],
                        );
                        packet[ihl + 6..ihl + 8].copy_from_slice(&udp_csum.to_be_bytes());
                    }

                    return true;
                }

                false
            }

            _ => false,
        }
    }

    pub fn active_session_count(&self) -> usize {
        self.outbound_sessions.len()
    }

    pub fn port_forward_rules(&self) -> Vec<PortForwardRule> {
        self.port_forwards
            .iter()
            .map(|(&(ext_port, proto), &target)| PortForwardRule {
                external_port: ext_port,
                internal_ip: target.ip,
                internal_port: target.port,
                protocol: proto,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipv4::Ipv4Packet;
    use crate::tcp::{TcpFlags, TcpSegment};

    #[test]
    fn test_snat_outbound_and_inbound_translation() {
        let public_ip = Ipv4Address::new(203, 0, 113, 1);
        let mut nat = NatTable::new(public_ip);

        let client_ip = Ipv4Address::new(192, 168, 1, 100);
        let web_server_ip = Ipv4Address::new(93, 184, 216, 34);

        // 1. Client creates TCP SYN packet to remote web server
        let tcp_syn = TcpSegment::serialize(
            client_ip,
            web_server_ip,
            54321,
            80,
            1000,
            0,
            TcpFlags::syn(),
            65535,
            &[],
        );
        let mut ip_syn =
            Ipv4Packet::serialize(client_ip, web_server_ip, IP_PROTO_TCP, 100, 64, &tcp_syn);

        // 2. Gateway translates outbound SNAT
        let translated = nat.translate_outbound(&mut ip_syn);
        assert!(translated);

        let parsed_out = Ipv4Packet::parse(&ip_syn, true).unwrap();
        assert_eq!(parsed_out.header.src_ip, public_ip); // Rewritten to Public IP!
        assert_eq!(parsed_out.header.dst_ip, web_server_ip);

        let parsed_tcp_out =
            TcpSegment::parse(public_ip, web_server_ip, parsed_out.payload, true).unwrap();
        assert_eq!(parsed_tcp_out.src_port, 40000); // Rewritten to NAT port 40000!

        // 3. Web server replies with SYN-ACK to (203.0.113.1:40000)
        let tcp_syn_ack = TcpSegment::serialize(
            web_server_ip,
            public_ip,
            80,
            40000,
            5000,
            1001,
            TcpFlags::syn_ack(),
            65535,
            &[],
        );
        let mut ip_syn_ack = Ipv4Packet::serialize(
            web_server_ip,
            public_ip,
            IP_PROTO_TCP,
            200,
            64,
            &tcp_syn_ack,
        );

        // 4. Gateway translates inbound reply back to client
        let in_translated = nat.translate_inbound(&mut ip_syn_ack);
        assert!(in_translated);

        let parsed_in = Ipv4Packet::parse(&ip_syn_ack, true).unwrap();
        assert_eq!(parsed_in.header.dst_ip, client_ip); // Restored to 192.168.1.100!

        let parsed_tcp_in =
            TcpSegment::parse(web_server_ip, client_ip, parsed_in.payload, true).unwrap();
        assert_eq!(parsed_tcp_in.dst_port, 54321); // Restored to original client port 54321!
    }

    #[test]
    fn test_dnat_port_forwarding() {
        let public_ip = Ipv4Address::new(203, 0, 113, 1);
        let mut nat = NatTable::new(public_ip);

        let internal_server = Ipv4Address::new(192, 168, 1, 10);
        // Forward public port 8080 -> 192.168.1.10:80
        nat.add_port_forward(8080, internal_server, 80, IP_PROTO_TCP);

        let internet_client = Ipv4Address::new(198, 51, 100, 50);

        // Internet client connects to public_ip:8080
        let tcp_req = TcpSegment::serialize(
            internet_client,
            public_ip,
            33333,
            8080,
            100,
            0,
            TcpFlags::syn(),
            65535,
            &[],
        );
        let mut ip_req =
            Ipv4Packet::serialize(internet_client, public_ip, IP_PROTO_TCP, 1, 64, &tcp_req);

        let forwarded = nat.translate_inbound(&mut ip_req);
        assert!(forwarded);

        let parsed = Ipv4Packet::parse(&ip_req, true).unwrap();
        assert_eq!(parsed.header.dst_ip, internal_server);

        let parsed_tcp =
            TcpSegment::parse(internet_client, internal_server, parsed.payload, true).unwrap();
        assert_eq!(parsed_tcp.dst_port, 80); // Forwarded to port 80!
    }
}
