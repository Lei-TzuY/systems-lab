//! BGP Ethernet VPN (EVPN - RFC 7432 / RFC 8365).
//!
//! Control-plane overlay routing for VXLAN/NVGRE data centers without flood-and-learn.

use crate::ethernet::MacAddress;
use crate::ipv4::Ipv4Address;
use std::collections::BTreeMap;
use std::fmt;

pub const BGP_AFI_L2VPN: u16 = 25;
pub const BGP_SAFI_EVPN: u8 = 70;

pub const EVPN_TYPE_ETH_AUTO_DISCOVERY: u8 = 1;
pub const EVPN_TYPE_MAC_IP_ADV: u8 = 2;
pub const EVPN_TYPE_INCLUSIVE_MULTICAST: u8 = 3;
pub const EVPN_TYPE_ETH_SEGMENT: u8 = 4;
pub const EVPN_TYPE_IP_PREFIX: u8 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDistinguisher {
    pub admin: u32,
    pub assigned: u16,
}

impl fmt::Display for RouteDistinguisher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ip = Ipv4Address(self.admin.to_be_bytes());
        write!(f, "{}:{}", ip, self.assigned)
    }
}

impl RouteDistinguisher {
    pub fn new(admin_ip: Ipv4Address, assigned: u16) -> Self {
        RouteDistinguisher {
            admin: u32::from_be_bytes(admin_ip.0),
            assigned,
        }
    }

    pub fn serialize(&self) -> [u8; 8] {
        let mut buf = [0u8; 8];
        buf[0] = 0x00; // Type 1 (IP address based)
        buf[1] = 0x01;
        buf[2..6].copy_from_slice(&self.admin.to_be_bytes());
        buf[6..8].copy_from_slice(&self.assigned.to_be_bytes());
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, EvpnError> {
        if data.len() < 8 {
            return Err(EvpnError::PacketTooShort(data.len()));
        }
        let admin = u32::from_be_bytes([data[2], data[3], data[4], data[5]]);
        let assigned = u16::from_be_bytes([data[6], data[7]]);
        Ok(RouteDistinguisher { admin, assigned })
    }
}

/// EVPN Route Type 2: MAC/IP Advertisement Route
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvpnMacIpAdv {
    pub rd: RouteDistinguisher,
    pub esi: [u8; 10], // Ethernet Segment Identifier
    pub eth_tag: u32,
    pub mac: MacAddress,
    pub ip: Option<Ipv4Address>,
    pub vni: u32, // 24-bit VNI / MPLS Label
}

/// EVPN Route Type 3: Inclusive Multicast Ethernet Tag Route
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvpnInclusiveMulticast {
    pub rd: RouteDistinguisher,
    pub eth_tag: u32,
    pub originating_router_ip: Ipv4Address,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvpnNlri {
    MacIpAdv(EvpnMacIpAdv),
    InclusiveMulticast(EvpnInclusiveMulticast),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvpnError {
    PacketTooShort(usize),
    InvalidRouteType(u8),
    /// MAC Address Length was not the 48 bits RFC 7432 requires.
    InvalidMacLength(u8),
    /// IP Address Length was neither 0, 32, nor 128 bits.
    InvalidIpLength(u8),
}

impl fmt::Display for EvpnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvpnError::PacketTooShort(l) => write!(f, "EVPN NLRI too short ({} bytes)", l),
            EvpnError::InvalidRouteType(t) => write!(f, "Unknown EVPN Route Type: {}", t),
            EvpnError::InvalidMacLength(l) => {
                write!(f, "EVPN MAC Address Length is {} bits, must be 48", l)
            }
            EvpnError::InvalidIpLength(l) => {
                write!(
                    f,
                    "EVPN IP Address Length is {} bits, must be 0, 32 or 128",
                    l
                )
            }
        }
    }
}

impl std::error::Error for EvpnError {}

impl EvpnNlri {
    pub fn build_mac_ip(
        rd: RouteDistinguisher,
        mac: MacAddress,
        ip: Option<Ipv4Address>,
        vni: u32,
    ) -> Self {
        EvpnNlri::MacIpAdv(EvpnMacIpAdv {
            rd,
            esi: [0u8; 10],
            eth_tag: 0,
            mac,
            ip,
            vni: vni & 0x00FF_FFFF,
        })
    }

