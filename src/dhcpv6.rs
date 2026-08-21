//! Dynamic Host Configuration Protocol for IPv6 (DHCPv6 - RFC 8415).
//!
//! Stateful and stateless IPv6 host autoconfiguration over UDP ports 546 (Client) and 547 (Server).

use crate::ipv6::Ipv6Address;
use std::fmt;

pub const DHCPV6_CLIENT_PORT: u16 = 546;
pub const DHCPV6_SERVER_PORT: u16 = 547;
pub const DHCPV6_HEADER_LEN: usize = 4;

// DHCPv6 Message Types
pub const DHCPV6_MSG_SOLICIT: u8 = 1;
pub const DHCPV6_MSG_ADVERTISE: u8 = 2;
pub const DHCPV6_MSG_REQUEST: u8 = 3;
pub const DHCPV6_MSG_CONFIRM: u8 = 4;
pub const DHCPV6_MSG_RENEW: u8 = 5;
pub const DHCPV6_MSG_REBIND: u8 = 6;
pub const DHCPV6_MSG_REPLY: u8 = 7;
pub const DHCPV6_MSG_RELEASE: u8 = 8;
pub const DHCPV6_MSG_DECLINE: u8 = 9;
pub const DHCPV6_MSG_INFO_REQUEST: u8 = 11;

// DHCPv6 Option Codes
pub const DHCPV6_OPT_CLIENTID: u16 = 1;
pub const DHCPV6_OPT_SERVERID: u16 = 2;
pub const DHCPV6_OPT_IA_NA: u16 = 3;
pub const DHCPV6_OPT_IAADDR: u16 = 5;
pub const DHCPV6_OPT_ORO: u16 = 6;
pub const DHCPV6_OPT_ELAPSED_TIME: u16 = 8;
pub const DHCPV6_OPT_STATUS_CODE: u16 = 13;
pub const DHCPV6_OPT_DNS_SERVERS: u16 = 23;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dhcpv6Option {
    pub code: u16,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dhcpv6Message {
    pub msg_type: u8,
    pub transaction_id: u32, // 24-bit integer
    pub options: Vec<Dhcpv6Option>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dhcpv6Error {
    PacketTooShort(usize),
    InvalidLength,
}

impl fmt::Display for Dhcpv6Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Dhcpv6Error::PacketTooShort(l) => write!(f, "DHCPv6 message too short ({} bytes)", l),
            Dhcpv6Error::InvalidLength => write!(f, "Invalid DHCPv6 option length"),
        }
    }
}

impl std::error::Error for Dhcpv6Error {}

impl Dhcpv6Message {
    pub fn build_solicit(transaction_id: u32, client_duid: &[u8]) -> Self {
        let mut options = Vec::new();
        options.push(Dhcpv6Option {
            code: DHCPV6_OPT_CLIENTID,
            data: client_duid.to_vec(),
        });

        // Elapsed time option (0 ms)
        options.push(Dhcpv6Option {
            code: DHCPV6_OPT_ELAPSED_TIME,
            data: vec![0x00, 0x00],
        });

        // Option Request Option (Request DNS Servers 23)
        options.push(Dhcpv6Option {
            code: DHCPV6_OPT_ORO,
            data: 23u16.to_be_bytes().to_vec(),
        });

        Dhcpv6Message {
            msg_type: DHCPV6_MSG_SOLICIT,
            transaction_id: transaction_id & 0x00FF_FFFF,
            options,
        }
    }

