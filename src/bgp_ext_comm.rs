//! BGP Extended Communities & Color / Tunnel Encapsulation Attributes (RFC 4360 / RFC 7153 / RFC 9012).
//!
//! Implements 8-byte transitive extended communities for VPN route filtering (RT/SOO),
//! SR-TE Policy Color steering (RFC 9012), and underlay tunnel encapsulation signaling.

use crate::ipv4::Ipv4Address;

pub const BGP_EXT_COMM_TYPE_2OCTET_AS: u8 = 0x00;
pub const BGP_EXT_COMM_TYPE_IPV4_ADDR: u8 = 0x01;
pub const BGP_EXT_COMM_TYPE_OPAQUE: u8 = 0x03;

pub const BGP_EXT_COMM_SUBTYPE_ROUTE_TARGET: u8 = 0x02;
pub const BGP_EXT_COMM_SUBTYPE_ROUTE_ORIGIN: u8 = 0x03;
pub const BGP_EXT_COMM_SUBTYPE_COLOR: u8 = 0x0B;
pub const BGP_EXT_COMM_SUBTYPE_TUNNEL_ENCAP: u8 = 0x0C;

pub const TUNNEL_TYPE_VXLAN: u16 = 8;
pub const TUNNEL_TYPE_NVGRE: u16 = 9;
pub const TUNNEL_TYPE_MPLS: u16 = 10;
pub const TUNNEL_TYPE_GENEVE: u16 = 19;
pub const TUNNEL_TYPE_SRV6: u16 = 27;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BgpExtendedCommunity {
    RouteTarget2Octet { asn: u16, value: u32 },
    RouteTargetIpv4 { ip: Ipv4Address, value: u16 },
    RouteOrigin2Octet { asn: u16, value: u32 },
    Color { flags: u16, color: u32 },
    TunnelEncapsulation { tunnel_type: u16 },
    Raw([u8; 8]),
}

impl BgpExtendedCommunity {
    pub fn serialize(&self) -> [u8; 8] {
        let mut buf = [0u8; 8];
        match self {
            BgpExtendedCommunity::RouteTarget2Octet { asn, value } => {
                buf[0] = BGP_EXT_COMM_TYPE_2OCTET_AS;
                buf[1] = BGP_EXT_COMM_SUBTYPE_ROUTE_TARGET;
                buf[2..4].copy_from_slice(&asn.to_be_bytes());
                buf[4..8].copy_from_slice(&value.to_be_bytes());
            }
            BgpExtendedCommunity::RouteTargetIpv4 { ip, value } => {
                buf[0] = BGP_EXT_COMM_TYPE_IPV4_ADDR;
                buf[1] = BGP_EXT_COMM_SUBTYPE_ROUTE_TARGET;
                buf[2..6].copy_from_slice(&ip.0);
                buf[6..8].copy_from_slice(&value.to_be_bytes());
            }
            BgpExtendedCommunity::RouteOrigin2Octet { asn, value } => {
                buf[0] = BGP_EXT_COMM_TYPE_2OCTET_AS;
                buf[1] = BGP_EXT_COMM_SUBTYPE_ROUTE_ORIGIN;
                buf[2..4].copy_from_slice(&asn.to_be_bytes());
                buf[4..8].copy_from_slice(&value.to_be_bytes());
            }
            BgpExtendedCommunity::Color { flags, color } => {
                buf[0] = BGP_EXT_COMM_TYPE_OPAQUE;
                buf[1] = BGP_EXT_COMM_SUBTYPE_COLOR;
                buf[2..4].copy_from_slice(&flags.to_be_bytes());
                buf[4..8].copy_from_slice(&color.to_be_bytes());
            }
            BgpExtendedCommunity::TunnelEncapsulation { tunnel_type } => {
                buf[0] = BGP_EXT_COMM_TYPE_OPAQUE;
                buf[1] = BGP_EXT_COMM_SUBTYPE_TUNNEL_ENCAP;
                buf[6..8].copy_from_slice(&tunnel_type.to_be_bytes());
            }
            BgpExtendedCommunity::Raw(raw) => {
                buf.copy_from_slice(raw);
            }
        }
        buf
    }

    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < 8 {
            return None;
        }

