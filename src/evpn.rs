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
}

impl fmt::Display for EvpnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvpnError::PacketTooShort(l) => write!(f, "EVPN NLRI too short ({} bytes)", l),
            EvpnError::InvalidRouteType(t) => write!(f, "Unknown EVPN Route Type: {}", t),
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
                if body.len() < 8 + 10 + 4 + 1 + 6 + 1 + 3 {
                    return Err(EvpnError::PacketTooShort(body.len()));
                }
                let rd = RouteDistinguisher::parse(&body[0..8])?;
                let mut esi = [0u8; 10];
                esi.copy_from_slice(&body[8..18]);
                let eth_tag = u32::from_be_bytes([body[18], body[19], body[20], body[21]]);
                let mac = MacAddress([body[23], body[24], body[25], body[26], body[27], body[28]]);

                let ip_len = body[29];
                let (ip, offset) = if ip_len == 32 && body.len() >= 34 {
                    (
                        Some(Ipv4Address([body[30], body[31], body[32], body[33]])),
                        34,
                    )
                } else {
                    (None, 30)
                };

                let vni = u32::from_be_bytes([0, body[offset], body[offset + 1], body[offset + 2]]);

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
                if body.len() < 8 + 4 + 1 + 4 {
                    return Err(EvpnError::PacketTooShort(body.len()));
                }
                let rd = RouteDistinguisher::parse(&body[0..8])?;
                let eth_tag = u32::from_be_bytes([body[8], body[9], body[10], body[11]]);
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
