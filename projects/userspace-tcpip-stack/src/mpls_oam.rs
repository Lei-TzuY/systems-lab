//! MPLS LSP Ping and Traceroute Protocol (RFC 4379 / RFC 8029).
//!
//! Provides data plane verification and failure detection for MPLS / Segment Routing LSPs.

use crate::ipv4::Ipv4Address;

pub const LSP_PING_UDP_PORT: u16 = 3503;

pub const LSP_MSG_ECHO_REQUEST: u8 = 1;
pub const LSP_MSG_ECHO_REPLY: u8 = 2;

pub const LSP_REPLY_MODE_UDP: u8 = 2;
pub const LSP_REPLY_MODE_ROUTER_ALERT: u8 = 3;

pub const LSP_RET_CODE_NO_CODE: u8 = 0;
pub const LSP_RET_CODE_MALFORMED: u8 = 1;
pub const LSP_RET_CODE_UNRECOGNIZED_TLV: u8 = 2;
pub const LSP_RET_CODE_EGRESS_FOR_FEC: u8 = 3;
pub const LSP_RET_CODE_LABEL_SWITCHED: u8 = 8;

pub const LSP_TLV_TARGET_FEC_STACK: u16 = 1;
pub const LSP_TLV_DOWNSTREAM_MAPPING: u16 = 2;
pub const LSP_TLV_PAD: u16 = 3;

