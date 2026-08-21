//! Remote Authentication Dial-In User Service (RADIUS - RFC 2865 / RFC 2866).
//!
//! Network access control, authentication, and accounting over UDP ports 1812 and 1813.

use crate::ipv4::Ipv4Address;
use std::fmt;

pub const RADIUS_AUTH_PORT: u16 = 1812;
pub const RADIUS_ACCT_PORT: u16 = 1813;
pub const RADIUS_HEADER_LEN: usize = 20;

// RADIUS Packet Codes
pub const RADIUS_CODE_ACCESS_REQUEST: u8 = 1;
pub const RADIUS_CODE_ACCESS_ACCEPT: u8 = 2;
pub const RADIUS_CODE_ACCESS_REJECT: u8 = 3;
pub const RADIUS_CODE_ACCOUNTING_REQUEST: u8 = 4;
pub const RADIUS_CODE_ACCOUNTING_RESPONSE: u8 = 5;
pub const RADIUS_CODE_ACCESS_CHALLENGE: u8 = 11;

// Standard Attribute Types
pub const RADIUS_ATTR_USER_NAME: u8 = 1;
pub const RADIUS_ATTR_USER_PASSWORD: u8 = 2;
pub const RADIUS_ATTR_NAS_IP_ADDRESS: u8 = 4;
pub const RADIUS_ATTR_SERVICE_TYPE: u8 = 6;
pub const RADIUS_ATTR_FRAMED_IP_ADDRESS: u8 = 8;
pub const RADIUS_ATTR_REPLY_MESSAGE: u8 = 18;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadiusAvp {
    pub attr_type: u8,
    pub value: Vec<u8>,
}

impl RadiusAvp {
    pub fn new_user_name(name: &str) -> Self {
        RadiusAvp {
            attr_type: RADIUS_ATTR_USER_NAME,
            value: name.as_bytes().to_vec(),
        }
    }

    pub fn new_user_password(
        password: &str,
        secret: &[u8],
        request_authenticator: &[u8; 16],
    ) -> Self {
        // RFC 2865: Password Obfuscation via XOR with hash(secret + authenticator)
        let mut key_stream = [0u8; 16];
        for i in 0..16 {
            let s_byte = secret.get(i % secret.len()).copied().unwrap_or(0);
            key_stream[i] = s_byte ^ request_authenticator[i];
        }

        let pass_bytes = password.as_bytes();
        let mut padded = [0u8; 16];
        let copy_len = pass_bytes.len().min(16);
        padded[..copy_len].copy_from_slice(&pass_bytes[..copy_len]);

        let mut obfuscated = vec![0u8; 16];
        for i in 0..16 {
            obfuscated[i] = padded[i] ^ key_stream[i];
        }

        RadiusAvp {
            attr_type: RADIUS_ATTR_USER_PASSWORD,
            value: obfuscated,
        }
    }

    pub fn new_nas_ip(ip: Ipv4Address) -> Self {
        RadiusAvp {
            attr_type: RADIUS_ATTR_NAS_IP_ADDRESS,
            value: ip.0.to_vec(),
        }
    }

    pub fn new_framed_ip(ip: Ipv4Address) -> Self {
        RadiusAvp {
            attr_type: RADIUS_ATTR_FRAMED_IP_ADDRESS,
            value: ip.0.to_vec(),
        }
    }

