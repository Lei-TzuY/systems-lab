//! IEEE 802.1BR Bridge Port Extension & E-TAG / VN-Tag (EtherType 0x893F).
//!
//! Provides fabric extender (FEX) port virtualization, 6-byte E-TAG encapsulation,
//! and 20-bit E-CID (Extender Port Identifier) demultiplexing.

use crate::ethernet::MacAddress;

pub const ETHERTYPE_ETAG: u16 = 0x893F;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ETagHeader {
    pub pcp: u8,              // 3 bits: Priority Code Point
    pub dei: bool,            // 1 bit: Drop Eligible Indicator
    pub ingress_e_cid: u32,   // 20 bits: Ingress Extender Port ID
    pub grp: u8,              // 2 bits: Group / Multicast indicator
    pub e_cid: u32,           // 20 bits: Target Extender Port ID
    pub inner_ethertype: u16, // 16 bits: Inner EtherType (e.g. 0x0800 IPv4)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ETagFrame {
    pub dst_mac: MacAddress,
    pub src_mac: MacAddress,
    pub etag: ETagHeader,
    pub payload: Vec<u8>,
}

impl ETagFrame {
    pub fn new(
        dst_mac: MacAddress,
        src_mac: MacAddress,
        etag: ETagHeader,
        payload: Vec<u8>,
    ) -> Self {
        ETagFrame {
            dst_mac,
            src_mac,
            etag,
            payload,
        }
    }

    /// Serializes an IEEE 802.1BR E-TAG frame (12 bytes MAC + 2 bytes EtherType 0x893F + 6 bytes E-TAG + 2 bytes Inner EtherType + Payload)
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(12 + 2 + 6 + 2 + self.payload.len());
        buf.extend_from_slice(&self.dst_mac.0);
        buf.extend_from_slice(&self.src_mac.0);
        buf.extend_from_slice(&ETHERTYPE_ETAG.to_be_bytes());

        // Byte 0: [PCP: 3b][DEI: 1b][Ingress_E-CID_Base High: 4b]
        let ingress_base = (self.etag.ingress_e_cid >> 8) & 0x0FFF;
        let b0 = ((self.etag.pcp & 0x07) << 5)
            | (if self.etag.dei { 0x10 } else { 0x00 })
            | ((ingress_base >> 8) as u8 & 0x0F);
        buf.push(b0);

        // Byte 1: [Ingress_E-CID_Base Low: 8b]
        buf.push((ingress_base & 0xFF) as u8);

        // Byte 2: [Res: 2b][GRP: 2b][E-CID_Base High: 4b]
        let e_cid_base = (self.etag.e_cid >> 8) & 0x0FFF;
        let b2 = ((self.etag.grp & 0x03) << 4) | ((e_cid_base >> 8) as u8 & 0x0F);
        buf.push(b2);

        // Byte 3: [E-CID_Base Low: 8b]
        buf.push((e_cid_base & 0xFF) as u8);

        // Byte 4: [Ingress_E-CID_Ext: 8b]
        buf.push((self.etag.ingress_e_cid & 0xFF) as u8);

        // Byte 5: [E-CID_Ext: 8b]
        buf.push((self.etag.e_cid & 0xFF) as u8);

        // Inner EtherType
        buf.extend_from_slice(&self.etag.inner_ethertype.to_be_bytes());
        buf.extend_from_slice(&self.payload);

        buf
    }

    /// Parses an IEEE 802.1BR E-TAG frame
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 22 {
            return None;
        }

        let mut dst = [0u8; 6];
        dst.copy_from_slice(&data[0..6]);
        let mut src = [0u8; 6];
        src.copy_from_slice(&data[6..12]);

        let etype = u16::from_be_bytes([data[12], data[13]]);
        if etype != ETHERTYPE_ETAG {
            return None;
        }

        let b0 = data[14];
        let pcp = (b0 >> 5) & 0x07;
        let dei = (b0 & 0x10) != 0;
        let ingress_base_hi = (b0 & 0x0F) as u32;
        let ingress_base_lo = data[15] as u32;
        let ingress_base = (ingress_base_hi << 8) | ingress_base_lo;

        let b2 = data[16];
        let grp = (b2 >> 4) & 0x03;
        let e_cid_base_hi = (b2 & 0x0F) as u32;
        let e_cid_base_lo = data[17] as u32;
        let e_cid_base = (e_cid_base_hi << 8) | e_cid_base_lo;

        let ingress_ext = data[18] as u32;
        let e_cid_ext = data[19] as u32;

        let ingress_e_cid = (ingress_base << 8) | ingress_ext;
        let e_cid = (e_cid_base << 8) | e_cid_ext;

        let inner_ethertype = u16::from_be_bytes([data[20], data[21]]);
        let payload = data[22..].to_vec();

        Some(ETagFrame {
            dst_mac: MacAddress(dst),
            src_mac: MacAddress(src),
            etag: ETagHeader {
                pcp,
                dei,
                ingress_e_cid,
                grp,
                e_cid,
                inner_ethertype,
            },
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_etag_frame_roundtrip() {
        let frame = ETagFrame::new(
            MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]),
            MacAddress([0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB]),
            ETagHeader {
                pcp: 5,
                dei: false,
                ingress_e_cid: 0x12345, // 20-bit port ID
                grp: 0,
                e_cid: 0x6789A, // 20-bit target port ID
                inner_ethertype: 0x0800,
            },
            b"IEEE 802.1BR Fabric Extender Payload".to_vec(),
        );

        let raw = frame.serialize();
        assert_eq!(raw.len(), 22 + 36);

        let parsed = ETagFrame::parse(&raw).unwrap();
        assert_eq!(parsed.dst_mac, frame.dst_mac);
        assert_eq!(parsed.src_mac, frame.src_mac);
        assert_eq!(parsed.etag.pcp, 5);
        assert_eq!(parsed.etag.ingress_e_cid, 0x12345);
        assert_eq!(parsed.etag.e_cid, 0x6789A);
        assert_eq!(parsed.etag.inner_ethertype, 0x0800);
        assert_eq!(&parsed.payload, b"IEEE 802.1BR Fabric Extender Payload");
    }
}
