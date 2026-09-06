//! Virtual Private LAN Service & Ethernet Pseudowire (VPLS / EoMPLS - RFC 4447 / RFC 4448 / RFC 4762).
//!
//! Provides multipoint Layer 2 VPN emulation over MPLS with Control Word sequencing,
//! dynamic MAC learning, and Split-Horizon loop prevention.

use crate::ethernet::{EthernetFrame, MacAddress};
use crate::mpls::{MplsHeader, MplsPacket};
use std::collections::HashMap;

pub const PW_CONTROL_WORD_LEN: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PwControlWord {
    pub flags: u8,
    pub length: u8,
    pub seq_num: u16,
}

impl PwControlWord {
    pub fn new(seq_num: u16) -> Self {
        PwControlWord {
            flags: 0,
            length: 0,
            seq_num,
        }
    }

    pub fn serialize(&self) -> [u8; 4] {
        let mut buf = [0u8; 4];
        buf[0] = (self.flags & 0x0F) << 4;
        buf[1] = self.length & 0x3F;
        buf[2..4].copy_from_slice(&self.seq_num.to_be_bytes());
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        let flags = (data[0] >> 4) & 0x0F;
        let length = data[1] & 0x3F;
        let seq_num = u16::from_be_bytes([data[2], data[3]]);
        Some(PwControlWord {
            flags,
            length,
            seq_num,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VplsPseudowire {
    pub peer_ip: crate::ipv4::Ipv4Address,
    pub vc_label_tx: u32,
    pub vc_label_rx: u32,
    pub tunnel_label_tx: u32,
}

#[derive(Debug, Clone, Default)]
pub struct VplsInstance {
    pub vpls_id: u32,
    pub pseudowires: HashMap<u32, VplsPseudowire>, // VC Label Rx -> Pseudowire
    pub mac_table: HashMap<MacAddress, Option<u32>>, // MAC -> Some(VC Label Rx) or None (Local)
}

impl VplsInstance {
    pub fn new(vpls_id: u32) -> Self {
        VplsInstance {
            vpls_id,
            pseudowires: HashMap::new(),
            mac_table: HashMap::new(),
        }
    }

    pub fn add_pseudowire(&mut self, pw: VplsPseudowire) {
        self.pseudowires.insert(pw.vc_label_rx, pw);
    }

    pub fn learn_mac(&mut self, mac: MacAddress, from_pw: Option<u32>) {
        self.mac_table.insert(mac, from_pw);
    }

    /// Encapsulates local Ethernet frame into MPLS VPLS packet (Tunnel Label + VC Label + CW + Frame)
    pub fn encapsulate_frame(
        &self,
        target_mac: MacAddress,
        eth_frame: &[u8],
        seq: u16,
    ) -> Option<Vec<u8>> {
        let egress_pw_id = self.mac_table.get(&target_mac)?.as_ref()?;
        let pw = self.pseudowires.get(egress_pw_id)?;

        let cw = PwControlWord::new(seq);
        let mut pw_payload = Vec::with_capacity(PW_CONTROL_WORD_LEN + eth_frame.len());
        pw_payload.extend_from_slice(&cw.serialize());
        pw_payload.extend_from_slice(eth_frame);

        // Build 2-label stack: [Tunnel Label (bos=0), VC Label (bos=1)]
        let labels = vec![
            MplsHeader::new(pw.tunnel_label_tx, 0, false, 64),
            MplsHeader::new(pw.vc_label_tx, 0, true, 64),
        ];

        let mpls_pkt = MplsPacket::new(labels, pw_payload);
        Some(mpls_pkt.serialize())
    }

    /// Decapsulates received MPLS VPLS packet and applies Split-Horizon loop check
    pub fn process_ingress_vpls(&mut self, data: &[u8]) -> Option<(MacAddress, Vec<u8>)> {
        let mpls_pkt = MplsPacket::parse(data).ok()?;
        if mpls_pkt.labels.is_empty() {
            return None;
        }

        let vc_label = mpls_pkt.labels.last()?.label;
        if !self.pseudowires.contains_key(&vc_label) {
            return None;
        }

        if mpls_pkt.payload.len() < PW_CONTROL_WORD_LEN {
            return None;
        }

        let eth_bytes = &mpls_pkt.payload[PW_CONTROL_WORD_LEN..];
        let parsed_eth = EthernetFrame::parse(eth_bytes).ok()?;

        // Dynamic MAC Learning from Pseudowire
        self.learn_mac(parsed_eth.src_mac, Some(vc_label));

        Some((parsed_eth.dst_mac, eth_bytes.to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ethernet::ETHERTYPE_IPV4;
    use crate::ipv4::Ipv4Address;

    #[test]
    fn test_vpls_encapsulation_and_mac_learning() {
        let mut vpls = VplsInstance::new(100);

        let peer_pw = VplsPseudowire {
            peer_ip: Ipv4Address::new(10, 0, 0, 2),
            vc_label_tx: 5001,
            vc_label_rx: 6001,
            tunnel_label_tx: 1001,
        };
        vpls.add_pseudowire(peer_pw);

        let client_mac = MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let remote_mac = MacAddress([0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);

        let inner_eth = EthernetFrame::serialize(
            remote_mac,
            client_mac,
            ETHERTYPE_IPV4,
            b"Encapsulated VPLS Customer Packet",
        );

        // 1. Simulate incoming packet from Remote over Pseudowire
        let cw = PwControlWord::new(1);
        let mut incoming_payload = Vec::new();
        incoming_payload.extend_from_slice(&cw.serialize());
        incoming_payload.extend_from_slice(&inner_eth);

        let incoming_mpls =
            MplsPacket::new(vec![MplsHeader::new(6001, 0, true, 64)], incoming_payload).serialize();

        let (dst_mac, decapped_eth) = vpls.process_ingress_vpls(&incoming_mpls).unwrap();
        assert_eq!(dst_mac, remote_mac);
        assert_eq!(decapped_eth, inner_eth);
        assert_eq!(vpls.mac_table.get(&client_mac), Some(&Some(6001)));

        // 2. Transmit return packet to learned MAC
        let return_vpls_pkt = vpls.encapsulate_frame(client_mac, &inner_eth, 2).unwrap();
        let parsed_mpls = MplsPacket::parse(&return_vpls_pkt).unwrap();
        assert_eq!(parsed_mpls.labels.len(), 2);
        assert_eq!(parsed_mpls.labels[0].label, 1001); // Tunnel label
        assert_eq!(parsed_mpls.labels[1].label, 5001); // VC label
    }
}