    pub fn new_reply_message(msg: &str) -> Self {
        RadiusAvp {
            attr_type: RADIUS_ATTR_REPLY_MESSAGE,
            value: msg.as_bytes().to_vec(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadiusPacket {
    pub code: u8,
    pub identifier: u8,
    pub authenticator: [u8; 16],
    pub attributes: Vec<RadiusAvp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadiusError {
    PacketTooShort(usize),
    LengthMismatch(usize, usize),
    InvalidAvpLength,
}

impl fmt::Display for RadiusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RadiusError::PacketTooShort(l) => {
                write!(f, "RADIUS packet too short ({} bytes, min 20)", l)
            }
            RadiusError::LengthMismatch(hdr, act) => write!(
                f,
                "RADIUS length mismatch: header specifies {}, received {}",
                hdr, act
            ),
            RadiusError::InvalidAvpLength => {
                write!(f, "Invalid RADIUS AVP length field (< 2 or exceeds buffer)")
            }
        }
    }
}

impl std::error::Error for RadiusError {}

impl RadiusPacket {
    pub fn parse(data: &[u8]) -> Result<Self, RadiusError> {
        if data.len() < RADIUS_HEADER_LEN {
            return Err(RadiusError::PacketTooShort(data.len()));
        }

        let code = data[0];
        let identifier = data[1];
        let length = u16::from_be_bytes([data[2], data[3]]) as usize;

        if data.len() < length {
            return Err(RadiusError::LengthMismatch(length, data.len()));
        }

        let mut authenticator = [0u8; 16];
        authenticator.copy_from_slice(&data[4..20]);

        let mut attributes = Vec::new();
        let mut offset = 20;
        while offset < length {
            if offset + 2 > length {
                return Err(RadiusError::InvalidAvpLength);
            }
            let attr_type = data[offset];
            let attr_len = data[offset + 1] as usize;
            if attr_len < 2 || offset + attr_len > length {
                return Err(RadiusError::InvalidAvpLength);
            }

            let value = data[offset + 2..offset + attr_len].to_vec();
            attributes.push(RadiusAvp { attr_type, value });
            offset += attr_len;
        }

        Ok(RadiusPacket {
            code,
            identifier,
            authenticator,
            attributes,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut total_len = RADIUS_HEADER_LEN;
        for avp in &self.attributes {
            total_len += 2 + avp.value.len();
        }

        let mut buf = vec![0u8; total_len];
        buf[0] = self.code;
        buf[1] = self.identifier;
        buf[2..4].copy_from_slice(&(total_len as u16).to_be_bytes());
        buf[4..20].copy_from_slice(&self.authenticator);

        let mut offset = 20;
        for avp in &self.attributes {
            let avp_len = (2 + avp.value.len()) as u8;
            buf[offset] = avp.attr_type;
            buf[offset + 1] = avp_len;
            buf[offset + 2..offset + (avp_len as usize)].copy_from_slice(&avp.value);
            offset += avp_len as usize;
        }

        buf
    }

    pub fn build_access_request(
        id: u8,
        auth: [u8; 16],
        username: &str,
        password: &str,
        secret: &[u8],
        nas_ip: Ipv4Address,
    ) -> Self {
        let attributes = vec![
            RadiusAvp::new_user_name(username),
            RadiusAvp::new_user_password(password, secret, &auth),
            RadiusAvp::new_nas_ip(nas_ip),
        ];

        RadiusPacket {
            code: RADIUS_CODE_ACCESS_REQUEST,
            identifier: id,
            authenticator: auth,
            attributes,
        }
    }

    pub fn build_access_accept(
        id: u8,
        req_auth: [u8; 16],
        framed_ip: Ipv4Address,
        msg: &str,
    ) -> Self {
        let attributes = vec![
            RadiusAvp::new_framed_ip(framed_ip),
            RadiusAvp::new_reply_message(msg),
        ];

        RadiusPacket {
            code: RADIUS_CODE_ACCESS_ACCEPT,
            identifier: id,
            authenticator: req_auth,
            attributes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_radius_access_request_roundtrip() {
        let auth = [0x42; 16];
        let nas_ip = Ipv4Address::new(192, 168, 1, 1);
        let pkt = RadiusPacket::build_access_request(
            1,
            auth,
            "alice",
            "supersecret",
            b"radiuskey",
            nas_ip,
        );

        let raw = pkt.serialize();
        assert!(raw.len() >= RADIUS_HEADER_LEN);

        let parsed = RadiusPacket::parse(&raw).unwrap();
        assert_eq!(parsed.code, RADIUS_CODE_ACCESS_REQUEST);
        assert_eq!(parsed.identifier, 1);
        assert_eq!(parsed.authenticator, auth);
        assert_eq!(parsed.attributes.len(), 3);
        assert_eq!(parsed.attributes[0].attr_type, RADIUS_ATTR_USER_NAME);
        assert_eq!(parsed.attributes[0].value, b"alice");
    }

    #[test]
    fn test_radius_access_accept_attributes() {
        let auth = [0x55; 16];
        let framed_ip = Ipv4Address::new(10, 200, 1, 50);
        let accept =
            RadiusPacket::build_access_accept(1, auth, framed_ip, "Welcome to Corporate VPN!");

        let raw = accept.serialize();
        let parsed = RadiusPacket::parse(&raw).unwrap();
        assert_eq!(parsed.code, RADIUS_CODE_ACCESS_ACCEPT);
        assert_eq!(
            parsed.attributes[0].attr_type,
            RADIUS_ATTR_FRAMED_IP_ADDRESS
        );
        assert_eq!(parsed.attributes[0].value, framed_ip.0);
        assert_eq!(parsed.attributes[1].attr_type, RADIUS_ATTR_REPLY_MESSAGE);
        assert_eq!(parsed.attributes[1].value, b"Welcome to Corporate VPN!");
    }
}
