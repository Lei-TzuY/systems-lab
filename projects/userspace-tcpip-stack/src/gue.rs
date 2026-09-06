//! Generic UDP Encapsulation (GUE - RFC 7763) & Foo-over-UDP (FOU - RFC 8086).
//!
//! Modern cloud datacenter UDP encapsulation for hardware RSS hashing and multi-protocol tunneling.

use std::fmt;

pub const GUE_UDP_PORT: u16 = 6080;
pub const FOU_UDP_PORT: u16 = 5555;
pub const GUE_HEADER_LEN: usize = 4;

// GUE Protocol Types
pub const GUE_PROTO_IPV4: u8 = 0x04;
pub const GUE_PROTO_IPV6: u8 = 0x29;
pub const GUE_PROTO_GRE: u8 = 0x2F;
pub const GUE_PROTO_ESP: u8 = 0x32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GueHeader {
    pub version: u8,
    pub control: bool,
    pub hlen: u8,
    pub next_proto: u8,
    pub flags: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuePacket {
    pub header: GueHeader,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FouPacket {
    pub proto: u8,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GueError {
    PacketTooShort(usize),
    InvalidVersion(u8),
}

impl fmt::Display for GueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GueError::PacketTooShort(l) => write!(f, "GUE packet too short ({} bytes)", l),
            GueError::InvalidVersion(v) => write!(f, "Unsupported GUE version: {}", v),
        }
    }
}

impl std::error::Error for GueError {}

impl GuePacket {
    pub fn build(next_proto: u8, payload: &[u8]) -> Self {
        GuePacket {
            header: GueHeader {
                version: 0,
                control: false,
                hlen: 0,
                next_proto,
                flags: 0,
            },
            payload: payload.to_vec(),
        }
    }

    pub fn build_ipv4(payload: &[u8]) -> Self {
        Self::build(GUE_PROTO_IPV4, payload)
    }

    pub fn build_ipv6(payload: &[u8]) -> Self {
        Self::build(GUE_PROTO_IPV6, payload)
    }

    pub fn build_gre(payload: &[u8]) -> Self {
        Self::build(GUE_PROTO_GRE, payload)
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut b0 = (self.header.version & 0x03) << 6;
        if self.header.control {
            b0 |= 0x20;
        }
        b0 |= self.header.hlen & 0x1F;
        buf.push(b0);
        buf.push(self.header.next_proto);
        buf.extend_from_slice(&self.header.flags.to_be_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, GueError> {
        if data.len() < GUE_HEADER_LEN {
            return Err(GueError::PacketTooShort(data.len()));
        }

        let version = (data[0] >> 6) & 0x03;
        if version != 0 {
            return Err(GueError::InvalidVersion(version));
        }

        let control = (data[0] & 0x20) != 0;
        let hlen = data[0] & 0x1F;
        let next_proto = data[1];
        let flags = u16::from_be_bytes([data[2], data[3]]);

        let total_header_len = GUE_HEADER_LEN + (hlen as usize * 4);
        if data.len() < total_header_len {
            return Err(GueError::PacketTooShort(data.len()));
        }

        let payload = data[total_header_len..].to_vec();

        Ok(GuePacket {
            header: GueHeader {
                version,
                control,
                hlen,
                next_proto,
                flags,
            },
            payload,
        })
    }
}

impl FouPacket {
    pub fn build_ip(payload: &[u8]) -> Self {
        FouPacket {
            proto: GUE_PROTO_IPV4,
            payload: payload.to_vec(),
        }
    }

    pub fn build_gre(payload: &[u8]) -> Self {
        FouPacket {
            proto: GUE_PROTO_GRE,
            payload: payload.to_vec(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        self.payload.clone()
    }

    pub fn parse(proto: u8, data: &[u8]) -> Self {
        FouPacket {
            proto,
            payload: data.to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gue_ipv4_and_gre_roundtrip() {
        let payload = b"Generic UDP Encapsulated Transit Packet";
        let gue = GuePacket::build_ipv4(payload);
        let raw = gue.serialize();

        assert_eq!(raw.len(), GUE_HEADER_LEN + payload.len());
        let parsed = GuePacket::parse(&raw).unwrap();
        assert_eq!(parsed.header.version, 0);
        assert_eq!(parsed.header.next_proto, GUE_PROTO_IPV4);
        assert_eq!(parsed.payload, payload);
        assert_eq!(GUE_UDP_PORT, 6080);
    }

    #[test]
    fn test_fou_packet_tunneling() {
        let payload = b"Foo-over-UDP direct IP payload";
        let fou = FouPacket::build_ip(payload);
        let raw = fou.serialize();

        let parsed = FouPacket::parse(GUE_PROTO_IPV4, &raw);
        assert_eq!(parsed.proto, GUE_PROTO_IPV4);
        assert_eq!(parsed.payload, payload);
        assert_eq!(FOU_UDP_PORT, 5555);
    }
}
