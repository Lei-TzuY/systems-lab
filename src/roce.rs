//! RDMA over Converged Ethernet v2 (RoCEv2) & IEEE 802.1Qbb Priority Flow Control (PFC).
//!
//! High-performance AI/ML and storage RDMA transport over UDP port 4791 with lossless Ethernet flow control.

use crate::ethernet::MacAddress;
use std::fmt;

pub const ROCEV2_UDP_PORT: u16 = 4791;
pub const ROCEV2_BTH_LEN: usize = 12;
pub const ROCEV2_RETH_LEN: usize = 16;
pub const ROCEV2_ICRC_LEN: usize = 4;

pub const PFC_MULTICAST_MAC: MacAddress = MacAddress([0x01, 0x80, 0xC2, 0x00, 0x00, 0x01]);
pub const ETHERTYPE_FLOW_CONTROL: u16 = 0x8808;
pub const PFC_OPCODE: u16 = 0x0101;

// InfiniBand Reliable Connected (RC) BTH OpCodes
pub const IB_OPCODE_RC_SEND_ONLY: u8 = 0x04;
pub const IB_OPCODE_RC_RDMA_WRITE_ONLY: u8 = 0x0A;
pub const IB_OPCODE_RC_RDMA_READ_REQUEST: u8 = 0x0C;
pub const IB_OPCODE_RC_ACK: u8 = 0x11;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BthHeader {
    pub opcode: u8,
    pub solicited: bool,
    pub pad_count: u8,
    pub pkey: u16,
    pub dest_qp: u32,  // 24-bit Destination Queue Pair
    pub ack_req: bool,
    pub psn: u32,      // 24-bit Packet Sequence Number
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RethHeader {
    pub virtual_addr: u64,
    pub rkey: u32,
    pub dma_len: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RocePacket {
    pub bth: BthHeader,
    pub reth: Option<RethHeader>,
    pub payload: Vec<u8>,
    pub icrc: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PfcPauseFrame {
    pub class_enable_vector: u8, // 8-bit mask for Priorities 0..7
    pub pause_times: [u16; 8],   // 8 pause quantums in 512 bit times
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoceError {
    PacketTooShort(usize),
    InvalidOpcode(u8),
    InvalidLength,
}

impl fmt::Display for RoceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RoceError::PacketTooShort(l) => write!(f, "RoCEv2 packet too short ({} bytes)", l),
            RoceError::InvalidOpcode(op) => write!(f, "Invalid InfiniBand BTH OpCode: 0x{:02X}", op),
            RoceError::InvalidLength => write!(f, "Invalid RoCEv2 length"),
        }
    }
}

impl std::error::Error for RoceError {}

impl BthHeader {
    pub fn new(opcode: u8, pkey: u16, dest_qp: u32, ack_req: bool, psn: u32) -> Self {
        BthHeader {
            opcode,
            solicited: false,
            pad_count: 0,
            pkey,
            dest_qp: dest_qp & 0x00FF_FFFF,
            ack_req,
            psn: psn & 0x00FF_FFFF,
        }
    }

    pub fn serialize(&self) -> [u8; 12] {
        let mut buf = [0u8; 12];
        buf[0] = self.opcode;
        let mut b1 = 0u8;
        if self.solicited {
            b1 |= 0x80;
        }
        b1 |= (self.pad_count & 0x03) << 4;
        buf[1] = b1;

        buf[2..4].copy_from_slice(&self.pkey.to_be_bytes());

        let qp_bytes = (self.dest_qp & 0x00FF_FFFF).to_be_bytes();
        buf[4] = 0x00; // Reserved
        buf[5..8].copy_from_slice(&qp_bytes[1..4]);

        let psn_bytes = (self.psn & 0x00FF_FFFF).to_be_bytes();
        buf[8] = if self.ack_req { 0x80 } else { 0x00 };
        buf[9..12].copy_from_slice(&psn_bytes[1..4]);

        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, RoceError> {
        if data.len() < ROCEV2_BTH_LEN {
            return Err(RoceError::PacketTooShort(data.len()));
        }

        let opcode = data[0];
        let solicited = (data[1] & 0x80) != 0;
        let pad_count = (data[1] >> 4) & 0x03;
        let pkey = u16::from_be_bytes([data[2], data[3]]);
        let dest_qp = u32::from_be_bytes([0, data[5], data[6], data[7]]);
        let ack_req = (data[8] & 0x80) != 0;
        let psn = u32::from_be_bytes([0, data[9], data[10], data[11]]);

        Ok(BthHeader {
            opcode,
            solicited,
            pad_count,
            pkey,
            dest_qp,
            ack_req,
            psn,
        })
    }
}

impl RethHeader {
    pub fn new(virtual_addr: u64, rkey: u32, dma_len: u32) -> Self {
        RethHeader {
            virtual_addr,
            rkey,
            dma_len,
        }
    }

    pub fn serialize(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&self.virtual_addr.to_be_bytes());
        buf[8..12].copy_from_slice(&self.rkey.to_be_bytes());
        buf[12..16].copy_from_slice(&self.dma_len.to_be_bytes());
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, RoceError> {
        if data.len() < ROCEV2_RETH_LEN {
            return Err(RoceError::PacketTooShort(data.len()));
        }

        let mut v_bytes = [0u8; 8];
        v_bytes.copy_from_slice(&data[0..8]);
        let virtual_addr = u64::from_be_bytes(v_bytes);
        let rkey = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let dma_len = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);

        Ok(RethHeader {
            virtual_addr,
            rkey,
            dma_len,
        })
    }
}

impl RocePacket {
    pub fn build_send(dest_qp: u32, psn: u32, payload: &[u8]) -> Self {
        let bth = BthHeader::new(IB_OPCODE_RC_SEND_ONLY, 0xFFFF, dest_qp, true, psn);
        let mut pkt = RocePacket {
            bth,
            reth: None,
            payload: payload.to_vec(),
            icrc: 0xDEADC0DE,
        };
        pkt.icrc = pkt.compute_icrc();
        pkt
    }

    pub fn build_rdma_write(dest_qp: u32, psn: u32, vaddr: u64, rkey: u32, payload: &[u8]) -> Self {
        let bth = BthHeader::new(IB_OPCODE_RC_RDMA_WRITE_ONLY, 0xFFFF, dest_qp, true, psn);
        let reth = RethHeader::new(vaddr, rkey, payload.len() as u32);
        let mut pkt = RocePacket {
            bth,
            reth: Some(reth),
            payload: payload.to_vec(),
            icrc: 0xDEADC0DE,
        };
        pkt.icrc = pkt.compute_icrc();
        pkt
    }

    pub fn build_ack(dest_qp: u32, psn: u32) -> Self {
        let bth = BthHeader::new(IB_OPCODE_RC_ACK, 0xFFFF, dest_qp, false, psn);
        let mut pkt = RocePacket {
            bth,
            reth: None,
            payload: Vec::new(),
            icrc: 0xDEADC0DE,
        };
        pkt.icrc = pkt.compute_icrc();
        pkt
    }

    pub fn compute_icrc(&self) -> u32 {
        let mut hash: u32 = 0x811C9DC5; // FNV-1a basis
        for b in self.bth.serialize() {
            hash ^= b as u32;
            hash = hash.wrapping_mul(0x01000193);
        }
        if let Some(ref r) = self.reth {
            for b in r.serialize() {
                hash ^= b as u32;
                hash = hash.wrapping_mul(0x01000193);
            }
        }
        for b in &self.payload {
            hash ^= *b as u32;
            hash = hash.wrapping_mul(0x01000193);
        }
        hash
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.bth.serialize());
        if let Some(ref r) = self.reth {
            buf.extend_from_slice(&r.serialize());
        }
        buf.extend_from_slice(&self.payload);
        buf.extend_from_slice(&self.icrc.to_be_bytes());
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, RoceError> {
        if data.len() < ROCEV2_BTH_LEN + ROCEV2_ICRC_LEN {
            return Err(RoceError::PacketTooShort(data.len()));
        }

        let bth = BthHeader::parse(&data[..ROCEV2_BTH_LEN])?;
        let mut offset = ROCEV2_BTH_LEN;

        let reth = if bth.opcode == IB_OPCODE_RC_RDMA_WRITE_ONLY || bth.opcode == IB_OPCODE_RC_RDMA_READ_REQUEST {
            if data.len() < offset + ROCEV2_RETH_LEN + ROCEV2_ICRC_LEN {
                return Err(RoceError::PacketTooShort(data.len()));
            }
            let r = RethHeader::parse(&data[offset..offset + ROCEV2_RETH_LEN])?;
            offset += ROCEV2_RETH_LEN;
            Some(r)
        } else {
            None
        };

        let payload_len = data.len() - offset - ROCEV2_ICRC_LEN;
        let payload = data[offset..offset + payload_len].to_vec();
        let icrc = u32::from_be_bytes([
            data[data.len() - 4],
            data[data.len() - 3],
            data[data.len() - 2],
            data[data.len() - 1],
        ]);

        Ok(RocePacket {
            bth,
            reth,
            payload,
            icrc,
        })
    }
}

impl PfcPauseFrame {
    pub fn new(paused_classes: &[u8], pause_duration: u16) -> Self {
        let mut cev = 0u8;
        let mut pause_times = [0u16; 8];
        for &cls in paused_classes {
            if cls < 8 {
                cev |= 1 << cls;
                pause_times[cls as usize] = pause_duration;
            }
        }

        PfcPauseFrame {
            class_enable_vector: cev,
            pause_times,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&PFC_OPCODE.to_be_bytes()); // 0x0101
        buf.push(0x00);
        buf.push(self.class_enable_vector); // 16-bit Class Enable Vector (MSB 0, LSB CEV)
        for pt in &self.pause_times {
            buf.extend_from_slice(&pt.to_be_bytes());
        }
        // Pad to minimum 46 bytes for Ethernet MAC Control payload
        while buf.len() < 46 {
            buf.push(0x00);
        }
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, RoceError> {
        if data.len() < 20 {
            return Err(RoceError::PacketTooShort(data.len()));
        }

        let opcode = u16::from_be_bytes([data[0], data[1]]);
        if opcode != PFC_OPCODE {
            return Err(RoceError::InvalidOpcode(data[1]));
        }

        let class_enable_vector = data[3];
        let mut pause_times = [0u16; 8];
        for i in 0..8 {
            let offset = 4 + i * 2;
            pause_times[i] = u16::from_be_bytes([data[offset], data[offset + 1]]);
        }

        Ok(PfcPauseFrame {
            class_enable_vector,
            pause_times,
        })
    }
}

#[derive(Debug, Clone)]
pub struct RdmaQueuePair {
    pub qp_number: u32,
    pub remote_qp: u32,
    pub next_send_psn: u32,
    pub expected_recv_psn: u32,
}

impl RdmaQueuePair {
    pub fn new(qp_number: u32, remote_qp: u32, initial_psn: u32) -> Self {
        RdmaQueuePair {
            qp_number,
            remote_qp,
            next_send_psn: initial_psn,
            expected_recv_psn: initial_psn,
        }
    }

    pub fn send_message(&mut self, payload: &[u8]) -> RocePacket {
        let psn = self.next_send_psn;
        self.next_send_psn = (self.next_send_psn + 1) & 0x00FF_FFFF;
        RocePacket::build_send(self.remote_qp, psn, payload)
    }

    pub fn receive_packet(&mut self, pkt: &RocePacket) -> bool {
        if pkt.bth.dest_qp == self.qp_number {
            self.expected_recv_psn = (pkt.bth.psn + 1) & 0x00FF_FFFF;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roce_send_and_ack_roundtrip() {
        let mut qp1 = RdmaQueuePair::new(101, 202, 1000);
        let mut qp2 = RdmaQueuePair::new(202, 101, 1000);

        let send_pkt = qp1.send_message(b"Direct GPU Memory RDMA Buffer Transfer");
        let raw = send_pkt.serialize();

        let parsed = RocePacket::parse(&raw).unwrap();
        assert_eq!(parsed.bth.opcode, IB_OPCODE_RC_SEND_ONLY);
        assert_eq!(parsed.bth.dest_qp, 202);
        assert_eq!(parsed.bth.psn, 1000);
        assert_eq!(parsed.payload, b"Direct GPU Memory RDMA Buffer Transfer");

        let ok = qp2.receive_packet(&parsed);
        assert_eq!(ok, true);
        assert_eq!(qp2.expected_recv_psn, 1001);

        let ack = RocePacket::build_ack(101, 1000);
        let raw_ack = ack.serialize();
        let parsed_ack = RocePacket::parse(&raw_ack).unwrap();
        assert_eq!(parsed_ack.bth.opcode, IB_OPCODE_RC_ACK);
        assert_eq!(parsed_ack.bth.dest_qp, 101);
    }

    #[test]
    fn test_pfc_pause_frame_roundtrip() {
        let pfc = PfcPauseFrame::new(&[3, 4], 65535);
        let raw = pfc.serialize();
        assert_eq!(raw.len() >= 46, true);

        let parsed = PfcPauseFrame::parse(&raw).unwrap();
        assert_eq!(parsed.class_enable_vector, 0b00011000); // Bits 3 and 4 enabled
        assert_eq!(parsed.pause_times[3], 65535);
        assert_eq!(parsed.pause_times[4], 65535);
        assert_eq!(parsed.pause_times[0], 0);
    }
}
