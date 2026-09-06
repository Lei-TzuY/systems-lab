//! Transport Layer Security (TLS 1.3 - RFC 8446) Record & Handshake Framing.
//!
//! Handles the TLS 5-byte record layer header, Handshake messages (ClientHello, ServerHello, Finished),
//! and encrypted application data encapsulation.

use std::fmt;

pub const TLS_VERSION_1_3_LEGACY: u16 = 0x0303; // TLS 1.2/1.3 wire version

// TLS Content Types
pub const TLS_CONTENT_CHANGE_CIPHER_SPEC: u8 = 20;
pub const TLS_CONTENT_ALERT: u8 = 21;
pub const TLS_CONTENT_HANDSHAKE: u8 = 22;
pub const TLS_CONTENT_APPLICATION_DATA: u8 = 23;

// TLS Handshake Types
pub const TLS_HANDSHAKE_CLIENT_HELLO: u8 = 1;
pub const TLS_HANDSHAKE_SERVER_HELLO: u8 = 2;
pub const TLS_HANDSHAKE_ENCRYPTED_EXTENSIONS: u8 = 8;
pub const TLS_HANDSHAKE_CERTIFICATE: u8 = 11;
pub const TLS_HANDSHAKE_FINISHED: u8 = 20;

// Cipher Suite: TLS_AES_128_GCM_SHA256
pub const TLS_AES_128_GCM_SHA256: u16 = 0x1301;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsRecord {
    pub content_type: u8,
    pub version: u16,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsError {
    RecordTooShort(usize),
    InvalidRecordLength { header_len: usize, available: usize },
    InvalidHandshake,
}

impl fmt::Display for TlsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TlsError::RecordTooShort(len) => {
                write!(f, "TLS record too short ({} bytes, min 5)", len)
            }
            TlsError::InvalidRecordLength {
                header_len,
                available,
            } => {
                write!(
                    f,
                    "TLS record length {} exceeds available buffer {}",
                    header_len, available
                )
            }
            TlsError::InvalidHandshake => write!(f, "Invalid TLS handshake message format"),
        }
    }
}

impl std::error::Error for TlsError {}

impl TlsRecord {
    pub fn new(content_type: u8, payload: Vec<u8>) -> Self {
        TlsRecord {
            content_type,
            version: TLS_VERSION_1_3_LEGACY,
            payload,
        }
    }