pub const FEC_SUBTYPE_IPV4_PREFIX: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFecIpv4 {
    pub prefix: Ipv4Address,
    pub mask_len: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspEchoPacket {
    pub version: u16,
    pub global_flags: u16,
    pub msg_type: u8,
    pub reply_mode: u8,
    pub return_code: u8,
    pub return_subcode: u8,
    pub sender_handle: u32,
    pub seq_number: u32,
    pub timestamp_sent_sec: u32,
    pub timestamp_sent_frac: u32,
    pub timestamp_recv_sec: u32,
    pub timestamp_recv_frac: u32,
    pub target_fec: Option<TargetFecIpv4>,
}

impl LspEchoPacket {
    pub fn build_echo_request(
        sender_handle: u32,
        seq_number: u32,
        fec_prefix: Ipv4Address,
        mask_len: u8,
        sent_sec: u32,
        sent_frac: u32,
    ) -> Self {
        LspEchoPacket {
            version: 1,
            global_flags: 0x0001, // Validate FEC Stack
            msg_type: LSP_MSG_ECHO_REQUEST,
            reply_mode: LSP_REPLY_MODE_UDP,
            return_code: LSP_RET_CODE_NO_CODE,
            return_subcode: 0,
            sender_handle,
            seq_number,
            timestamp_sent_sec: sent_sec,
            timestamp_sent_frac: sent_frac,
            timestamp_recv_sec: 0,
            timestamp_recv_frac: 0,
            target_fec: Some(TargetFecIpv4 {
                prefix: fec_prefix,
                mask_len,
            }),
        }
    }

    pub fn build_echo_reply(
        req: &LspEchoPacket,
        return_code: u8,
        recv_sec: u32,
        recv_frac: u32,
    ) -> Self {
        LspEchoPacket {
            version: 1,
            global_flags: req.global_flags,
            msg_type: LSP_MSG_ECHO_REPLY,
            reply_mode: req.reply_mode,
            return_code,
            return_subcode: 0,
            sender_handle: req.sender_handle,
            seq_number: req.seq_number,
            timestamp_sent_sec: req.timestamp_sent_sec,
            timestamp_sent_frac: req.timestamp_sent_frac,
            timestamp_recv_sec: recv_sec,
            timestamp_recv_frac: recv_frac,
            target_fec: req.target_fec.clone(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(64);
        buf.extend_from_slice(&self.version.to_be_bytes());
        buf.extend_from_slice(&self.global_flags.to_be_bytes());
        buf.push(self.msg_type);
        buf.push(self.reply_mode);
        buf.push(self.return_code);
        buf.push(self.return_subcode);
        buf.extend_from_slice(&self.sender_handle.to_be_bytes());
        buf.extend_from_slice(&self.seq_number.to_be_bytes());
        buf.extend_from_slice(&self.timestamp_sent_sec.to_be_bytes());
        buf.extend_from_slice(&self.timestamp_sent_frac.to_be_bytes());
        buf.extend_from_slice(&self.timestamp_recv_sec.to_be_bytes());
        buf.extend_from_slice(&self.timestamp_recv_frac.to_be_bytes());

        // Target FEC Stack TLV (Type 1)
        if let Some(ref fec) = self.target_fec {
            buf.extend_from_slice(&LSP_TLV_TARGET_FEC_STACK.to_be_bytes());
            buf.extend_from_slice(&12u16.to_be_bytes()); // total TLV value length: 4B sub-header + 8B sub-payload

            // Sub-TLV: IPv4 Prefix (Sub-Type 1)
            buf.extend_from_slice(&FEC_SUBTYPE_IPV4_PREFIX.to_be_bytes());
            buf.extend_from_slice(&8u16.to_be_bytes()); // sub-TLV length
            buf.extend_from_slice(&fec.prefix.0);
            buf.push(fec.mask_len);
            buf.push(0); // Reserved
            buf.push(0);
            buf.push(0);
        }

        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 32 {
            return None;
        }

        let version = u16::from_be_bytes([data[0], data[1]]);
        let global_flags = u16::from_be_bytes([data[2], data[3]]);
        let msg_type = data[4];
        let reply_mode = data[5];
        let return_code = data[6];
        let return_subcode = data[7];
        let sender_handle = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let seq_number = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
        let timestamp_sent_sec = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let timestamp_sent_frac = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        let timestamp_recv_sec = u32::from_be_bytes([data[24], data[25], data[26], data[27]]);
        let timestamp_recv_frac = u32::from_be_bytes([data[28], data[29], data[30], data[31]]);

        let mut target_fec = None;
        let mut offset = 32;

        while offset + 4 <= data.len() {
            let tlv_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let tlv_len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
            offset += 4;

            if offset + tlv_len > data.len() {
                break;
            }

            let tlv_data = &data[offset..offset + tlv_len];
            if tlv_type == LSP_TLV_TARGET_FEC_STACK && tlv_data.len() >= 12 {
                let sub_type = u16::from_be_bytes([tlv_data[0], tlv_data[1]]);
                if sub_type == FEC_SUBTYPE_IPV4_PREFIX {
                    let prefix = Ipv4Address([tlv_data[4], tlv_data[5], tlv_data[6], tlv_data[7]]);
                    let mask_len = tlv_data[8];
                    target_fec = Some(TargetFecIpv4 { prefix, mask_len });
                }
            }

            offset += tlv_len;
        }

        Some(LspEchoPacket {
            version,
            global_flags,
            msg_type,
            reply_mode,
            return_code,
            return_subcode,
            sender_handle,
            seq_number,
            timestamp_sent_sec,
            timestamp_sent_frac,
            timestamp_recv_sec,
            timestamp_recv_frac,
            target_fec,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_echo_request_reply_roundtrip() {
        let req = LspEchoPacket::build_echo_request(
            0x12345678,
            1,
            Ipv4Address::new(10, 0, 0, 1),
            32,
            1700000000,
            500000,
        );
        let raw_req = req.serialize();
        assert!(raw_req.len() >= 32);

        let parsed_req = LspEchoPacket::parse(&raw_req).unwrap();
        assert_eq!(parsed_req.msg_type, LSP_MSG_ECHO_REQUEST);
        assert_eq!(parsed_req.seq_number, 1);
        assert_eq!(parsed_req.sender_handle, 0x12345678);
        assert_eq!(
            parsed_req.target_fec,
            Some(TargetFecIpv4 {
                prefix: Ipv4Address::new(10, 0, 0, 1),
                mask_len: 32,
            })
        );

        let reply = LspEchoPacket::build_echo_reply(
            &parsed_req,
            LSP_RET_CODE_EGRESS_FOR_FEC,
            1700000000,
            501200,
        );
        let raw_rep = reply.serialize();
        let parsed_rep = LspEchoPacket::parse(&raw_rep).unwrap();
        assert_eq!(parsed_rep.msg_type, LSP_MSG_ECHO_REPLY);
        assert_eq!(parsed_rep.return_code, LSP_RET_CODE_EGRESS_FOR_FEC);
        assert_eq!(parsed_rep.timestamp_recv_frac, 501200);
    }
}
