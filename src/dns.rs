//! Application Layer: Domain Name System (DNS - RFC 1035).
//!
//! Handles encoding and decoding DNS queries and Type-A IPv4 address responses over UDP port 53.

use crate::ipv4::Ipv4Address;
use std::fmt;

pub const DNS_PORT: u16 = 53;
pub const DNS_TYPE_A: u16 = 1;
pub const DNS_CLASS_IN: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsQuestion {
    pub name: String,
    pub qtype: u16,
    pub qclass: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsAnswer {
    pub name: String,
    pub rtype: u16,
    pub rclass: u16,
    pub ttl: u32,
    pub ip: Ipv4Address,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsMessage {
    pub id: u16,
    pub is_response: bool,
    pub recursion_desired: bool,
    pub recursion_available: bool,
    pub rcode: u8,
    pub questions: Vec<DnsQuestion>,
    pub answers: Vec<DnsAnswer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnsError {
    PacketTooShort(usize),
    InvalidLabel(String),
    UnsupportedFormat,
}

impl fmt::Display for DnsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DnsError::PacketTooShort(len) => {
                write!(f, "DNS packet too short ({} bytes, min 12)", len)
            }
            DnsError::InvalidLabel(l) => write!(f, "Invalid DNS label format: {}", l),
            DnsError::UnsupportedFormat => write!(f, "Unsupported DNS record format"),
        }
    }
}

impl std::error::Error for DnsError {}

impl DnsMessage {
    pub fn build_query(id: u16, hostname: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&id.to_be_bytes());
        // Flags: Standard query, Recursion Desired (RD = 1) -> 0x0100
        buf.extend_from_slice(&0x0100u16.to_be_bytes());
        buf.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT = 1
        buf.extend_from_slice(&0u16.to_be_bytes()); // ANCOUNT = 0
        buf.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT = 0
        buf.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT = 0

        // Encode QNAME: "www.example.com" -> 3www7example3com0
        encode_qname(hostname, &mut buf);

        // QTYPE = A (1), QCLASS = IN (1)
        buf.extend_from_slice(&DNS_TYPE_A.to_be_bytes());
        buf.extend_from_slice(&DNS_CLASS_IN.to_be_bytes());

        buf
    }

    pub fn build_response(id: u16, hostname: &str, ip: Ipv4Address, ttl: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&id.to_be_bytes());
        // Flags: QR=1 (Response), AA=1 (Authoritative), RA=1 -> 0x8180
        buf.extend_from_slice(&0x8180u16.to_be_bytes());
        buf.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT = 1
        buf.extend_from_slice(&1u16.to_be_bytes()); // ANCOUNT = 1
        buf.extend_from_slice(&0u16.to_be_bytes());
        buf.extend_from_slice(&0u16.to_be_bytes());

        // Question section
        let qname_start = buf.len();
        encode_qname(hostname, &mut buf);
        buf.extend_from_slice(&DNS_TYPE_A.to_be_bytes());
        buf.extend_from_slice(&DNS_CLASS_IN.to_be_bytes());

        // Answer section: Name pointer to QNAME (0xC000 | qname_start)
        let ptr = 0xC000u16 | (qname_start as u16);
        buf.extend_from_slice(&ptr.to_be_bytes());
        buf.extend_from_slice(&DNS_TYPE_A.to_be_bytes());
        buf.extend_from_slice(&DNS_CLASS_IN.to_be_bytes());
        buf.extend_from_slice(&ttl.to_be_bytes());
        buf.extend_from_slice(&4u16.to_be_bytes()); // RDLENGTH = 4
        buf.extend_from_slice(&ip.0);

        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, DnsError> {
        if data.len() < 12 {
            return Err(DnsError::PacketTooShort(data.len()));
        }

        let id = u16::from_be_bytes([data[0], data[1]]);
        let flags = u16::from_be_bytes([data[2], data[3]]);
        let is_response = (flags & 0x8000) != 0;
        let recursion_desired = (flags & 0x0100) != 0;
        let recursion_available = (flags & 0x0080) != 0;
        let rcode = (flags & 0x000F) as u8;

        let qdcount = u16::from_be_bytes([data[4], data[5]]);
        let ancount = u16::from_be_bytes([data[6], data[7]]);

        let mut offset = 12;
        let mut questions = Vec::new();

        for _ in 0..qdcount {
            let (name, next_off) = decode_qname(data, offset)?;
            offset = next_off;
            if offset + 4 > data.len() {
                return Err(DnsError::PacketTooShort(data.len()));
            }
            let qtype = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let qclass = u16::from_be_bytes([data[offset + 2], data[offset + 3]]);
            offset += 4;
            questions.push(DnsQuestion {
                name,
                qtype,
                qclass,
            });
        }

        let mut answers = Vec::new();
        for _ in 0..ancount {
            let (name, next_off) = decode_qname(data, offset)?;
            offset = next_off;
            if offset + 10 > data.len() {
                return Err(DnsError::PacketTooShort(data.len()));
            }
            let rtype = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let rclass = u16::from_be_bytes([data[offset + 2], data[offset + 3]]);
            let ttl = u32::from_be_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let rdlength = u16::from_be_bytes([data[offset + 8], data[offset + 9]]) as usize;
            offset += 10;

            if offset + rdlength > data.len() {
                return Err(DnsError::PacketTooShort(data.len()));
            }

            if rtype == DNS_TYPE_A && rdlength == 4 {
                let mut ip_bytes = [0u8; 4];
                ip_bytes.copy_from_slice(&data[offset..offset + 4]);
                answers.push(DnsAnswer {
                    name,
                    rtype,
                    rclass,
                    ttl,
                    ip: Ipv4Address(ip_bytes),
                });
            }
            offset += rdlength;
        }

        Ok(DnsMessage {
            id,
            is_response,
            recursion_desired,
            recursion_available,
            rcode,
            questions,
            answers,
        })
    }
}