    pub fn parse(data: &[u8]) -> Result<Self, TlsError> {
        if data.len() < 5 {
            return Err(TlsError::RecordTooShort(data.len()));
        }

        let content_type = data[0];
        let version = u16::from_be_bytes([data[1], data[2]]);
        let length = u16::from_be_bytes([data[3], data[4]]) as usize;

        if data.len() < 5 + length {
            return Err(TlsError::InvalidRecordLength {
                header_len: length,
                available: data.len() - 5,
            });
        }

        let payload = data[5..5 + length].to_vec();

        Ok(TlsRecord {
            content_type,
            version,
            payload,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let total_len = 5 + self.payload.len();
        let mut buf = Vec::with_capacity(total_len);

        buf.push(self.content_type);
        buf.extend_from_slice(&self.version.to_be_bytes());
        buf.extend_from_slice(&(self.payload.len() as u16).to_be_bytes());
        buf.extend_from_slice(&self.payload);

        buf
    }

    /// Builds a TLS 1.3 ClientHello handshake message
    pub fn build_client_hello(hostname: &str, random_32: [u8; 32]) -> Self {
        let mut hs = Vec::new();
        hs.push(TLS_HANDSHAKE_CLIENT_HELLO);

        let mut body = Vec::new();
        body.extend_from_slice(&TLS_VERSION_1_3_LEGACY.to_be_bytes());
        body.extend_from_slice(&random_32);
        body.push(0); // Session ID length = 0

        // Cipher suites (1 suite = 2 bytes)
        body.extend_from_slice(&2u16.to_be_bytes()); // Length = 2
        body.extend_from_slice(&TLS_AES_128_GCM_SHA256.to_be_bytes());

        // Compression methods (1 method = null 0x00)
        body.push(1); // Length = 1
        body.push(0);

        // Extensions (SNI: Server Name Indication)
        let mut exts = Vec::new();
        // SNI extension (Type 0)
        let sni_len = (hostname.len() + 5) as u16;
        exts.extend_from_slice(&0u16.to_be_bytes()); // Type = 0 (server_name)
        exts.extend_from_slice(&sni_len.to_be_bytes());
        exts.extend_from_slice(&((hostname.len() + 3) as u16).to_be_bytes()); // ServerNameList length
        exts.push(0); // NameType = host_name (0)
        exts.extend_from_slice(&(hostname.len() as u16).to_be_bytes());
        exts.extend_from_slice(hostname.as_bytes());

        // Supported Versions extension (Type 43) -> TLS 1.3 (0x0304)
        exts.extend_from_slice(&43u16.to_be_bytes());
        exts.extend_from_slice(&3u16.to_be_bytes()); // Length = 3
        exts.push(2); // Supported versions list len = 2
        exts.extend_from_slice(&0x0304u16.to_be_bytes());

        body.extend_from_slice(&(exts.len() as u16).to_be_bytes());
        body.extend_from_slice(&exts);

        // 3-byte Handshake length
        let len_24 = body.len() as u32;
        hs.push(((len_24 >> 16) & 0xFF) as u8);
        hs.push(((len_24 >> 8) & 0xFF) as u8);
        hs.push((len_24 & 0xFF) as u8);
        hs.extend_from_slice(&body);

        TlsRecord::new(TLS_CONTENT_HANDSHAKE, hs)
    }

    /// Builds a TLS 1.3 ServerHello handshake message
    pub fn build_server_hello(random_32: [u8; 32]) -> Self {
        let mut hs = Vec::new();
        hs.push(TLS_HANDSHAKE_SERVER_HELLO);

        let mut body = Vec::new();
        body.extend_from_slice(&TLS_VERSION_1_3_LEGACY.to_be_bytes());
        body.extend_from_slice(&random_32);
        body.push(0); // Session ID length = 0
        body.extend_from_slice(&TLS_AES_128_GCM_SHA256.to_be_bytes()); // Selected cipher
        body.push(0); // Selected compression (null)

        // Supported version extension: TLS 1.3 (0x0304)
        let mut exts = Vec::new();
        exts.extend_from_slice(&43u16.to_be_bytes());
        exts.extend_from_slice(&2u16.to_be_bytes());
        exts.extend_from_slice(&0x0304u16.to_be_bytes());

        body.extend_from_slice(&(exts.len() as u16).to_be_bytes());
        body.extend_from_slice(&exts);

        let len_24 = body.len() as u32;
        hs.push(((len_24 >> 16) & 0xFF) as u8);
        hs.push(((len_24 >> 8) & 0xFF) as u8);
        hs.push((len_24 & 0xFF) as u8);
        hs.extend_from_slice(&body);

        TlsRecord::new(TLS_CONTENT_HANDSHAKE, hs)
    }

    /// Wraps plaintext into TLS 1.3 Application Data Record
    pub fn build_application_data(payload: &[u8]) -> Self {
        TlsRecord::new(TLS_CONTENT_APPLICATION_DATA, payload.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_record_roundtrip() {
        let rec = TlsRecord::new(
            TLS_CONTENT_APPLICATION_DATA,
            b"GET / HTTP/1.1\r\n\r\n".to_vec(),
        );
        let raw = rec.serialize();
        let parsed = TlsRecord::parse(&raw).unwrap();

        assert_eq!(parsed.content_type, TLS_CONTENT_APPLICATION_DATA);
        assert_eq!(parsed.version, TLS_VERSION_1_3_LEGACY);
        assert_eq!(parsed.payload, b"GET / HTTP/1.1\r\n\r\n");
    }

    #[test]
    fn test_tls_client_and_server_hello_handshake() {
        let random = [0x42; 32];
        let ch = TlsRecord::build_client_hello("toy-tcpip.org", random);
        assert_eq!(ch.content_type, TLS_CONTENT_HANDSHAKE);

        let parsed_ch = TlsRecord::parse(&ch.serialize()).unwrap();
        assert_eq!(parsed_ch.payload[0], TLS_HANDSHAKE_CLIENT_HELLO);

        let sh = TlsRecord::build_server_hello(random);
        assert_eq!(sh.content_type, TLS_CONTENT_HANDSHAKE);
        let parsed_sh = TlsRecord::parse(&sh.serialize()).unwrap();
        assert_eq!(parsed_sh.payload[0], TLS_HANDSHAKE_SERVER_HELLO);
    }
}
