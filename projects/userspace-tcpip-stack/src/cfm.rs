//! Connectivity Fault Management & Ethernet OAM (IEEE 802.1ag / ITU-T Y.1731 - CFM).
//!
//! Provides Carrier Ethernet link monitoring, Continuity Check Messages (CCM),
//! Loopback (LBM/LBR), and Linktrace over EtherType 0x8902.

use crate::ethernet::MacAddress;

pub const ETHERTYPE_CFM: u16 = 0x8902;
pub const CFM_OPCODE_CCM: u8 = 1;
pub const CFM_OPCODE_LBR: u8 = 2;
pub const CFM_OPCODE_LBM: u8 = 3;
pub const CFM_OPCODE_LTR: u8 = 4;
pub const CFM_OPCODE_LTM: u8 = 5;

pub const CFM_MULTICAST_CLASS1: MacAddress = MacAddress([0x01, 0x80, 0xC2, 0x00, 0x00, 0x30]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CfmHeader {
    pub md_level: u8,         // 3 bits: Maintenance Domain Level (0..7)
    pub version: u8,          // 5 bits: CFM protocol version (0)
    pub opcode: u8,           // 1 byte: 1=CCM, 2=LBR, 3=LBM, etc.
    pub flags: u8,            // 1 byte: Flags (e.g. RDI bit 0x80, Interval 3 bits)
    pub first_tlv_offset: u8, // Offset to first TLV from after flags
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CcmPdu {
    pub sequence_number: u32,
    pub mep_id: u16,
    pub maid: [u8; 48], // Maintenance Association Identifier (Domain + Short MA Name)
    pub tx_fcf: u32,    // Tx Frame Count Forward (Y.1731 LM)
    pub rx_fcb: u32,    // Rx Frame Count Backward (Y.1731 LM)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfmPacket {
    pub header: CfmHeader,
    pub payload: Vec<u8>,
}

impl CfmPacket {
    pub fn new(header: CfmHeader, payload: Vec<u8>) -> Self {
        CfmPacket { header, payload }
    }

    pub fn build_ccm(md_level: u8, mep_id: u16, seq: u32, maid_str: &str, rdi: bool) -> Self {
        let flags = if rdi { 0x80 } else { 0x00 } | 0x04; // 0x04 = 1 second interval
        let header = CfmHeader {
            md_level,
            version: 0,
            opcode: CFM_OPCODE_CCM,
            flags,
            first_tlv_offset: 70, // 4(seq) + 2(mep) + 48(maid) + 16(y.1731 counters)
        };

        let mut payload = Vec::with_capacity(70);
        payload.extend_from_slice(&seq.to_be_bytes());
        payload.extend_from_slice(&mep_id.to_be_bytes());

        let mut maid_bytes = [0u8; 48];
        let bytes = maid_str.as_bytes();
        let len = bytes.len().min(48);
        maid_bytes[..len].copy_from_slice(&bytes[..len]);
        payload.extend_from_slice(&maid_bytes);

        // Y.1731 TxFCf, RxFCb, TxFCb, Reserved
        payload.extend_from_slice(&[0u8; 16]);

        CfmPacket { header, payload }
    }

    pub fn build_lbm(md_level: u8, trans_id: u32, data_pattern: &[u8]) -> Self {
        let header = CfmHeader {
            md_level,
            version: 0,
            opcode: CFM_OPCODE_LBM,
            flags: 0,
            first_tlv_offset: 4,
        };

        let mut payload = Vec::with_capacity(4 + data_pattern.len());
        payload.extend_from_slice(&trans_id.to_be_bytes());
        payload.extend_from_slice(data_pattern);

        CfmPacket { header, payload }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + self.payload.len() + 1);
        let byte0 = ((self.header.md_level & 0x07) << 5) | (self.header.version & 0x1F);
        buf.push(byte0);
        buf.push(self.header.opcode);
        buf.push(self.header.flags);
        buf.push(self.header.first_tlv_offset);
        buf.extend_from_slice(&self.payload);
        buf.push(0x00); // End TLV
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        let md_level = (data[0] >> 5) & 0x07;
        let version = data[0] & 0x1F;
        let opcode = data[1];
        let flags = data[2];
        let first_tlv_offset = data[3];

        let header = CfmHeader {
            md_level,
            version,
            opcode,
            flags,
            first_tlv_offset,
        };

        let payload = data[4..].to_vec();
        Some(CfmPacket { header, payload })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MepStatus {
    pub mep_id: u16,
    pub last_seq: u32,
    pub rdi: bool,
    pub ccm_count: u32,
}

#[derive(Debug, Clone, Default)]
pub struct CfmEngine {
    pub local_mep_id: u16,
    pub md_level: u8,
    pub maid: String,
    pub remote_meps: std::collections::HashMap<u16, MepStatus>,
}

impl CfmEngine {
    pub fn new(local_mep_id: u16, md_level: u8, maid: &str) -> Self {
        CfmEngine {
            local_mep_id,
            md_level,
            maid: maid.to_string(),
            remote_meps: std::collections::HashMap::new(),
        }
    }

    pub fn process_cfm_frame(&mut self, data: &[u8]) -> Option<CfmPacket> {
        let pkt = CfmPacket::parse(data)?;
        if pkt.header.md_level != self.md_level {
            return None;
        }

        match pkt.header.opcode {
            CFM_OPCODE_CCM => {
                if pkt.payload.len() >= 6 {
                    let seq = u32::from_be_bytes([
                        pkt.payload[0],
                        pkt.payload[1],
                        pkt.payload[2],
                        pkt.payload[3],
                    ]);
                    let mep_id = u16::from_be_bytes([pkt.payload[4], pkt.payload[5]]);
                    let rdi = (pkt.header.flags & 0x80) != 0;

                    let entry = self.remote_meps.entry(mep_id).or_insert(MepStatus {
                        mep_id,
                        last_seq: seq,
                        rdi,
                        ccm_count: 0,
                    });
                    entry.last_seq = seq;
                    entry.rdi = rdi;
                    entry.ccm_count += 1;
                }
                None
            }
            CFM_OPCODE_LBM => {
                // Generate Loopback Reply (LBR)
                let mut reply = pkt.clone();
                reply.header.opcode = CFM_OPCODE_LBR;
                Some(reply)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cfm_ccm_generation_and_processing() {
        let mut engine = CfmEngine::new(10, 4, "carrier.domain.service1");

        let ccm = CfmPacket::build_ccm(4, 20, 1001, "carrier.domain.service1", false);
        let raw = ccm.serialize();

        let resp = engine.process_cfm_frame(&raw);
        assert!(resp.is_none());

        let peer = engine.remote_meps.get(&20).unwrap();
        assert_eq!(peer.mep_id, 20);
        assert_eq!(peer.last_seq, 1001);
        assert_eq!(peer.ccm_count, 1);
        assert!(!peer.rdi);
    }

    #[test]
    fn test_cfm_loopback_lbm_lbr() {
        let mut engine = CfmEngine::new(10, 4, "carrier.domain.service1");
        let lbm = CfmPacket::build_lbm(4, 0x12345678, b"Loopback Test Pattern 12345");
        let raw = lbm.serialize();

        let reply = engine.process_cfm_frame(&raw).unwrap();
        assert_eq!(reply.header.opcode, CFM_OPCODE_LBR);
        assert_eq!(reply.header.md_level, 4);
        assert_eq!(&reply.payload[0..4], &0x12345678u32.to_be_bytes());
    }
}
