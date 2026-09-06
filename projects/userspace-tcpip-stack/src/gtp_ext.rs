//! 5G N3 GTP-U Extension Headers & PDU Session Container (3GPP TS 38.415 & TS 29.281).
//!
//! Implements GTP-U User Plane Extension Headers carrying the PDU Session Container,
//! 6-bit QoS Flow Identifier (QFI), and Reflective QoS Indicator (RQI).

pub const GTP_EXT_HDR_PDU_SESSION_CONTAINER: u8 = 0x85;
pub const PDU_SESSION_TYPE_DL: u8 = 0;
pub const PDU_SESSION_TYPE_UL: u8 = 1;

/// 5G PDU Session Container (3GPP TS 38.415)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PduSessionContainer {
    pub pdu_type: u8,    // 0 = DL, 1 = UL
    pub qfi: u8,         // 6-bit QoS Flow Identifier (1..64)
    pub rqi: bool,       // Reflective QoS Indicator (DL only)
    pub ppi: Option<u8>, // Paging Policy Indicator (optional 3-bit)
}

impl PduSessionContainer {
    pub fn new_dl(qfi: u8, rqi: bool) -> Self {
        PduSessionContainer {
            pdu_type: PDU_SESSION_TYPE_DL,
            qfi: qfi & 0x3F,
            rqi,
            ppi: None,
        }
    }

    pub fn new_ul(qfi: u8) -> Self {
        PduSessionContainer {
            pdu_type: PDU_SESSION_TYPE_UL,
            qfi: qfi & 0x3F,
            rqi: false,
            ppi: None,
        }
    }

    /// Serializes PDU Session Container extension header (4 octets standard)
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4);
        buf.push(1); // Extension Header Length in 4-octet units (1 = 4 octets)

        let type_and_flags = (self.pdu_type << 4) | (if self.rqi { 0x01 } else { 0x00 });
        buf.push(type_and_flags);
        buf.push(self.qfi & 0x3F);
        buf.push(0x00); // Next Extension Header Type (0x00 = No more extension headers)
        buf
    }

    /// Parses PDU Session Container extension header
    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < 4 {
            return None;
        }

        let _len_units = buf[0];
        let pdu_type = (buf[1] >> 4) & 0x0F;
        let rqi = (buf[1] & 0x01) != 0;
        let qfi = buf[2] & 0x3F;

        Some(PduSessionContainer {
            pdu_type,
            qfi,
            rqi,
            ppi: None,
        })
    }
}

/// Helper to serialize a full GTP-U packet containing PDU Session Container extension header
pub fn build_gtpu_with_pdu_container(
    teid: u32,
    container: &PduSessionContainer,
    payload: &[u8],
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(16 + payload.len());
    // GTP-U Flags: Flags=0x34 (v1, ProtocolType=1, E=1 (Extension header present))
    buf.push(0x34);
    buf.push(0xFF); // Message Type: G-PDU (0xFF)

    let total_payload_len = 4 + 4 + payload.len(); // 4 (Seq+NPDU+NextExt) + 4 (Ext Hdr) + payload
    buf.extend_from_slice(&(total_payload_len as u16).to_be_bytes());
    buf.extend_from_slice(&teid.to_be_bytes());

    // Optional fields (Seq No = 0, N-PDU = 0, Next Ext Hdr = 0x85)
    buf.push(0x00);
    buf.push(0x00);
    buf.push(0x00);
    buf.push(GTP_EXT_HDR_PDU_SESSION_CONTAINER);

    // Append Extension Header
    buf.extend_from_slice(&container.serialize());
    // Append Inner IP payload
    buf.extend_from_slice(payload);
    buf
}

/// Helper to parse a GTP-U packet and extract TEID, PDU Session Container, and Inner Payload
pub fn parse_gtpu_with_pdu_container(buf: &[u8]) -> Option<(u32, PduSessionContainer, &[u8])> {
    if buf.len() < 16 || buf[1] != 0xFF {
        return None;
    }

    let has_ext = (buf[0] & 0x04) != 0;
    if !has_ext || buf[11] != GTP_EXT_HDR_PDU_SESSION_CONTAINER {
        return None;
    }

    let teid = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let container = PduSessionContainer::parse(&buf[12..16])?;
    let payload = &buf[16..];

    Some((teid, container, payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdu_session_container_serialization_roundtrip() {
        let dl_container = PduSessionContainer::new_dl(9, true); // QFI=9 (Default Internet), RQI=true
        let bytes = dl_container.serialize();
        assert_eq!(bytes.len(), 4);

        let parsed = PduSessionContainer::parse(&bytes).unwrap();
        assert_eq!(parsed.pdu_type, PDU_SESSION_TYPE_DL);
        assert_eq!(parsed.qfi, 9);
        assert!(parsed.rqi);
    }

    #[test]
    fn test_gtpu_with_pdu_session_container_packet_codec() {
        let container = PduSessionContainer::new_dl(1, false); // QFI=1 (URLLC / Voice)
        let inner_ip = vec![0x45, 0x00, 0x00, 0x14, 0x01, 0x02, 0x03, 0x04];
        let packet = build_gtpu_with_pdu_container(0xCAFEBABE, &container, &inner_ip);

        let (teid, parsed_cont, parsed_payload) = parse_gtpu_with_pdu_container(&packet).unwrap();
        assert_eq!(teid, 0xCAFEBABE);
        assert_eq!(parsed_cont.qfi, 1);
        assert_eq!(parsed_payload, inner_ip.as_slice());
    }
}