        let type_byte = buf[0];
        let subtype_byte = buf[1];

        match (type_byte, subtype_byte) {
            (BGP_EXT_COMM_TYPE_2OCTET_AS, BGP_EXT_COMM_SUBTYPE_ROUTE_TARGET) => {
                let asn = u16::from_be_bytes([buf[2], buf[3]]);
                let value = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
                Some(BgpExtendedCommunity::RouteTarget2Octet { asn, value })
            }
            (BGP_EXT_COMM_TYPE_IPV4_ADDR, BGP_EXT_COMM_SUBTYPE_ROUTE_TARGET) => {
                let ip = Ipv4Address::new(buf[2], buf[3], buf[4], buf[5]);
                let value = u16::from_be_bytes([buf[6], buf[7]]);
                Some(BgpExtendedCommunity::RouteTargetIpv4 { ip, value })
            }
            (BGP_EXT_COMM_TYPE_2OCTET_AS, BGP_EXT_COMM_SUBTYPE_ROUTE_ORIGIN) => {
                let asn = u16::from_be_bytes([buf[2], buf[3]]);
                let value = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
                Some(BgpExtendedCommunity::RouteOrigin2Octet { asn, value })
            }
            (BGP_EXT_COMM_TYPE_OPAQUE, BGP_EXT_COMM_SUBTYPE_COLOR) => {
                let flags = u16::from_be_bytes([buf[2], buf[3]]);
                let color = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
                Some(BgpExtendedCommunity::Color { flags, color })
            }
            (BGP_EXT_COMM_TYPE_OPAQUE, BGP_EXT_COMM_SUBTYPE_TUNNEL_ENCAP) => {
                let tunnel_type = u16::from_be_bytes([buf[6], buf[7]]);
                Some(BgpExtendedCommunity::TunnelEncapsulation { tunnel_type })
            }
            _ => {
                let mut raw = [0u8; 8];
                raw.copy_from_slice(&buf[0..8]);
                Some(BgpExtendedCommunity::Raw(raw))
            }
        }
    }
}

/// Container for a set of BGP Extended Communities attached to a route
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BgpExtCommunityContainer {
    pub communities: Vec<BgpExtendedCommunity>,
}

impl BgpExtCommunityContainer {
    pub fn new() -> Self {
        BgpExtCommunityContainer {
            communities: Vec::new(),
        }
    }

    pub fn add(&mut self, comm: BgpExtendedCommunity) {
        if !self.communities.contains(&comm) {
            self.communities.push(comm);
        }
    }

    pub fn get_color(&self) -> Option<u32> {
        for comm in &self.communities {
            if let BgpExtendedCommunity::Color { color, .. } = comm {
                return Some(*color);
            }
        }
        None
    }

    pub fn get_tunnel_encap(&self) -> Option<u16> {
        for comm in &self.communities {
            if let BgpExtendedCommunity::TunnelEncapsulation { tunnel_type } = comm {
                return Some(*tunnel_type);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bgp_ext_community_color_and_tunnel_encap() {
        let color_comm = BgpExtendedCommunity::Color { flags: 0, color: 100 };
        let encap_comm = BgpExtendedCommunity::TunnelEncapsulation { tunnel_type: TUNNEL_TYPE_VXLAN };

        let raw_color = color_comm.serialize();
        let parsed_color = BgpExtendedCommunity::parse(&raw_color).unwrap();
        assert_eq!(parsed_color, color_comm);

        let raw_encap = encap_comm.serialize();
        let parsed_encap = BgpExtendedCommunity::parse(&raw_encap).unwrap();
        assert_eq!(parsed_encap, encap_comm);

        let mut container = BgpExtCommunityContainer::new();
        container.add(color_comm);
        container.add(encap_comm);

        assert_eq!(container.get_color(), Some(100));
        assert_eq!(container.get_tunnel_encap(), Some(TUNNEL_TYPE_VXLAN));
    }
}
