//! Geneve Extended Metadata & Dynamic In-Band TLV Options (RFC 8926).
//!
//! Provides variable-length cloud virtualization metadata options (OVS/Linux, Cisco, VMware, Amazon)
//! attached to Geneve overlay encapsulation frames.

pub const GENEVE_CLASS_STANDARD: u16 = 0x0100;
pub const GENEVE_CLASS_CISCO: u16 = 0x0101;
pub const GENEVE_CLASS_VMWARE: u16 = 0x0104;
pub const GENEVE_CLASS_OVS_LINUX: u16 = 0x0108;

pub const GENEVE_TYPE_SECURITY_GROUP: u8 = 0x01;
pub const GENEVE_TYPE_INBAND_TELEMETRY: u8 = 0x02;
pub const GENEVE_TYPE_SERVICE_CHAIN: u8 = 0x03;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneveOptionTlv {
    pub class: u16,
    pub type_code: u8,
    pub critical: bool,
    pub data: Vec<u8>,
}

impl GeneveOptionTlv {
    pub fn new(class: u16, type_code: u8, critical: bool, data: &[u8]) -> Self {
        // Pad data to multiple of 4 bytes as required by RFC 8926
        let mut padded = data.to_vec();
        while padded.len() % 4 != 0 {
            padded.push(0);
        }

        GeneveOptionTlv {
            class,
            type_code,
            critical,
            data: padded,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.class.to_be_bytes());

        let type_byte = if self.critical {
            self.type_code | 0x80
        } else {
            self.type_code & 0x7F
        };
        buf.push(type_byte);

        let length_in_4bytes = (self.data.len() / 4) as u8;
        buf.push(length_in_4bytes & 0x1F); // 3 bits reserved (0) + 5 bits length

        buf.extend_from_slice(&self.data);
        buf
    }

    pub fn parse(data: &[u8]) -> Option<(Self, usize)> {
        if data.len() < 4 {
            return None;
        }

        let class = u16::from_be_bytes([data[0], data[1]]);
        let raw_type = data[2];
        let critical = (raw_type & 0x80) != 0;
        let type_code = raw_type & 0x7F;

        let len_4b = (data[3] & 0x1F) as usize;
        let data_len = len_4b * 4;

        if data.len() < 4 + data_len {
            return None;
        }

        let opt_data = data[4..4 + data_len].to_vec();
        let total_consumed = 4 + data_len;

        Some((
            GeneveOptionTlv {
                class,
                type_code,
                critical,
                data: opt_data,
            },
            total_consumed,
        ))
    }

    pub fn parse_all(mut data: &[u8]) -> Vec<Self> {
        let mut list = Vec::new();
        while !data.is_empty() {
            if let Some((opt, consumed)) = Self::parse(data) {
                list.push(opt);
                data = &data[consumed..];
            } else {
                break;
            }
        }
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geneve_option_tlv_roundtrip() {
        let sec_group = GeneveOptionTlv::new(
            GENEVE_CLASS_OVS_LINUX,
            GENEVE_TYPE_SECURITY_GROUP,
            false,
            &[0x00, 0x00, 0x03, 0xE8], // Security Group ID 1000
        );

        let telemetry = GeneveOptionTlv::new(
            GENEVE_CLASS_STANDARD,
            GENEVE_TYPE_INBAND_TELEMETRY,
            true,
            &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
        );

        let mut combined = Vec::new();
        combined.extend_from_slice(&sec_group.serialize());
        combined.extend_from_slice(&telemetry.serialize());

        let parsed = GeneveOptionTlv::parse_all(&combined);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].class, GENEVE_CLASS_OVS_LINUX);
        assert_eq!(parsed[0].type_code, GENEVE_TYPE_SECURITY_GROUP);
        assert_eq!(parsed[0].critical, false);
        assert_eq!(parsed[0].data, vec![0x00, 0x00, 0x03, 0xE8]);

        assert_eq!(parsed[1].class, GENEVE_CLASS_STANDARD);
        assert_eq!(parsed[1].type_code, GENEVE_TYPE_INBAND_TELEMETRY);
        assert_eq!(parsed[1].critical, true);
    }
}