    pub fn build_advertise(
        transaction_id: u32,
        client_duid: &[u8],
        server_duid: &[u8],
        assigned_ip: Ipv6Address,
        dns_server: Ipv6Address,
    ) -> Self {
        let mut options = Vec::new();
        options.push(Dhcpv6Option {
            code: DHCPV6_OPT_CLIENTID,
            data: client_duid.to_vec(),
        });
        options.push(Dhcpv6Option {
            code: DHCPV6_OPT_SERVERID,
            data: server_duid.to_vec(),
        });

        // IA_NA (Identity Association for Non-temporary Addresses) option containing IAADDR
        let mut iaaddr_bytes = Vec::new();
        iaaddr_bytes.extend_from_slice(&assigned_ip.0);
        iaaddr_bytes.extend_from_slice(&3600u32.to_be_bytes()); // Preferred Lifetime 1h
        iaaddr_bytes.extend_from_slice(&7200u32.to_be_bytes()); // Valid Lifetime 2h

        let mut ia_na_bytes = Vec::new();
        ia_na_bytes.extend_from_slice(&1u32.to_be_bytes()); // IAID
        ia_na_bytes.extend_from_slice(&1800u32.to_be_bytes()); // T1 (renew)
        ia_na_bytes.extend_from_slice(&2880u32.to_be_bytes()); // T2 (rebind)
        ia_na_bytes.extend_from_slice(&DHCPV6_OPT_IAADDR.to_be_bytes());
        ia_na_bytes.extend_from_slice(&(iaaddr_bytes.len() as u16).to_be_bytes());
        ia_na_bytes.extend_from_slice(&iaaddr_bytes);

        options.push(Dhcpv6Option {
            code: DHCPV6_OPT_IA_NA,
            data: ia_na_bytes,
        });

        // DNS Servers Option
        options.push(Dhcpv6Option {
            code: DHCPV6_OPT_DNS_SERVERS,
            data: dns_server.0.to_vec(),
        });

        Dhcpv6Message {
            msg_type: DHCPV6_MSG_ADVERTISE,
            transaction_id: transaction_id & 0x00FF_FFFF,
            options,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.msg_type);
        let tid_bytes = (self.transaction_id & 0x00FF_FFFF).to_be_bytes();
        buf.extend_from_slice(&tid_bytes[1..4]); // 24-bit TID

        for opt in &self.options {
            buf.extend_from_slice(&opt.code.to_be_bytes());
            buf.extend_from_slice(&(opt.data.len() as u16).to_be_bytes());
            buf.extend_from_slice(&opt.data);
        }

        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, Dhcpv6Error> {
        if data.len() < DHCPV6_HEADER_LEN {
            return Err(Dhcpv6Error::PacketTooShort(data.len()));
        }

        let msg_type = data[0];
        let transaction_id = u32::from_be_bytes([0, data[1], data[2], data[3]]);

        let mut options = Vec::new();
        let mut offset = 4;

        while offset + 4 <= data.len() {
            let code = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;

            if offset + 4 + len > data.len() {
                return Err(Dhcpv6Error::InvalidLength);
            }

            let opt_data = data[offset + 4..offset + 4 + len].to_vec();
            options.push(Dhcpv6Option {
                code,
                data: opt_data,
            });
            offset += 4 + len;
        }

        Ok(Dhcpv6Message {
            msg_type,
            transaction_id,
            options,
        })
    }

    pub fn get_assigned_ipv6(&self) -> Option<Ipv6Address> {
        for opt in &self.options {
            if opt.code == DHCPV6_OPT_IA_NA && opt.data.len() >= 12 + 4 + 16 {
                let sub_opt_code = u16::from_be_bytes([opt.data[12], opt.data[13]]);
                if sub_opt_code == DHCPV6_OPT_IAADDR {
                    let mut ip_bytes = [0u8; 16];
                    ip_bytes.copy_from_slice(&opt.data[16..32]);
                    return Some(Ipv6Address(ip_bytes));
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct Dhcpv6Server {
    pub server_duid: Vec<u8>,
    pub next_ip_suffix: u16,
    pub dns_server: Ipv6Address,
}

impl Default for Dhcpv6Server {
    fn default() -> Self {
        Self::new()
    }
}

impl Dhcpv6Server {
    pub fn new() -> Self {
        Dhcpv6Server {
            server_duid: vec![
                0x00, 0x01, 0x00, 0x01, 0x2A, 0x55, 0x00, 0x50, 0x56, 0x00, 0x00, 0x01,
            ],
            next_ip_suffix: 100,
            dns_server: Ipv6Address([
                0x20, 0x01, 0x48, 0x60, 0x48, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x88, 0x88,
            ]),
        }
    }

    pub fn handle_solicit(&mut self, msg: &Dhcpv6Message) -> Option<Dhcpv6Message> {
        let client_duid = msg
            .options
            .iter()
            .find(|o| o.code == DHCPV6_OPT_CLIENTID)?
            .data
            .clone();

        let mut ip_bytes = [0u8; 16];
        ip_bytes[0] = 0x20;
        ip_bytes[1] = 0x01;
        ip_bytes[2] = 0x0D;
        ip_bytes[3] = 0xB8;
        ip_bytes[14] = (self.next_ip_suffix >> 8) as u8;
        ip_bytes[15] = self.next_ip_suffix as u8;
        self.next_ip_suffix = self.next_ip_suffix.wrapping_add(1);

        Some(Dhcpv6Message::build_advertise(
            msg.transaction_id,
            &client_duid,
            &self.server_duid,
            Ipv6Address(ip_bytes),
            self.dns_server,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_dhcpv6_solicit_and_advertise_roundtrip() {
        let client_duid = vec![0x00, 0x03, 0x00, 0x01, 0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let solicit = Dhcpv6Message::build_solicit(0x123456, &client_duid);
        let raw_solicit = solicit.serialize();

        let parsed_solicit = Dhcpv6Message::parse(&raw_solicit).unwrap();
        assert_eq!(parsed_solicit.msg_type, DHCPV6_MSG_SOLICIT);
        assert_eq!(parsed_solicit.transaction_id, 0x123456);

        let mut server = Dhcpv6Server::new();
        let advertise = server.handle_solicit(&parsed_solicit).unwrap();
        let raw_advertise = advertise.serialize();

        let parsed_advertise = Dhcpv6Message::parse(&raw_advertise).unwrap();
        assert_eq!(parsed_advertise.msg_type, DHCPV6_MSG_ADVERTISE);
        let assigned = parsed_advertise.get_assigned_ipv6().unwrap();
        assert_eq!(assigned, Ipv6Address::from_str("2001:db8::64").unwrap());
    }
}