    pub fn build_inclusive_multicast(
        rd: RouteDistinguisher,
        originating_router_ip: Ipv4Address,
    ) -> Self {
        EvpnNlri::InclusiveMulticast(EvpnInclusiveMulticast {
            rd,
            eth_tag: 0,
            originating_router_ip,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        match self {
            EvpnNlri::MacIpAdv(m) => {
                buf.push(EVPN_TYPE_MAC_IP_ADV);
                let mut body = Vec::new();
                body.extend_from_slice(&m.rd.serialize());
                body.extend_from_slice(&m.esi);
                body.extend_from_slice(&m.eth_tag.to_be_bytes());
                body.push(48); // MAC address length in bits
                body.extend_from_slice(&m.mac.0);

                if let Some(ip) = m.ip {
                    body.push(32); // IP address length in bits
                    body.extend_from_slice(&ip.0);
                } else {
                    body.push(0); // No IP
                }

                let vni_bytes = (m.vni & 0x00FF_FFFF).to_be_bytes();
                body.extend_from_slice(&vni_bytes[1..4]);

                buf.push(body.len() as u8); // NLRI length
                buf.extend_from_slice(&body);
            }
            EvpnNlri::InclusiveMulticast(im) => {
                buf.push(EVPN_TYPE_INCLUSIVE_MULTICAST);
                let mut body = Vec::new();
                body.extend_from_slice(&im.rd.serialize());
                body.extend_from_slice(&im.eth_tag.to_be_bytes());
                body.push(32); // IP length in bits
                body.extend_from_slice(&im.originating_router_ip.0);

                buf.push(body.len() as u8);
                buf.extend_from_slice(&body);
            }
        }
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, EvpnError> {
        if data.len() < 2 {
            return Err(EvpnError::PacketTooShort(data.len()));
        }

        let route_type = data[0];
        let nlri_len = data[1] as usize;

        if data.len() < 2 + nlri_len {
            return Err(EvpnError::PacketTooShort(data.len()));
        }

        let body = &data[2..2 + nlri_len];

        match route_type {
            EVPN_TYPE_MAC_IP_ADV => {
                // RD(8) ESI(10) EthTag(4) MacLen(1) MAC(6) IpLen(1) [IP] Label(3).
                // Every one of those offsets is derived from a length field the
                // sender chose, so each is checked against what actually remains
                // before it is used. The variable-length IP in the middle is what
                // makes that necessary: a route claiming a 32-bit IP inside a body
                // too short to hold one would otherwise index past the end.
                const FIXED_HEAD: usize = 8 + 10 + 4 + 1 + 6 + 1;
                if body.len() < FIXED_HEAD + 3 {
                    return Err(EvpnError::PacketTooShort(body.len()));
                }
                let rd = RouteDistinguisher::parse(&body[0..8])?;
                let mut esi = [0u8; 10];
                esi.copy_from_slice(&body[8..18]);
                let eth_tag = u32::from_be_bytes([body[18], body[19], body[20], body[21]]);

                // RFC 7432 fixes the MAC address length at 48 bits. Anything else
                // would shift every field after it, so it is refused rather than
                // read as if it had been 48.
                if body[22] != 48 {
                    return Err(EvpnError::InvalidMacLength(body[22]));
                }
                let mac = MacAddress([body[23], body[24], body[25], body[26], body[27], body[28]]);

                let ip_len = body[29];
                let ip_octets = match ip_len {
                    0 => 0usize,
                    32 => 4usize,
                    128 => 16usize,
                    other => return Err(EvpnError::InvalidIpLength(other)),
                };
                let ip_end = FIXED_HEAD + ip_octets;
                if body.len() < ip_end + 3 {
                    return Err(EvpnError::PacketTooShort(body.len()));
                }
                // An IPv6 host address has nowhere to live in this IPv4 overlay,
                // so the route is kept as a MAC-only advertisement rather than
                // being dropped: the MAC is still usable for bridging.
                let ip = if ip_octets == 4 {
                    Some(Ipv4Address([
                        body[FIXED_HEAD],
                        body[FIXED_HEAD + 1],
                        body[FIXED_HEAD + 2],
                        body[FIXED_HEAD + 3],
                    ]))
                } else {
                    None
                };

                let vni = u32::from_be_bytes([0, body[ip_end], body[ip_end + 1], body[ip_end + 2]]);

                Ok(EvpnNlri::MacIpAdv(EvpnMacIpAdv {
                    rd,
                    esi,
                    eth_tag,
                    mac,
                    ip,
                    vni,
                }))
            }
            EVPN_TYPE_INCLUSIVE_MULTICAST => {
                // RD(8) EthTag(4) IpLen(1) OriginatingRouterIp(4 or 16).
                if body.len() < 8 + 4 + 1 + 4 {
                    return Err(EvpnError::PacketTooShort(body.len()));
                }
                let rd = RouteDistinguisher::parse(&body[0..8])?;
                let eth_tag = u32::from_be_bytes([body[8], body[9], body[10], body[11]]);
                let ip_octets = match body[12] {
                    32 => 4usize,
                    128 => 16usize,
                    other => return Err(EvpnError::InvalidIpLength(other)),
                };
                if body.len() < 13 + ip_octets {
                    return Err(EvpnError::PacketTooShort(body.len()));
                }
                if ip_octets != 4 {
                    // A Type 3 route whose originating router is an IPv6 address
                    // names a VTEP this IPv4 underlay cannot reach.
                    return Err(EvpnError::InvalidIpLength(body[12]));
                }
                let orig_ip = Ipv4Address([body[13], body[14], body[15], body[16]]);

                Ok(EvpnNlri::InclusiveMulticast(EvpnInclusiveMulticast {
                    rd,
                    eth_tag,
                    originating_router_ip: orig_ip,
                }))
            }
            _ => Err(EvpnError::InvalidRouteType(route_type)),
        }
    }
}

/// Control-plane EVPN MAC-to-VTEP Forwarding Table
#[derive(Debug, Clone, Default)]
pub struct EvpnMacTable {
    pub entries: BTreeMap<(u32, MacAddress), (Ipv4Address, Option<Ipv4Address>)>, // (VNI, MAC) -> (VTEP IP, Host IP)
}

impl EvpnMacTable {
    pub fn new() -> Self {
        EvpnMacTable {
            entries: BTreeMap::new(),
        }
    }

    pub fn learn_route(&mut self, adv: &EvpnMacIpAdv, next_hop_vtep: Ipv4Address) {
        self.entries
            .insert((adv.vni, adv.mac), (next_hop_vtep, adv.ip));
    }

    pub fn lookup(&self, vni: u32, mac: &MacAddress) -> Option<(Ipv4Address, Option<Ipv4Address>)> {
        self.entries.get(&(vni, *mac)).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evpn_mac_ip_adv_and_table_lookup() {
        let rd = RouteDistinguisher::new(Ipv4Address::new(10, 0, 0, 1), 100);
        let host_mac = MacAddress([0x00, 0x50, 0x56, 0xAA, 0xBB, 0xCC]);
        let host_ip = Ipv4Address::new(192, 168, 10, 55);
        let vni = 5001;

        let nlri = EvpnNlri::build_mac_ip(rd.clone(), host_mac, Some(host_ip), vni);
        let raw = nlri.serialize();

        let parsed = EvpnNlri::parse(&raw).unwrap();
        if let EvpnNlri::MacIpAdv(adv) = parsed {
            assert_eq!(adv.rd.to_string(), "10.0.0.1:100");
            assert_eq!(adv.mac, host_mac);
            assert_eq!(adv.ip, Some(host_ip));
            assert_eq!(adv.vni, 5001);

            let mut table = EvpnMacTable::new();
            let vtep_ip = Ipv4Address::new(10, 0, 0, 1);
            table.learn_route(&adv, vtep_ip);

            let res = table.lookup(5001, &host_mac).unwrap();
            assert_eq!(res.0, vtep_ip);
            assert_eq!(res.1, Some(host_ip));
        } else {
            panic!("Expected MAC/IP advertisement");
        }
    }
}
