//! SRv6 Mobile User Plane (SRv6-MUP) & 5G Core UPF Interworking (IETF draft-ietf-dmm-srv6-mobile-uplane).
//!
//! Enables seamless translation and interworking between 3GPP GTP-U (UDP 2152) and
//! Segment Routing over IPv6 (SRv6) mobile user plane functions (`End.M.GTP4.E` and `End.M.GTP4.D`).

use crate::gtp::{GtpPacket, GTP_U_UDP_PORT};
use crate::ipv4::{Ipv4Address, Ipv4Packet, IP_PROTO_UDP};
use crate::ipv6::{Ipv6Address, Ipv6Packet, NEXT_HEADER_ROUTING};
use crate::srv6::Srv6Header;
use crate::tunnel::IP_PROTO_IP_IN_IP;
use crate::udp::UdpDatagram;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Srv6MupSession {
    pub gnb_ipv4: Ipv4Address,
    pub upf_ipv4: Ipv4Address,
    pub teid: u32,
    pub srv6_sid: Ipv6Address,
    pub qfi: u8,
}

#[derive(Debug, Clone, Default)]
pub struct Srv6MupEngine {
    pub uplink_sessions: HashMap<(Ipv4Address, u32), Srv6MupSession>,
    pub downlink_sessions: HashMap<Ipv6Address, Srv6MupSession>,
}

impl Srv6MupEngine {
    pub fn new() -> Self {
        Srv6MupEngine {
            uplink_sessions: HashMap::new(),
            downlink_sessions: HashMap::new(),
        }
    }

    pub fn register_session(&mut self, session: Srv6MupSession) {
        self.uplink_sessions.insert((session.gnb_ipv4, session.teid), session.clone());
        self.downlink_sessions.insert(session.srv6_sid, session);
    }

    /// End.M.GTP4.E: Translates incoming GTP-U/UDP/IPv4 packet into an SRv6 packet
    pub fn process_uplink_gtp_to_srv6(
        &self,
        src_gnb: Ipv4Address,
        teid: u32,
        user_payload: &[u8],
        outer_src_ipv6: Ipv6Address,
    ) -> Option<Vec<u8>> {
        let session = self.uplink_sessions.get(&(src_gnb, teid))?;
        let srh = Srv6Header::build(IP_PROTO_IP_IN_IP, &[session.srv6_sid]);
        let srh_raw = srh.serialize();

        let mut ipv6_payload = Vec::with_capacity(srh_raw.len() + user_payload.len());
        ipv6_payload.extend_from_slice(&srh_raw);
        ipv6_payload.extend_from_slice(user_payload);

        let ipv6_pkt = Ipv6Packet::serialize(
            outer_src_ipv6,
            session.srv6_sid,
            NEXT_HEADER_ROUTING,
            64,
            &ipv6_payload,
        );

        Some(ipv6_pkt)
    }

    /// End.M.GTP4.D: Translates incoming SRv6 packet back into a GTP-U/UDP/IPv4 packet for gNodeB/UPF
    pub fn process_downlink_srv6_to_gtp(
        &self,
        target_sid: Ipv6Address,
        user_payload: &[u8],
        outer_src_ipv4: Ipv4Address,
    ) -> Option<Vec<u8>> {
        let session = self.downlink_sessions.get(&target_sid)?;

        // 1. Build GTP-U Packet
        let gtp_pkt = GtpPacket::build_gpdu(session.teid, user_payload);
        let gtp_raw = gtp_pkt.serialize();

        // 2. Build UDP Datagram (port 2152)
        let udp_bytes = UdpDatagram::serialize(
            outer_src_ipv4,
            session.gnb_ipv4,
            GTP_U_UDP_PORT,
            GTP_U_UDP_PORT,
            &gtp_raw,
        );

        // 3. Build Outer IPv4 Packet
        let ip_bytes = Ipv4Packet::serialize(
            outer_src_ipv4,
            session.gnb_ipv4,
            IP_PROTO_UDP,
            1,
            64,
            &udp_bytes,
        );

        Some(ip_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_srv6_mup_end_m_gtp4_e_and_d_pipeline() {
        let mut engine = Srv6MupEngine::new();

        let gnb_ip = Ipv4Address::new(192, 168, 1, 10);
        let upf_ip = Ipv4Address::new(10, 0, 0, 1);
        let teid = 0x00AABBCC;
        let sid = Ipv6Address::from_str("2001:db8:50:1::1").unwrap();
        let router_v6 = Ipv6Address::from_str("2001:db8:a::1").unwrap();

        engine.register_session(Srv6MupSession {
            gnb_ipv4: gnb_ip,
            upf_ipv4: upf_ip,
            teid,
            srv6_sid: sid,
            qfi: 9,
        });

        let user_data = b"5G NR PDU Session User Data Payload";

        // Uplink: GTP-U -> SRv6
        let srv6_pkt = engine.process_uplink_gtp_to_srv6(gnb_ip, teid, user_data, router_v6).unwrap();
        let parsed_v6 = Ipv6Packet::parse(&srv6_pkt).unwrap();
        assert_eq!(parsed_v6.header.dst_ip, sid);

        // Downlink: SRv6 -> GTP-U
        let gtp_ip_pkt = engine.process_downlink_srv6_to_gtp(sid, user_data, upf_ip).unwrap();
        let parsed_v4 = Ipv4Packet::parse(&gtp_ip_pkt, true).unwrap();
        assert_eq!(parsed_v4.header.dst_ip, gnb_ip);
    }
}