fn encode_qname(hostname: &str, buf: &mut Vec<u8>) {
    for label in hostname.split('.') {
        if !label.is_empty() {
            buf.push(label.len() as u8);
            buf.extend_from_slice(label.as_bytes());
        }
    }
    buf.push(0x00);
}

fn decode_qname(data: &[u8], mut offset: usize) -> Result<(String, usize), DnsError> {
    let mut labels = Vec::new();
    let mut jumped = false;
    let mut return_offset = offset;

    let mut hops = 0;
    while offset < data.len() && hops < 20 {
        hops += 1;
        let len = data[offset];
        if len == 0 {
            if !jumped {
                return_offset = offset + 1;
            }
            break;
        }

        // Pointer compression: 0b11xxxxxx
        if (len & 0xC0) == 0xC0 {
            if offset + 1 >= data.len() {
                return Err(DnsError::PacketTooShort(data.len()));
            }
            let ptr_offset = (((len & 0x3F) as usize) << 8) | (data[offset + 1] as usize);
            if !jumped {
                return_offset = offset + 2;
                jumped = true;
            }
            offset = ptr_offset;
            continue;
        }

        offset += 1;
        let end = offset + (len as usize);
        if end > data.len() {
            return Err(DnsError::PacketTooShort(data.len()));
        }

        let label = String::from_utf8_lossy(&data[offset..end]).to_string();
        labels.push(label);
        offset = end;
    }

    Ok((labels.join("."), return_offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_query_and_response_roundtrip() {
        let hostname = "toy-tcpip.org";
        let resolved_ip = Ipv4Address::new(192, 168, 1, 10);
        let id = 0xbeef;

        // 1. Build Query
        let query_bytes = DnsMessage::build_query(id, hostname);
        let query = DnsMessage::parse(&query_bytes).unwrap();
        assert_eq!(query.id, id);
        assert!(!query.is_response);
        assert_eq!(query.questions.len(), 1);
        assert_eq!(query.questions[0].name, hostname);

        // 2. Build Response
        let resp_bytes = DnsMessage::build_response(id, hostname, resolved_ip, 300);
        let resp = DnsMessage::parse(&resp_bytes).unwrap();
        assert_eq!(resp.id, id);
        assert!(resp.is_response);
        assert_eq!(resp.answers.len(), 1);
        assert_eq!(resp.answers[0].ip, resolved_ip);
        assert_eq!(resp.answers[0].ttl, 300);
    }
}
