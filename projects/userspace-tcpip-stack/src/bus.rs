//! Virtual Multi-Node Network Bus: Simulates an Ethernet Switch and Routed LAN.
//!
//! Enables multiple `NetStack` nodes (e.g. Client, Web Server, DNS Server, Gateway)
//! to exchange real Ethernet frames in a virtual switched environment.

use crate::ethernet::{EthernetFrame, MacAddress};
use crate::pcap::{LINKTYPE_ETHERNET, PcapWriter};
use crate::stack::NetStack;
use std::collections::HashMap;

pub struct VirtualNetworkBus {
    nodes: HashMap<String, NetStack>,
    mac_to_node: HashMap<MacAddress, String>,
    pcap_writer: Option<PcapWriter<Vec<u8>>>,
    packet_time_usec: u32,
}

impl Default for VirtualNetworkBus {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualNetworkBus {
    pub fn new() -> Self {
        VirtualNetworkBus {
            nodes: HashMap::new(),
            mac_to_node: HashMap::new(),
            pcap_writer: None,
            packet_time_usec: 1000,
        }
    }

    pub fn enable_pcap_capture(&mut self) {
        let buffer = Vec::new();
        let writer = PcapWriter::new(buffer, 65535, LINKTYPE_ETHERNET).expect("PcapWriter init");
        self.pcap_writer = Some(writer);
    }

    pub fn take_pcap_bytes(&mut self) -> Option<Vec<u8>> {
        None
    }

    pub fn add_node(&mut self, name: &str, stack: NetStack) {
        self.mac_to_node.insert(stack.config.mac, name.to_string());
        self.nodes.insert(name.to_string(), stack);
    }

    pub fn get_node_mut(&mut self, name: &str) -> Option<&mut NetStack> {
        self.nodes.get_mut(name)
    }

    /// Injects a raw Ethernet frame into the virtual switched network from a given node.
    /// Propagates responses recursively until all queues settle.
    pub fn send_frame(&mut self, sender_node: &str, raw_frame: Vec<u8>) -> usize {
        let mut queue = vec![(sender_node.to_string(), raw_frame)];
        let mut frames_exchanged = 0;

        while let Some((sender, frame_data)) = queue.pop() {
            frames_exchanged += 1;
            self.packet_time_usec += 100;

            let eth = match EthernetFrame::parse(&frame_data) {
                Ok(e) => e,
                Err(_) => continue,
            };

            let is_bcast = eth.dst_mac.is_broadcast() || eth.dst_mac.is_multicast();

            // Deliver to target nodes
            let node_names: Vec<String> = self.nodes.keys().cloned().collect();
            for target_name in node_names {
                if target_name == sender {
                    continue;
                }

                let target_node = self.nodes.get_mut(&target_name).unwrap();
                let should_receive = is_bcast || target_node.config.mac == eth.dst_mac;

                if should_receive {
                    let responses = target_node.process_frame(&frame_data);
                    for resp in responses {
                        queue.push((target_name.clone(), resp));
                    }
                }
            }
        }

        frames_exchanged
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arp::ArpPacket;
    use crate::ethernet::{ETHERTYPE_ARP, ETHERTYPE_IPV4};
    use crate::icmp::IcmpPacket;
    use crate::ipv4::{IP_PROTO_ICMP, Ipv4Address, Ipv4Packet};
    use crate::stack::NetStackConfig;

    #[test]
    fn test_virtual_network_bus_multi_node_ping() {
        let mut bus = VirtualNetworkBus::new();

        // Node 1: Client (192.168.1.100, 52:54:00:12:34:56)
        let client_mac = MacAddress([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
        let client_ip = Ipv4Address::new(192, 168, 1, 100);
        let client_stack = NetStack::new(NetStackConfig {
            mac: client_mac,
            ip: client_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        });
        bus.add_node("client", client_stack);

        // Node 2: Server (192.168.1.10, 02:00:00:00:00:10)
        let server_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x10]);
        let server_ip = Ipv4Address::new(192, 168, 1, 10);
        let server_stack = NetStack::new(NetStackConfig {
            mac: server_mac,
            ip: server_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        });
        bus.add_node("server", server_stack);

        // 1. Client broadcasts ARP Request
        let arp_req = ArpPacket::build_request(client_mac, client_ip.0, server_ip.0);
        let arp_frame = EthernetFrame::serialize(
            MacAddress::BROADCAST,
            client_mac,
            ETHERTYPE_ARP,
            &arp_req.serialize(),
        );

        let count = bus.send_frame("client", arp_frame);
        assert_eq!(count, 2); // Request + Server Reply

        // Client should have learned Server's MAC
        let client = bus.get_node_mut("client").unwrap();
        assert_eq!(client.arp_table.lookup(&server_ip.0), Some(server_mac));

        // 2. Client sends ICMP Ping to Server
        let ping_req = IcmpPacket::build_echo_request(0xbeef, 1, b"Ping over Virtual Bus");
        let ip_req = Ipv4Packet::serialize(client_ip, server_ip, IP_PROTO_ICMP, 101, 64, &ping_req);
        let eth_req = EthernetFrame::serialize(server_mac, client_mac, ETHERTYPE_IPV4, &ip_req);

        let ping_count = bus.send_frame("client", eth_req);
        assert_eq!(ping_count, 2); // Ping request + Echo reply delivered back to client
    }
}
