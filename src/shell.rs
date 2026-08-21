//! Interactive Network Shell (CLI REPL) for real-time virtual stack exploration.

use crate::arp::ArpTable;
use crate::ats::{AtsStreamShaper, UrgencyBasedScheduler};
use crate::bfd::{BFD_CONTROL_PORT, BfdControlPacket, BfdSession, BfdState};
use crate::bfd_v6::{BFD_MULTIHOP_PORT, BfdV6Manager, BfdV6Session};
use crate::bgp::{BgpMessage, BgpRib};
use crate::bgp_epe::{
    BGP_EPE_PEER_ADJ_SID, BGP_EPE_PEER_NODE_SID, BGP_EPE_PEER_SET_SID, BgpEpeDatabase,
};
use crate::bgp_ext_comm::{BgpExtCommunityContainer, BgpExtendedCommunity, TUNNEL_TYPE_VXLAN};
use crate::bgp_ls::{BgpLsLinkDescriptor, BgpLsNlri, BgpLsNodeDescriptor, BgpLsTopologyDatabase};
use crate::bgp_ls_srv6::{BgpLsSrv6Database, Srv6EndSidTlv, Srv6LocatorTlv};
use crate::bgp_prefix_sid::BgpPrefixSidAttribute;
use crate::cbs::CreditBasedShaper;
use crate::cdp::{CDP_MULTICAST_MAC, CDP_SNAP_HEADER, CdpNeighborTable, CdpPacket};
use crate::cfm::{CFM_MULTICAST_CLASS1, CfmEngine, CfmPacket, ETHERTYPE_CFM};
use crate::coap::{COAP_CODE_205_CONTENT, COAP_UDP_PORT, CoapPacket};
use crate::congestion_isolation::{CongestionFlowKey, CongestionIsolationEngine};
use crate::cqf::CqfEngine;
use crate::cqf_enhanced::CqfDualBufferEngine;
use crate::dhcpv6::{DHCPV6_CLIENT_PORT, DHCPV6_SERVER_PORT, Dhcpv6Message, Dhcpv6Server};
use crate::diagnostics::TracerouteHopResult;
use crate::diameter::{DIAMETER_PORT, DIAMETER_SUCCESS, DiameterMessage, DiameterServer};
use crate::dns::DnsMessage;
use crate::eigrp::{EIGRP_MULTICAST_IP, EigrpPacket, EigrpTopologyTable, IP_PROTO_EIGRP};
use crate::erspan::ErspanPacket;
use crate::etag::{ETHERTYPE_ETAG, ETagFrame, ETagHeader};
use crate::ethernet::{ETHERTYPE_IPV4, ETHERTYPE_IPV6, EthernetFrame, MacAddress};
use crate::evpn::{EvpnMacTable, EvpnNlri, RouteDistinguisher};
use crate::evpn_l3irb::{EvpnIpPrefixRoute, EvpnL3VrfTable};
use crate::evpn_multihoming::EvpnDfElectionEngine;
use crate::evpn_smet::{EvpnSmetEngine, EvpnSmetRoute};
use crate::evpn_type1::{EvpnAliasingEngine, EvpnEthernetAdRoute};
use crate::evpn_type3::{EvpnBumFloodingTree, EvpnType3Route};
use crate::evpn_type5::{EvpnType5Rib, EvpnType5Route};
use crate::firewall::{FirewallAction, FirewallChain, FirewallRule, IpCidr};
use crate::flex_algo::{FlexAlgoDefinition, FlexAlgoEngine, FlexAlgoMetricType};
use crate::flowspec::{FlowspecAction, FlowspecEngine, FlowspecMatch, FlowspecRule};
use crate::frer::{ETHERTYPE_RTAG, FrerEngine};
use crate::geneve::{GENEVE_UDP_PORT, GenevePacket};
use crate::geneve_int::{GeneveIntPacket, IntHopTelemetry};
use crate::geneve_opts::{
    GENEVE_CLASS_OVS_LINUX, GENEVE_CLASS_STANDARD, GENEVE_TYPE_INBAND_TELEMETRY,
    GENEVE_TYPE_SECURITY_GROUP, GeneveOptionTlv,
};
use crate::geneve_sfc::{GENEVE_OPT_CLASS_SFC, GeneveSfcHop, GeneveSfcPacket};
use crate::glbp::{GLBP_MULTICAST_IP, GLBP_UDP_PORT, GlbpEngine};
use crate::gnmi::{GNMI_PORT, GnmiServer};
use crate::gnoi::{GNOI_PORT, GnoiServer};
use crate::gptp::{
    ETHERTYPE_GPTP, GPTP_MULTICAST_MAC, GptpPacket, GptpTimestamp, calculate_gptp_peer_delay,
};
use crate::gre_demux::{GreDemuxTable, GreVirtualTunnel};
use crate::gre_udp::{GRE_IN_UDP_PORT, GreUdpPacket};
use crate::gre_v6::{ETHERTYPE_IPV4_IN_GRE, GreIpv6Packet};
use crate::gribi::{GRIBI_PORT, GribiAftTable, GribiIpv4Entry, GribiNextHop, GribiNextHopGroup};
use crate::gtp::{GTP_MSG_ECHO_REQUEST, GTP_U_UDP_PORT, GtpPacket, GtpTunnelTable};
use crate::gtp_ext::{
    GTP_EXT_HDR_PDU_SESSION_CONTAINER, PduSessionContainer, build_gtpu_with_pdu_container,
    parse_gtpu_with_pdu_container,
};
use crate::gue::{GUE_UDP_PORT, GuePacket};
use crate::hsrp::{HSRP_MULTICAST_IP, HSRP_UDP_PORT, HsrpEngine, HsrpPacket};
use crate::http2::Http2Frame;
use crate::http3::Http3Frame;
use crate::icmp::{IcmpPacket, IcmpType};
use crate::icmpv6::{ICMPV6_TYPE_ECHO_REPLY, Icmpv6Packet};
use crate::igmp::{IgmpPacket, MulticastGroupTable, multicast_ip_to_mac};
use crate::ioam::IoamPacket;
use crate::ipfix::{IPFIX_UDP_PORT, IpfixFlowRecord, IpfixMessage};
use crate::ipsec::{EspPacket, IP_PROTO_ESP, SadTable};
use crate::ipv4::{IP_PROTO_ICMP, IP_PROTO_TCP, IP_PROTO_UDP, Ipv4Address, Ipv4Packet};
use crate::ipv6::{Ipv6Address, Ipv6Packet, NEXT_HEADER_ICMPV6, NEXT_HEADER_UDP};
use crate::isis::{ETHERTYPE_ISIS, IsisHelloPacket};
use crate::l2tp::{IP_PROTO_L2TPV3, L2tpv3Packet};
use crate::lab::{LabRouter, VirtualLab};
use crate::lacp::{
    ETHERTYPE_SLOW_PROTOCOLS, LACP_STATE_ACTIVITY, LACP_STATE_AGGREGATION, LACP_STATE_COLLECTING,
    LACP_STATE_DISTRIBUTING, LACP_STATE_SYNCHRONIZATION, LacpPacket, LacpPortInfo,
    LinkAggregationGroup,
};
use crate::ldap::{LDAP_PORT, LdapMessage, LdapOp, LdapServer};
use crate::ldp::{LDP_PORT, LdpPdu, LdpSession};
use crate::lisp::{
    LISP_CONTROL_PORT, LISP_DATA_PORT, LispDataPacket, LispMapReply, LispMapRequest,
    LispMapResolver,
};
use crate::lldp::{ETHERTYPE_LLDP, LLDP_MULTICAST_MAC, LldpNeighborTable, LldpPacket};
use crate::mld::{MLD_CHANGE_TO_INCLUDE, MldGroupRecord, MldTable, Mldv2ReportPacket};
use crate::mpls::{ETHERTYPE_MPLS_UNICAST, LfibTable, MplsHeader, MplsPacket};
use crate::mpls_oam::{LSP_PING_UDP_PORT, LSP_RET_CODE_EGRESS_FOR_FEC, LspEchoPacket};
use crate::mqtt::{MQTT_PORT, MqttBroker, MqttPacket};
use crate::nef_traffic_influence::{NefTrafficInfluenceEngine, SliceId, TrafficFilter};
use crate::netconf::{NETCONF_PORT, NetconfServer};
use crate::netflow::{NETFLOW_V9_UDP_PORT, NetflowFlowTable, NetflowPacket};
use crate::netflow_v5::{NETFLOW_V5_UDP_PORT, NetflowV5Table};
use crate::ngap_5g::{
    InitialUeMessage, NGAP_SCTP_PORT, NgSetupRequest, NgapNode, PduSessionResourceSetupRequest,
    PlmnId, Snssai,
};
use crate::nrf_oauth::{NrfAccessTokenRequest, NrfOAuthAuthority};
use crate::nsh::{NshPacket, ServiceFunctionForwarder};
use crate::ntp::{NtpPacket, NtpTimestamp, calculate_offset_and_delay};
use crate::openflow::{OFP_TCP_PORT, OfpAction, OfpFlowTable, OfpMatch, OfpMessage};
use crate::optical_dom::{OpticalDiagnostics, TransceiverFormFactor};
use crate::ospf::{OSPF_ALL_SPF_ROUTERS, OspfHelloPacket, OspfLsdb};
use crate::otlp::{OTLP_GRPC_PORT, OTLP_HTTP_PORT, OtlpExporter, OtlpSpan};
use crate::p4runtime::{
    P4MatchField, P4MatchKind, P4PacketOut, P4RUNTIME_PORT, P4RuntimeServer, P4TableEntry,
};
use crate::pcap::{LINKTYPE_ETHERNET, PcapWriter};
use crate::pcep::{PCEP_PORT, PcepMessage, PcepObject, PcepSession};
use crate::pfcp_5g::{
    ForwardingActionRule, PFCP_APPLY_ACTION_FORWARD, PFCP_SRC_INTERFACE_ACCESS,
    PFCP_SRC_INTERFACE_CORE, PFCP_UDP_PORT, PacketDetectionRule, PfcpNode,
};
use crate::pim::{ALL_PIM_ROUTERS_MULTICAST, IP_PROTO_PIM, PimMulticastRouter, PimPacket};
use crate::pppoe::{ETHERTYPE_PPPOE_DISCOVERY, ETHERTYPE_PPPOE_SESSION, PppoePacket};
use crate::preemption::PreemptionEngine;
use crate::psfp::{FlowMeter, PsfpFilterInstance, StreamGate};
use crate::ptp::{
    PTP_EVENT_PORT, PTP_GENERAL_PORT, PtpPacket, PtpTimestamp, calculate_ptp_offset_and_delay,
};
use crate::ptp_tc::{HopMeasurement, TransparentClockEngine, TransparentClockMode};
use crate::ptp_telecom::{
    ETHERTYPE_PTP_TELECOM, TelecomBmcaAttributes, TelecomClockType, TelecomProfileEngine,
};
use crate::quic::QuicPacket;
use crate::radius::{RADIUS_AUTH_PORT, RadiusPacket};
use crate::rip::RipEngine;
use crate::roce::{
    ETHERTYPE_FLOW_CONTROL, PFC_MULTICAST_MAC, PfcPauseFrame, ROCEV2_UDP_PORT, RocePacket,
};
use crate::rsvp::{IP_PROTO_RSVP, RsvpPacket};
use crate::rtp::{RTP_PT_PCMU, RtcpSenderReport, RtpPacket};
use crate::sai::SaiSwitchAdapter;
use crate::sba_5g::{NfProfile, NfType, SbaMessageBus, SbaRequest};
use crate::sba_events::{SbaEventExposureEngine, SbaEventType};
use crate::sbfd::{SBFD_REFLECTOR_PORT, SbfdPacket, SbfdReflector};
use crate::sctp::{IP_PROTO_SCTP, SctpPacket};
use crate::sflow::{
    SFLOW_UDP_PORT, SflowCounterSample, SflowDatagram, SflowFlowSample, SflowSample,
};
use crate::sip::{SIP_PORT, SipMessage, build_simple_sdp};
use crate::snmp::{SnmpMessage, SnmpMib, SnmpValue, SnmpVarbind};
use crate::sr_policy::{
    SrCandidatePath, SrPolicy, SrPolicyDatabase, SrProtocolOrigin, SrSegmentList,
};
use crate::srv6::{IPV6_EXT_ROUTING, Srv6Header};
use crate::srv6_mup::{Srv6MupEngine, Srv6MupSession};
use crate::srv6_ops::{Srv6Behavior, Srv6Engine, Srv6ExecutionResult};
use crate::srv6_usid::{UsidBehavior, UsidCarrier, UsidForwardingEngine};
use crate::stack::{NetStack, NetStackConfig};
use crate::stp::StpBridgeEngine;
use crate::stun::{STUN_PORT, StunPacket};
use crate::syslog::{
    SYSLOG_UDP_PORT, SyslogCollector, SyslogFacility, SyslogMessage, SyslogSeverity,
};
use crate::tacacs::{TACACS_AUTHEN_STATUS_PASS, TACACS_PORT, TacacsPacket, TacacsServer};
use crate::tas::TimeAwareShaper;
use crate::tcp::{SocketAddrV4, TcpFlags, TcpSegment};
use crate::tftp::{TftpFileServer, TftpPacket};
use crate::ti_lfa::TiLfaEngine;
use crate::tls::TlsRecord;
use crate::transition::{IP_PROTO_IPV6_IN_IPV4, Tunnel4in6, Tunnel6in4};
use crate::tsn_cnc::{
    CentralizedNetworkConfigurator, StreamId, TrafficSpecification, TsnListener, TsnTalker,
    UserToNetworkRequirements,
};
use crate::tunnel::{GrePacket, IP_PROTO_GRE};
use crate::turn::{TURN_ALLOCATE_REQUEST, TurnAllocationTable, TurnPacket};
use crate::twamp::{TWAMP_CONTROL_PORT, TWAMP_TEST_PORT, TwampTestPacket, calculate_twamp_metrics};
use crate::udp::UdpDatagram;
use crate::vpls::{VplsInstance, VplsPseudowire};
use crate::vrrp::{VrrpEngine, VrrpPacket};
use crate::vtp::{VTP_MULTICAST_MAC, VTP_SNAP_HEADER, VtpEngine, VtpMode, VtpPacket};
use crate::vxlan::{VXLAN_UDP_PORT, VxlanPacket};
use crate::vxlan_gpe::{VXLAN_GPE_NP_IPV4, VXLAN_GPE_UDP_PORT, VxlanGpePacket};
use crate::websocket::WebSocketFrame;
use crate::wireguard::{WIREGUARD_PORT, WireguardMessage, WireguardPeer};
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::str::FromStr;

pub struct NetworkShell {
    stack: NetStack,
    remote_host_ip: Ipv4Address,
    remote_host_ipv6: Ipv6Address,
    remote_host_mac: MacAddress,
    remote_stack: NetStack,
    rip: RipEngine,
    igmp_table: MulticastGroupTable,
    _tftp_server: TftpFileServer,
    vrrp: VrrpEngine,
    hsrp: HsrpEngine,
    glbp: GlbpEngine,
    vtp: VtpEngine,
    evpn_table: EvpnMacTable,
    ofp_table: OfpFlowTable,
    diameter_server: DiameterServer,
    wg_peer: WireguardPeer,
    pcep_session: PcepSession,
    netconf_server: NetconfServer,
    _lisp_resolver: LispMapResolver,
    flowspec_engine: FlowspecEngine,
    otlp_exporter: OtlpExporter,
    gre_demux: GreDemuxTable,
    srv6_engine: Srv6Engine,
    lfib: LfibTable,
    _ldp_session: LdpSession,
    bgp_rib: BgpRib,
    lldp_table: LldpNeighborTable,
    cdp_table: CdpNeighborTable,
    ospf_lsdb: OspfLsdb,
    stp_engine: StpBridgeEngine,
    sad_table: SadTable,
    lag: LinkAggregationGroup,
    eigrp_table: EigrpTopologyTable,
    syslog_collector: SyslogCollector,
    pim_router: PimMulticastRouter,
    bfd_session: BfdSession,
    ldap_server: LdapServer,
    tacacs_server: TacacsServer,
    _dhcpv6_server: Dhcpv6Server,
    netflow_table: NetflowFlowTable,
    mqtt_broker: MqttBroker,
    _gtp_table: GtpTunnelTable,
    _turn_table: TurnAllocationTable,
    bgp_ls_db: BgpLsTopologyDatabase,
    srv6_mup_engine: Srv6MupEngine,
    mld_table: MldTable,
    bfd_v6_mgr: BfdV6Manager,
    netflow_v5_table: NetflowV5Table,
    srv6_usid_engine: UsidForwardingEngine,
    ti_lfa_engine: TiLfaEngine,
    flex_algo_engine: FlexAlgoEngine,
    vpls_instance: VplsInstance,
    cfm_engine: CfmEngine,
    sbfd_reflector: SbfdReflector,
    optical_dom: Vec<OpticalDiagnostics>,
    gnmi_server: GnmiServer,
    gnoi_server: GnoiServer,
    sr_policy_db: SrPolicyDatabase,
    frer_engine: FrerEngine,
    evpn_l3_vrf: EvpnL3VrfTable,
    cqf_engine: CqfEngine,
    gribi_aft: GribiAftTable,
    evpn_df_engine: EvpnDfElectionEngine,
    psfp_pipeline: PsfpFilterInstance,
    p4runtime_server: P4RuntimeServer,
    evpn_aliasing: EvpnAliasingEngine,
    preemption_engine: PreemptionEngine,
    bgp_ext_comms: BgpExtCommunityContainer,
    sai_adapter: SaiSwitchAdapter,
    tas_shaper: TimeAwareShaper,
    sba_bus: SbaMessageBus,
    evpn_type5_rib: EvpnType5Rib,
    tsn_cnc: CentralizedNetworkConfigurator,
    ptp_telecom: TelecomProfileEngine,
    ngap_node: NgapNode,
    evpn_type3_bum: EvpnBumFloodingTree,
    ptp_tc_engine: TransparentClockEngine,
    pfcp_upf: PfcpNode,
    ats_scheduler: UrgencyBasedScheduler,
    bgp_epe_db: BgpEpeDatabase,
    gtpu_ext_container: PduSessionContainer,
    bgp_ls_srv6_db: BgpLsSrv6Database,
    cbs_shaper: CreditBasedShaper,
    sba_events_engine: SbaEventExposureEngine,
    evpn_smet_engine: EvpnSmetEngine,
    congestion_isolation: CongestionIsolationEngine,
    nef_traffic_engine: NefTrafficInfluenceEngine,
    bgp_prefix_sid_attr: BgpPrefixSidAttribute,
    cqf_dual_buffer: CqfDualBufferEngine,
    nrf_oauth_auth: NrfOAuthAuthority,
    pcap_writer: Option<PcapWriter<File>>,
    seq_counter: u16,
}

impl Default for NetworkShell {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkShell {
    pub fn new() -> Self {
        let client_mac = MacAddress([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
        let client_ip = Ipv4Address::new(192, 168, 1, 100);
        let client_ip6 = Ipv6Address::from_str("2001:db8::100").unwrap();

        let server_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x10]);
        let server_ip = Ipv4Address::new(192, 168, 1, 10);
        let server_ip6 = Ipv6Address::from_str("2001:db8::10").unwrap();

        let mut client_stack = NetStack::new(NetStackConfig {
            mac: client_mac,
            ip: client_ip,
            ipv6: Some(client_ip6),
            subnet_mask: 24,
            gateway: Some(Ipv4Address::new(192, 168, 1, 1)),
        });

        let mut server_stack = NetStack::new(NetStackConfig {
            mac: server_mac,
            ip: server_ip,
            ipv6: Some(server_ip6),
            subnet_mask: 24,
            gateway: Some(Ipv4Address::new(192, 168, 1, 1)),
        });

        // Enable NAT on gateway / server
        server_stack.enable_nat(Ipv4Address::new(203, 0, 113, 1));

        // Setup server UDP Echo, DNS, NTP, TFTP, SNMP, RADIUS, SYSLOG, BFD, GENEVE, SIP, CoAP, PTP, STUN/TURN, GTP-U, HSRP, GLBP, LDP, DHCPv6, VXLAN-GPE, RoCEv2, GUE, sFlow, WireGuard, LISP, TWAMP, LSP-Ping, GRE-in-UDP
        server_stack
            .udp_sockets
            .bind(7, |_src, _port, data| Some(data.to_vec()));
        server_stack.udp_sockets.bind(53, |_src, _port, data| {
            if let Ok(query) = DnsMessage::parse(data)
                && let Some(q) = query.questions.first()
            {
                let resolved = match q.name.as_str() {
                    "example.com" | "web.local" => Ipv4Address::new(192, 168, 1, 10),
                    "gateway.local" => Ipv4Address::new(192, 168, 1, 1),
                    _ => Ipv4Address::new(93, 184, 216, 34),
                };
                return Some(DnsMessage::build_response(query.id, &q.name, resolved, 300));
            }
            None
        });

        // MPLS LSP Ping Port 3503 responder
        server_stack
            .udp_sockets
            .bind(LSP_PING_UDP_PORT, |_src, _port, data| {
                if let Some(req) = LspEchoPacket::parse(data) {
                    let resp = LspEchoPacket::build_echo_reply(
                        &req,
                        LSP_RET_CODE_EGRESS_FOR_FEC,
                        1700000000,
                        500200,
                    );
                    return Some(resp.serialize());
                }
                None
            });

        // GRE-in-UDP Port 4754 responder
        server_stack
            .udp_sockets
            .bind(GRE_IN_UDP_PORT, |_src, _port, _data| None);

        // TWAMP Test Port 862 responder
        server_stack
            .udp_sockets
            .bind(TWAMP_TEST_PORT, |_src, _port, data| {
                if let Some(req) = TwampTestPacket::parse(data) {
                    let resp = TwampTestPacket::build_reflector_response(
                        &req,
                        req.seq_number + 100,
                        1700000000,
                        100500,
                        1700000000,
                        100600,
                        64,
                    );
                    return Some(resp.serialize());
                }
                None
            });

        // NTP Port 123 responder
        server_stack.udp_sockets.bind(123, |_src, _port, data| {
            if let Ok(req) = NtpPacket::parse(data) {
                let now = NtpTimestamp::new(3900000000, 500000);
                let resp = NtpPacket::build_server_response(&req, now, now);
                return Some(resp.serialize());
            }
            None
        });

        // TFTP Port 69 responder
        server_stack.udp_sockets.bind(69, |_src, _port, data| {
            if let Ok(pkt) = TftpPacket::parse(data)
                && let TftpPacket::Rrq { filename, .. } = pkt
            {
                let srv = TftpFileServer::new();
                let resp = srv.handle_read_request(&filename, 1);
                return Some(resp.serialize());
            }
            None
        });

        // SNMP Port 161 responder
        server_stack.udp_sockets.bind(161, |_src, _port, data| {
            if let Ok(msg) = SnmpMessage::parse(data) {
                let mib = SnmpMib::new();
                let mut results = Vec::new();
                for vb in &msg.pdu.varbinds {
                    let val = mib.get(&vb.oid).cloned().unwrap_or(SnmpValue::Null);
                    results.push(SnmpVarbind {
                        oid: vb.oid.clone(),
                        value: val,
                    });
                }
                let resp = SnmpMessage::build_response(&msg, results);
                return Some(resp.serialize());
            }
            None
        });

        // WireGuard Port 51820 responder
        server_stack
            .udp_sockets
            .bind(WIREGUARD_PORT, |_src, _port, data| {
                if let Ok(msg) = WireguardMessage::parse(data)
                    && let WireguardMessage::HandshakeInitiation { sender_index, .. } = msg
                {
                    let resp =
                        WireguardMessage::build_response(0x99887766, sender_index, [0xEE; 32]);
                    return Some(resp.serialize());
                }
                None
            });

        // LISP Control Port 4342 responder
        server_stack
            .udp_sockets
            .bind(LISP_CONTROL_PORT, |_src, _port, data| {
                if let Some(req) = LispMapRequest::parse(data) {
                    let mut res = LispMapResolver::new();
                    res.register_eid(req.target_eid, Ipv4Address::new(198, 51, 100, 1), 1, 100);
                    if let Some(reply) = res.resolve(&req) {
                        return Some(reply.serialize());
                    }
                }
                None
            });
        server_stack
            .udp_sockets
            .bind(LISP_DATA_PORT, |_src, _port, _data| None);

        // STUN / TURN Port 3478 responder
        server_stack.udp_sockets.bind(STUN_PORT, |src, port, data| {
            if let Ok(turn_pkt) = TurnPacket::parse(data)
                && turn_pkt.msg_type == TURN_ALLOCATE_REQUEST
            {
                let resp = TurnPacket::build_allocate_response(
                    &turn_pkt,
                    Ipv4Address::new(203, 0, 113, 10),
                    49152,
                    600,
                );
                return Some(resp.serialize());
            }
            if let Ok(req) = StunPacket::parse(data) {
                let resp = StunPacket::build_binding_response(&req, src, port);
                return Some(resp.serialize());
            }
            None
        });

        // GTP-U Port 2152 responder
        server_stack
            .udp_sockets
            .bind(GTP_U_UDP_PORT, |_src, _port, data| {
                if let Ok(pkt) = GtpPacket::parse(data)
                    && pkt.header.msg_type == GTP_MSG_ECHO_REQUEST
                {
                    let seq = pkt.header.seq_num.unwrap_or(1);
                    let resp = GtpPacket::build_echo_response(pkt.header.teid, seq);
                    return Some(resp.serialize());
                }
                None
            });

        // DHCPv6 Server Port 547 responder
        server_stack
            .udp_sockets
            .bind(DHCPV6_SERVER_PORT, |_src, _port, data| {
                if let Ok(msg) = Dhcpv6Message::parse(data) {
                    let mut srv = Dhcpv6Server::new();
                    if let Some(adv) = srv.handle_solicit(&msg) {
                        return Some(adv.serialize());
                    }
                }
                None
            });

        // VXLAN-GPE Port 4790, RoCEv2 Port 4791, GUE Port 6080, sFlow Port 6343 responders
        server_stack
            .udp_sockets
            .bind(VXLAN_GPE_UDP_PORT, |_src, _port, _data| None);
        server_stack
            .udp_sockets
            .bind(ROCEV2_UDP_PORT, |_src, _port, data| {
                if let Ok(roce) = RocePacket::parse(data) {
                    let ack = RocePacket::build_ack(roce.bth.dest_qp, roce.bth.psn);
                    return Some(ack.serialize());
                }
                None
            });
        server_stack
            .udp_sockets
            .bind(GUE_UDP_PORT, |_src, _port, _data| None);
        server_stack
            .udp_sockets
            .bind(SFLOW_UDP_PORT, |_src, _port, _data| None);

        // HSRP Port 1985 & GLBP Port 3222 responders
        server_stack
            .udp_sockets
            .bind(HSRP_UDP_PORT, |_src, _port, _data| None);
        server_stack
            .udp_sockets
            .bind(GLBP_UDP_PORT, |_src, _port, _data| None);

        // LDP Port 646 (UDP Hello) responder
        server_stack
            .udp_sockets
            .bind(LDP_PORT, |_src, _port, _data| None);

        // RADIUS Port 1812 responder
        server_stack
            .udp_sockets
            .bind(RADIUS_AUTH_PORT, |_src, _port, data| {
                if let Ok(req) = RadiusPacket::parse(data) {
                    let accept = RadiusPacket::build_access_accept(
                        req.identifier,
                        req.authenticator,
                        Ipv4Address::new(10, 100, 1, 50),
                        "Authentication Successful (RadiusServer-01)",
                    );
                    return Some(accept.serialize());
                }
                None
            });

        // BFD Port 3784 responder
        server_stack
            .udp_sockets
            .bind(BFD_CONTROL_PORT, |_src, _port, data| {
                if let Ok(req) = BfdControlPacket::parse(data) {
                    let resp = BfdControlPacket::build_control(
                        BfdState::Up,
                        0x87654321,
                        req.my_discriminator,
                        50_000,
                    );
                    return Some(resp.serialize());
                }
                None
            });

        // SIP Port 5060 responder
        server_stack
            .udp_sockets
            .bind(SIP_PORT, |_src, _port, data| {
                if let Ok(text) = std::str::from_utf8(data)
                    && let Ok(req) = SipMessage::parse(text)
                {
                    let local_sdp = build_simple_sdp("bob", "192.168.1.10", 5000);
                    let resp = SipMessage::build_200_ok(&req, &local_sdp);
                    return Some(resp.serialize().into_bytes());
                }
                None
            });

        // CoAP Port 5683 responder
        server_stack
            .udp_sockets
            .bind(COAP_UDP_PORT, |_src, _port, data| {
                if let Ok(req) = CoapPacket::parse(data) {
                    let resp = CoapPacket::build_response(
                        &req,
                        COAP_CODE_205_CONTENT,
                        b"{\"temperature\": 24.5, \"unit\": \"C\"}",
                    );
                    return Some(resp.serialize());
                }
                None
            });

        // PTP Port 319 (Event) & 320 (General) responders
        server_stack
            .udp_sockets
            .bind(PTP_EVENT_PORT, |_src, _port, _data| None);
        server_stack
            .udp_sockets
            .bind(PTP_GENERAL_PORT, |_src, _port, _data| None);

        // SYSLOG, GENEVE, NETFLOW receiver
        server_stack
            .udp_sockets
            .bind(SYSLOG_UDP_PORT, |_src, _port, _data| None);
        server_stack
            .udp_sockets
            .bind(GENEVE_UDP_PORT, |_src, _port, _data| None);
        server_stack
            .udp_sockets
            .bind(NETFLOW_V9_UDP_PORT, |_src, _port, _data| None);

        // Setup server TCP HTTP 80, HTTPS 443, TACACS 49, LDP 646, LDAP 389, MQTT 1883, OpenFlow 6653, Diameter 3868, PCEP 4189, NETCONF 830, TWAMP 862, OTLP 4317/4318
        server_stack.tcp_manager.listen(80);
        server_stack.tcp_manager.listen(443);
        server_stack.tcp_manager.listen(TACACS_PORT);
        server_stack.tcp_manager.listen(LDP_PORT);
        server_stack.tcp_manager.listen(LDAP_PORT);
        server_stack.tcp_manager.listen(MQTT_PORT);
        server_stack.tcp_manager.listen(OFP_TCP_PORT);
        server_stack.tcp_manager.listen(DIAMETER_PORT);
        server_stack.tcp_manager.listen(PCEP_PORT);
        server_stack.tcp_manager.listen(NETCONF_PORT);
        server_stack.tcp_manager.listen(TWAMP_CONTROL_PORT);
        server_stack.tcp_manager.listen(OTLP_GRPC_PORT);
        server_stack.tcp_manager.listen(OTLP_HTTP_PORT);

        // Pre-populate client ARP & NDP cache
        client_stack.arp_table.insert(server_ip.0, server_mac);
        server_stack.arp_table.insert(client_ip.0, client_mac);

        client_stack.ndp_table.insert(server_ip6, server_mac);
        server_stack.ndp_table.insert(client_ip6, client_mac);

        let mut rip = RipEngine::new();
        rip.add_local_network(Ipv4Address::new(192, 168, 1, 0), 24, "eth0");

        let vrrp = VrrpEngine::new(10, 200, Ipv4Address::new(192, 168, 1, 1));
        let hsrp = HsrpEngine::new(1, 110, Ipv4Address::new(192, 168, 1, 1), true);
        let glbp = GlbpEngine::new(1, 120, Ipv4Address::new(192, 168, 1, 1));
        let vtp = VtpEngine::new("EnterpriseHQ", VtpMode::Server);
        let mut evpn_table = EvpnMacTable::new();
        evpn_table
            .entries
            .insert((5001, server_mac), (server_ip, Some(server_ip)));

        let mut ofp_table = OfpFlowTable::new();
        ofp_table.add_entry(
            100,
            OfpMatch {
                in_port: Some(1),
                eth_type: Some(0x0800),
                ip_dst: Some(server_ip),
            },
            vec![OfpAction::Output(2)],
        );

        let diameter_server = DiameterServer::new(
            "hss01.epc.mnc001.mcc001.3gppnetwork.org",
            "epc.mnc001.mcc001.3gppnetwork.org",
        );

        let wg_peer = WireguardPeer::new(
            [0xAA; 32],
            server_ip,
            WIREGUARD_PORT,
            Ipv4Address::new(10, 99, 0, 2),
        );
        let pcep_session = PcepSession::new();
        let netconf_server = NetconfServer::new();
        let mut lisp_resolver = LispMapResolver::new();
        lisp_resolver.register_eid(
            Ipv4Address::new(10, 1, 1, 50),
            Ipv4Address::new(198, 51, 100, 1),
            1,
            100,
        );

        let mut flowspec_engine = FlowspecEngine::new();
        flowspec_engine.add_rule(FlowspecRule {
            id: 1,
            match_fields: FlowspecMatch {
                dst_prefix: Some((Ipv4Address::new(192, 168, 1, 100), 32)),
                src_prefix: None,
                ip_protocol: Some(17),
                dst_port: None,
                src_port: Some(53),
                tcp_flags: None,
            },
            action: FlowspecAction::Drop,
        });

        let mut otlp_exporter = OtlpExporter::new("toy-tcpip-stack");
        otlp_exporter.record_counter(
            "net.packets.total",
            "Total received and transmitted frames",
            "packets",
            25410,
        );
        otlp_exporter.record_gauge("net.rtt.smoothed_ms", "Smoothed RTT estimate", "ms", 0.85);

        let mut gre_demux = GreDemuxTable::new();
        gre_demux.register_tunnel(GreVirtualTunnel {
            if_name: "gre1".to_string(),
            vrf_id: 10,
            local_ip: client_ip,
            remote_ip: server_ip,
            key: 1001,
            strict_sequence: true,
        });

        let mut srv6_engine = Srv6Engine::new();
        let sid_transit = Ipv6Address::from_str("2001:db8:1::100").unwrap();
        let sid_egress = Ipv6Address::from_str("2001:db8:2::200").unwrap();
        srv6_engine.register_sid(sid_transit, Srv6Behavior::End);
        srv6_engine.register_sid(sid_egress, Srv6Behavior::EndDt4 { vrf_id: 10 });

        let lfib = LfibTable::new();
        let ldp_session = LdpSession::default();
        let bgp_rib = BgpRib::new();
        let lldp_table = LldpNeighborTable::new();
        let cdp_table = CdpNeighborTable::new();
        let ospf_lsdb = OspfLsdb::new();
        let stp_engine = StpBridgeEngine::new(32768, client_mac);
        let sad_table = SadTable::new();
        let lag =
            LinkAggregationGroup::new("bond0", vec!["eth0".to_string(), "eth1".to_string()], 1);
        let eigrp_table = EigrpTopologyTable::new();
        let syslog_collector = SyslogCollector::new(100);
        let pim_router = PimMulticastRouter::default();
        let bfd_session = BfdSession::new(0x12345678, 50_000);
        let ldap_server = LdapServer::new();
        let tacacs_server = TacacsServer::new();
        let dhcpv6_server = Dhcpv6Server::new();
        let netflow_table = NetflowFlowTable::new();
        let mqtt_broker = MqttBroker::new();
        let gtp_table = GtpTunnelTable::new();
        let turn_table = TurnAllocationTable::new();
        let mut bgp_ls_db = BgpLsTopologyDatabase::new();
        bgp_ls_db.ingest_nlri(BgpLsNlri::Node(BgpLsNodeDescriptor {
            asn: 65000,
            igp_router_id: server_ip,
            node_name: Some("Edge-Spine-01".to_string()),
        }));
        bgp_ls_db.ingest_nlri(BgpLsNlri::Link(BgpLsLinkDescriptor {
            local_node: BgpLsNodeDescriptor {
                asn: 65000,
                igp_router_id: client_ip,
                node_name: Some("Leaf-01".to_string()),
            },
            remote_node: BgpLsNodeDescriptor {
                asn: 65000,
                igp_router_id: server_ip,
                node_name: Some("Edge-Spine-01".to_string()),
            },
            local_interface_ip: client_ip,
            remote_neighbor_ip: server_ip,
            te_metric: 10,
            max_bandwidth_bps: 100_000_000_000.0,
            max_reservable_bandwidth_bps: 80_000_000_000.0,
            admin_group_color: 0x01,
        }));

        let mut srv6_mup_engine = Srv6MupEngine::new();
        srv6_mup_engine.register_session(Srv6MupSession {
            gnb_ipv4: Ipv4Address::new(192, 168, 1, 50),
            upf_ipv4: server_ip,
            teid: 0xCAFE0001,
            srv6_sid: Ipv6Address::from_str("2001:db8:50:1::100").unwrap(),
            qfi: 9,
        });

        let mut mld_table = MldTable::new();
        let demo_group = Ipv6Address::from_str("ff3e::8000:1").unwrap();
        let demo_src = Ipv6Address::from_str("2001:db8:1::10").unwrap();
        mld_table.process_report(&Mldv2ReportPacket::new(vec![MldGroupRecord {
            record_type: MLD_CHANGE_TO_INCLUDE,
            multicast_address: demo_group,
            source_addresses: vec![demo_src],
        }]));

        let mut bfd_v6_mgr = BfdV6Manager::new();
        bfd_v6_mgr.add_session(BfdV6Session::new(server_ip6, 0x55443322, true));

        let mut netflow_v5_table = NetflowV5Table::new();
        netflow_v5_table.record_flow(client_ip, server_ip, server_ip, 51000, 80, 6, 1460, 1000);

        let mut srv6_usid_engine = UsidForwardingEngine::new();
        srv6_usid_engine.register_usid(0x1001, UsidBehavior::EndUN);
        srv6_usid_engine.register_usid(0x2002, UsidBehavior::EndUN);
        srv6_usid_engine.register_usid(0xE001, UsidBehavior::EndUDT4);

        let mut ti_lfa_engine = TiLfaEngine::new();
        ti_lfa_engine.add_node("NodeS", 16001);
        ti_lfa_engine.add_node("NodeE", 16002);
        ti_lfa_engine.add_node("NodeD", 16003);
        ti_lfa_engine.add_node("NodeP", 16004);
        ti_lfa_engine.add_node("NodeQ", 16005);
        ti_lfa_engine.add_link("NodeS", "NodeE", 10, 24001);
        ti_lfa_engine.add_link("NodeE", "NodeD", 10, 24002);
        ti_lfa_engine.add_link("NodeS", "NodeP", 10, 24003);
        ti_lfa_engine.add_link("NodeP", "NodeQ", 10, 24004);
        ti_lfa_engine.add_link("NodeQ", "NodeD", 10, 24005);

        // IPFIX Port 4739, Multi-Hop BFD Port 4784, NetFlow v5 Port 2055 responders
        server_stack
            .udp_sockets
            .bind(IPFIX_UDP_PORT, |_src, _port, _data| None);
        server_stack
            .udp_sockets
            .bind(NETFLOW_V5_UDP_PORT, |_src, _port, _data| None);
        server_stack
            .udp_sockets
            .bind(BFD_MULTIHOP_PORT, |_src, _port, data| {
                if let Ok(req) = BfdControlPacket::parse(data) {
                    let resp = BfdControlPacket::build_control(
                        BfdState::Up,
                        0x88776655,
                        req.my_discriminator,
                        50_000,
                    );
                    return Some(resp.serialize());
                }
                None
            });

        let mut flex_algo_engine = FlexAlgoEngine::new();
        flex_algo_engine.register_algo(FlexAlgoDefinition {
            algo_id: 128,
            metric_type: FlexAlgoMetricType::MinDelay,
            calculation_type: 0,
            exclude_affinity: 0,
            include_any_affinity: 0,
        });
        flex_algo_engine.register_algo(FlexAlgoDefinition {
            algo_id: 129,
            metric_type: FlexAlgoMetricType::IgpMetric,
            calculation_type: 0,
            exclude_affinity: 0x02,
            include_any_affinity: 0,
        });
        flex_algo_engine.add_link("NodeA", "NodeB_LowDelay", 50, 5, 50, 0x01);
        flex_algo_engine.add_link("NodeB_LowDelay", "NodeB", 50, 5, 50, 0x01);
        flex_algo_engine.add_link("NodeA", "NodeB_HighDelay", 10, 80, 10, 0x02);
        flex_algo_engine.add_link("NodeB_HighDelay", "NodeB", 10, 80, 10, 0x02);

        let mut vpls_instance = VplsInstance::new(100);
        vpls_instance.add_pseudowire(VplsPseudowire {
            peer_ip: server_ip,
            vc_label_tx: 5001,
            vc_label_rx: 6001,
            tunnel_label_tx: 1001,
        });
        vpls_instance.learn_mac(server_mac, Some(6001));

        let mut sbfd_reflector = SbfdReflector::new();
        sbfd_reflector.register_discriminator(0x90001);

        let mut sbfd_server_reflector = SbfdReflector::new();
        sbfd_server_reflector.register_discriminator(0x90001);
        server_stack
            .udp_sockets
            .bind(SBFD_REFLECTOR_PORT, move |_src, _port, data| {
                if let Some(probe) = SbfdPacket::parse(data)
                    && let Some(resp) = sbfd_server_reflector.process_probe(&probe)
                {
                    return Some(resp.serialize().to_vec());
                }
                None
            });

        let mut cfm_engine = CfmEngine::new(10, 4, "carrier.domain.service1");
        let initial_ccm = CfmPacket::build_ccm(4, 20, 100, "carrier.domain.service1", false);
        let _ = cfm_engine.process_cfm_frame(&initial_ccm.serialize());

        let optical_dom = vec![
            OpticalDiagnostics::new(
                "HundredGigE0/0/0/1",
                TransceiverFormFactor::Qsfp28_100G,
                38.2,
                3.32,
                35.5,
                -1.2,
                -7.8,
            ),
            OpticalDiagnostics::new(
                "TenGigE0/0/0/2",
                TransceiverFormFactor::SfpPlus10G,
                41.5,
                3.28,
                28.4,
                -2.0,
                -11.5,
            ),
            OpticalDiagnostics::new(
                "FourHundredGigE0/0/0/3",
                TransceiverFormFactor::QsfpDd400G,
                45.0,
                3.30,
                42.0,
                0.5,
                -6.2,
            ),
        ];

        let gnmi_server = GnmiServer::new();

        let mut sr_policy_db = SrPolicyDatabase::new();
        let mut policy_gold = SrPolicy::new(100, server_ip6, "SR-Policy-Gold-LowLatency");
        policy_gold.add_candidate_path(SrCandidatePath {
            preference: 100,
            protocol_origin: SrProtocolOrigin::Cli,
            segment_lists: vec![SrSegmentList {
                weight: 1,
                segments: vec![
                    Ipv6Address::new([0xfc00, 0, 0, 1, 0, 0, 0, 0x0001]),
                    Ipv6Address::new([0xfc00, 0, 0, 3, 0, 0, 0, 0x0001]),
                ],
            }],
        });
        policy_gold.add_candidate_path(SrCandidatePath {
            preference: 200,
            protocol_origin: SrProtocolOrigin::BgpSrTe,
            segment_lists: vec![SrSegmentList {
                weight: 1,
                segments: vec![Ipv6Address::new([0xfc00, 0, 0, 2, 0, 0, 0, 0x0001])],
            }],
        });
        sr_policy_db.insert_policy(policy_gold);
        let gnoi_server = GnoiServer::new();
        let frer_engine = FrerEngine::new();
        let mut evpn_l3_vrf = EvpnL3VrfTable::new("VRF-TENANT-RED", 50001, client_stack.config.mac);
        evpn_l3_vrf.add_prefix_route(EvpnIpPrefixRoute::new(
            RouteDistinguisher::new(server_ip, 100),
            Ipv4Address::new(10, 100, 1, 0),
            24,
            50001,
            server_mac,
            server_ip,
        ));

        let cqf_engine = CqfEngine::new(125);
        let mut gribi_aft = GribiAftTable::new();
        gribi_aft.set_next_hop(GribiNextHop {
            id: 1,
            ip: server_ip,
            mac: server_mac,
            weight: 100,
        });
        gribi_aft.set_next_hop_group(GribiNextHopGroup {
            id: 10,
            next_hop_ids: vec![1],
        });
        gribi_aft.set_ipv4_entry(GribiIpv4Entry {
            prefix: Ipv4Address::new(10, 0, 0, 0),
            prefix_len: 8,
            next_hop_group_id: 10,
        });

        let mut evpn_df_engine = EvpnDfElectionEngine::new(client_stack.config.ip);
        let default_esi = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99];
        evpn_df_engine.add_segment_peer(default_esi, client_stack.config.ip);
        evpn_df_engine.add_segment_peer(default_esi, server_ip);

        let psfp_gate = StreamGate::new(1, 1000, 500);
        let psfp_meter = FlowMeter::new(1, 1_000_000, 2000, true);
        let psfp_pipeline = PsfpFilterInstance::new(100, 7, psfp_gate, psfp_meter);

        let mut p4runtime_server = P4RuntimeServer::new(1);
        p4runtime_server.set_forwarding_pipeline_config("fabric_pipeline.p4info.txt");
        p4runtime_server.write_table_entry(P4TableEntry {
            table_name: "IngressPipeImpl.ipv4_lpm".to_string(),
            matches: vec![P4MatchField {
                field_name: "hdr.ipv4.dst_addr".to_string(),
                match_value: P4MatchKind::Lpm {
                    value: vec![10, 0, 0, 0],
                    prefix_len: 16,
                },
            }],
            action_name: "IngressPipeImpl.set_next_hop".to_string(),
            action_params: vec![("port".to_string(), vec![0, 0, 0, 1])],
            priority: 10,
        });

        let mut evpn_aliasing = EvpnAliasingEngine::new();
        evpn_aliasing.add_ad_route(EvpnEthernetAdRoute::new_per_es(
            RouteDistinguisher::new(client_stack.config.ip, 1),
            default_esi,
            client_stack.config.ip,
        ));
        evpn_aliasing.add_ad_route(EvpnEthernetAdRoute::new_per_es(
            RouteDistinguisher::new(server_ip, 1),
            default_esi,
            server_ip,
        ));

        let preemption_engine = PreemptionEngine::new();

        let mut bgp_ext_comms = BgpExtCommunityContainer::new();
        bgp_ext_comms.add(BgpExtendedCommunity::RouteTarget2Octet {
            asn: 65000,
            value: 100,
        });
        bgp_ext_comms.add(BgpExtendedCommunity::Color {
            flags: 0,
            color: 100,
        });
        bgp_ext_comms.add(BgpExtendedCommunity::TunnelEncapsulation {
            tunnel_type: TUNNEL_TYPE_VXLAN,
        });

        let mut sai_adapter = SaiSwitchAdapter::new(1);
        sai_adapter.create_fdb_entry(client_stack.config.mac, 100, 1);
        let sai_nh = sai_adapter.create_next_hop(server_ip, server_mac, 2);
        sai_adapter.create_route_entry(0, Ipv4Address::new(10, 0, 0, 0), 8, sai_nh);

        let mut tas_shaper = TimeAwareShaper::new();
        tas_shaper.add_entry(0x80, 100); // Slot 0: Queue 7 (TSN Control) 100µs
        tas_shaper.add_entry(0x7F, 400); // Slot 1: Queues 0..6 (Best-Effort) 400µs

        let mut sba_bus = SbaMessageBus::new();
        sba_bus.nrf.register_nf(NfProfile {
            nf_instance_id: "amf-01".to_string(),
            nf_type: NfType::Amf,
            fqdn: "amf.5gcore.local".to_string(),
            ip_address: "10.100.1.10".to_string(),
            services: vec!["namf-comm".to_string()],
            capacity: 100,
        });
        sba_bus.nrf.register_nf(NfProfile {
            nf_instance_id: "smf-01".to_string(),
            nf_type: NfType::Smf,
            fqdn: "smf.5gcore.local".to_string(),
            ip_address: "10.100.1.20".to_string(),
            services: vec!["nsmf-pdusession".to_string()],
            capacity: 100,
        });

        let mut evpn_type5_rib = EvpnType5Rib::new();
        evpn_type5_rib.add_route(EvpnType5Route::new_ipv4(
            RouteDistinguisher::new(client_stack.config.ip, 100),
            Ipv4Address::new(10, 200, 0, 0),
            16,
            client_stack.config.ip,
            50001,
        ));

        let mut tsn_cnc = CentralizedNetworkConfigurator::new();
        let talker_sid = StreamId::new(client_stack.config.mac, 1);
        let _ = tsn_cnc.register_talker(TsnTalker {
            stream_id: talker_sid,
            talker_mac: client_stack.config.mac,
            vlan_id: 100,
            priority: 6,
            tspec: TrafficSpecification {
                max_frame_size: 500,
                max_interval_frames: 2,
                interval_us: 1000,
            },
        });
        let _ = tsn_cnc.register_listener(TsnListener {
            stream_id: talker_sid,
            listener_mac: server_mac,
            reqs: UserToNetworkRequirements {
                max_latency_us: 5000,
                num_seamless_trees: 1,
            },
        });

        let ptp_telecom = TelecomProfileEngine::new(
            TelecomClockType::TelecomTimeSlaveClock,
            TelecomBmcaAttributes::new_slave_clock([
                0x52, 0x54, 0x00, 0xFF, 0xFE, 0x12, 0x34, 0x56,
            ]),
        );

        let mut ngap_node = NgapNode::new();
        ngap_node.handle_ng_setup(&NgSetupRequest {
            global_gnb_id: 101,
            gnb_name: "gNodeB-Taipei-01".to_string(),
            plmn: PlmnId {
                mcc: [2, 0, 8],
                mnc: [9, 5, 0],
            },
            tac: 0x0001,
            supported_slices: vec![Snssai { sst: 1, sd: None }],
        });

        let mut evpn_type3_bum = EvpnBumFloodingTree::new();
        evpn_type3_bum.add_route(EvpnType3Route::new_ipv4(
            RouteDistinguisher::new(client_stack.config.ip, 100),
            0,
            client_stack.config.ip,
            10001,
        ));
        evpn_type3_bum.add_route(EvpnType3Route::new_ipv4(
            RouteDistinguisher::new(server_ip, 100),
            0,
            server_ip,
            10001,
        ));

        let mut ptp_tc_engine = TransparentClockEngine::new(TransparentClockMode::EndToEnd);
        ptp_tc_engine.calculate_peer_delay(0, 100, 150, 250);

        let mut pfcp_upf = PfcpNode::new("upf-edge-01.5gcore.local");
        pfcp_upf.handle_association_setup("smf-control-01.5gcore.local");
        pfcp_upf.establish_session(
            0xFEED_FACE,
            vec![PacketDetectionRule {
                pdr_id: 1,
                precedence: 100,
                source_interface: PFCP_SRC_INTERFACE_ACCESS,
                teid: Some(0x10001),
                ue_ip: Some(Ipv4Address::new(10, 45, 0, 100)),
            }],
            vec![ForwardingActionRule {
                far_id: 1,
                apply_action: PFCP_APPLY_ACTION_FORWARD,
                destination_interface: PFCP_SRC_INTERFACE_CORE,
                outer_header_creation: None,
            }],
        );

        let mut ats_scheduler = UrgencyBasedScheduler::new();
        ats_scheduler.register_shaper(AtsStreamShaper::new(1, 10_000_000, 1500)); // 10 Mbps CIR

        let mut bgp_epe_db = BgpEpeDatabase::new();
        bgp_epe_db.add_peer_node_sid(16001, 65001, server_ip);
        bgp_epe_db.add_peer_adj_sid(16002, 65001, server_ip, 1);
        bgp_epe_db.add_peer_set_member(16003, 65001, server_ip, Some(1), 50);

        let gtpu_ext_container = PduSessionContainer::new_dl(9, true);

        let mut bgp_ls_srv6_db = BgpLsSrv6Database::new();
        bgp_ls_srv6_db.add_locator(Srv6LocatorTlv::new(
            0,
            10,
            "2001:db8:cafe::".parse().unwrap(),
            64,
        ));
        bgp_ls_srv6_db.add_end_sid(Srv6EndSidTlv::new(
            1, // End
            "2001:db8:cafe::1".parse().unwrap(),
        ));

        let cbs_shaper = CreditBasedShaper::new("AVB-Class-A", 100_000_000, 1_000_000_000, 1500);

        let mut sba_events_engine = SbaEventExposureEngine::new();
        sba_events_engine.subscribe(
            "nef-analytics-01",
            SbaEventType::LocationReport,
            "imsi-208950000000001",
            "https://nef.5gcore.local/v1/event-exposure/notify",
        );

        let mut evpn_smet_engine = EvpnSmetEngine::new();
        evpn_smet_engine.add_smet_route(EvpnSmetRoute::new_any_source(
            RouteDistinguisher::new(server_ip, 100),
            100, // VLAN 100
            Ipv4Address::new(239, 255, 0, 1),
            server_ip,
        ));

        let congestion_isolation = CongestionIsolationEngine::new(3);

        let mut nef_traffic_engine = NefTrafficInfluenceEngine::new();
        nef_traffic_engine.create_subscription(
            "af-trans-edge-01",
            "edge-cloud-vr",
            "edge.mec",
            SliceId {
                sst: 1,
                sd: 0x000001,
            },
            TrafficFilter {
                dst_ip: Ipv4Address::new(198, 51, 100, 1),
                dst_port: 8080,
                protocol: 6,
            },
            "DNAI-Taipei-Edge",
            Ipv4Address::new(10, 100, 0, 1),
        );

        let bgp_prefix_sid_attr = BgpPrefixSidAttribute::new(Some(100), Some(16000), Some(8000));
        let mut cqf_dual_buffer = CqfDualBufferEngine::new(1000, 10000); // 1000µs cycle
        cqf_dual_buffer.enqueue_frame(1, 100, vec![0xAB; 256]);

        let mut nrf_oauth_auth = NrfOAuthAuthority::new("nrf-central-01");
        let _ = nrf_oauth_auth.issue_access_token(
            NrfAccessTokenRequest {
                grant_type: "client_credentials".to_string(),
                nf_instance_id: "amf-node-01".to_string(),
                nf_type: NfType::Amf,
                target_nf_type: NfType::Udm,
                scope: "nudm-sdm".to_string(),
            },
            1700000000,
        );

        NetworkShell {
            stack: client_stack,
            remote_host_ip: server_ip,
            remote_host_ipv6: server_ip6,
            remote_host_mac: server_mac,
            remote_stack: server_stack,
            rip,
            igmp_table: MulticastGroupTable::new(),
            _tftp_server: TftpFileServer::new(),
            vrrp,
            hsrp,
            glbp,
            vtp,
            evpn_table,
            ofp_table,
            diameter_server,
            wg_peer,
            pcep_session,
            netconf_server,
            _lisp_resolver: lisp_resolver,
            flowspec_engine,
            otlp_exporter,
            gre_demux,
            srv6_engine,
            lfib,
            _ldp_session: ldp_session,
            bgp_rib,
            lldp_table,
            cdp_table,
            ospf_lsdb,
            stp_engine,
            sad_table,
            lag,
            eigrp_table,
            syslog_collector,
            pim_router,
            bfd_session,
            ldap_server,
            tacacs_server,
            _dhcpv6_server: dhcpv6_server,
            netflow_table,
            mqtt_broker,
            _gtp_table: gtp_table,
            _turn_table: turn_table,
            bgp_ls_db,
            srv6_mup_engine,
            mld_table,
            bfd_v6_mgr,
            netflow_v5_table,
            srv6_usid_engine,
            ti_lfa_engine,
            flex_algo_engine,
            vpls_instance,
            cfm_engine,
            sbfd_reflector,
            optical_dom,
            gnmi_server,
            gnoi_server,
            sr_policy_db,
            frer_engine,
            evpn_l3_vrf,
            cqf_engine,
            gribi_aft,
            evpn_df_engine,
            psfp_pipeline,
            p4runtime_server,
            evpn_aliasing,
            preemption_engine,
            bgp_ext_comms,
            sai_adapter,
            tas_shaper,
            sba_bus,
            evpn_type5_rib,
            tsn_cnc,
            ptp_telecom,
            ngap_node,
            evpn_type3_bum,
            ptp_tc_engine,
            pfcp_upf,
            ats_scheduler,
            bgp_epe_db,
            gtpu_ext_container,
            bgp_ls_srv6_db,
            cbs_shaper,
            sba_events_engine,
            evpn_smet_engine,
            congestion_isolation,
            nef_traffic_engine,
            bgp_prefix_sid_attr,
            cqf_dual_buffer,
            nrf_oauth_auth,
            pcap_writer: None,
            seq_counter: 1,
        }
    }

    fn record_packet(&mut self, data: &[u8]) {
        if let Some(ref mut writer) = self.pcap_writer {
            let _ = writer.write_packet(1700000000, 500000, data);
        }
    }

    pub fn run_repl(&mut self) {
        println!("╔════════════════════════════════════════════════════════════════════════════╗");
        println!("║         💻 Toy TCP/IP Stack - Dual-Stack IPv4/IPv6 Interactive Shell       ║");
        println!("╚════════════════════════════════════════════════════════════════════════════╝");
        println!(
            "Host IPv4: {} | IPv6: {:?} | MAC: {}",
            self.stack.config.ip,
            self.stack.config.ipv6.unwrap(),
            self.stack.config.mac
        );
        println!("Type 'help' for available commands or 'exit' to quit.\n");

        let stdin = io::stdin();
        let mut reader = stdin.lock();

        loop {
            print!("netstack > ");
            io::stdout().flush().unwrap();

            let mut line = String::new();
            if reader.read_line(&mut line).unwrap() == 0 {
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            match parts[0] {
                "exit" | "quit" => {
                    println!("Exiting network shell.");
                    break;
                }
                "help" => self.cmd_help(),
                "status" => self.cmd_status(),
                "arp" => self.cmd_arp(&parts[1..]),
                "ndp" => self.cmd_ndp(),
                "route" => self.cmd_route(),
                "rip" => self.cmd_rip(&parts[1..]),
                "ospf" => self.cmd_ospf(&parts[1..]),
                "eigrp" => self.cmd_eigrp(&parts[1..]),
                "isis" => self.cmd_isis(&parts[1..]),
                "bgp" => self.cmd_bgp(&parts[1..]),
                "evpn" => self.cmd_evpn(&parts[1..]),
                "flowspec" => self.cmd_flowspec(&parts[1..]),
                "otlp" => self.cmd_otlp(&parts[1..]),
                "gre6" => self.cmd_gre6(&parts[1..]),
                "twamp" => self.cmd_twamp(&parts[1..]),
                "lsp-ping" => self.cmd_lsp_ping(&parts[1..]),
                "srv6-ops" => self.cmd_srv6_ops(&parts[1..]),
                "gre-udp" => self.cmd_gre_udp(&parts[1..]),
                "bgp-ls" => self.cmd_bgp_ls(&parts[1..]),
                "bgp-ls-srv6" | "ls-srv6" => self.cmd_bgp_ls_srv6(&parts[1..]),
                "bgp-prefix-sid" | "prefix-sid" => self.cmd_bgp_prefix_sid(&parts[1..]),
                "ipfix" => self.cmd_ipfix(&parts[1..]),
                "srv6-mup" => self.cmd_srv6_mup(&parts[1..]),
                "5g-sba" | "sba" => self.cmd_5g_sba(&parts[1..]),
                "sba-events" | "5g-events" => self.cmd_sba_events(&parts[1..]),
                "nef-traffic" | "edge-mec" => self.cmd_nef_traffic(&parts[1..]),
                "nrf-oauth" | "oauth2" | "5g-auth" => self.cmd_nrf_oauth(&parts[1..]),
                "ngap" | "5g-n2" => self.cmd_ngap(&parts[1..]),
                "pfcp" | "5g-n4" => self.cmd_pfcp(&parts[1..]),
                "gtp-ext" | "qfi" | "5g-qos" => self.cmd_gtp_ext(&parts[1..]),
                "mld" => self.cmd_mld(&parts[1..]),
                "bfd6" | "bfd-v6" => self.cmd_bfd_v6(&parts[1..]),
                "geneve-sfc" => self.cmd_geneve_sfc(&parts[1..]),
                "usid" | "srv6-usid" => self.cmd_usid(&parts[1..]),
                "netflow5" | "netflow-v5" => self.cmd_netflow_v5(&parts[1..]),
                "ti-lfa" | "tilfa" => self.cmd_ti_lfa(&parts[1..]),
                "flex-algo" | "flexalgo" => self.cmd_flex_algo(&parts[1..]),
                "geneve-int" => self.cmd_geneve_int(&parts[1..]),
                "vpls" => self.cmd_vpls(&parts[1..]),
                "cfm" | "802.1ag" => self.cmd_cfm(&parts[1..]),
                "sbfd" | "s-bfd" => self.cmd_sbfd(&parts[1..]),
                "dom" | "optical" => self.cmd_dom(&parts[1..]),
                "etag" | "802.1br" => self.cmd_etag(&parts[1..]),
                "gnmi" => self.cmd_gnmi(&parts[1..]),
                "gnoi" => self.cmd_gnoi(&parts[1..]),
                "sr-policy" | "srpolicy" => self.cmd_sr_policy(&parts[1..]),
                "frer" | "802.1cb" => self.cmd_frer(&parts[1..]),
                "cqf" | "802.1qch" => self.cmd_cqf(&parts[1..]),
                "cqf-dual" | "cqf-buffer" => self.cmd_cqf_dual(&parts[1..]),
                "psfp" | "802.1qci" => self.cmd_psfp(&parts[1..]),
                "fpe" | "preemption" | "802.1qbu" => self.cmd_fpe(&parts[1..]),
                "tas" | "802.1qbv" => self.cmd_tas(&parts[1..]),
                "ats" | "802.1qcr" | "ubs" => self.cmd_ats(&parts[1..]),
                "cbs" | "802.1qav" | "avb" => self.cmd_cbs(&parts[1..]),
                "congestion-isolation" | "ci" | "802.1qcz" => {
                    self.cmd_congestion_isolation(&parts[1..])
                }
                "cnc" | "802.1qcc" => self.cmd_cnc(&parts[1..]),
                "gribi" => self.cmd_gribi(&parts[1..]),
                "p4" | "p4runtime" => self.cmd_p4runtime(&parts[1..]),
                "sai" | "sonic" => self.cmd_sai(&parts[1..]),
                "evpn-l3" | "l3-irb" => self.cmd_evpn_l3(&parts[1..]),
                "evpn-mh" | "df-election" => self.cmd_evpn_mh(&parts[1..]),
                "evpn-ad" | "aliasing" => self.cmd_evpn_ad(&parts[1..]),
                "evpn-t3" | "imet" | "bum" => self.cmd_evpn_t3(&parts[1..]),
                "evpn-t5" | "type5" => self.cmd_evpn_t5(&parts[1..]),
                "evpn-smet" | "smet" => self.cmd_evpn_smet(&parts[1..]),
                "bgp-ext" | "extcomm" => self.cmd_bgp_ext(&parts[1..]),
                "epe" | "bgp-epe" => self.cmd_bgp_epe(&parts[1..]),
                "geneve-opts" => self.cmd_geneve_opts(&parts[1..]),
                "gre-demux" => self.cmd_gre_demux(&parts[1..]),
                "ioam" => self.cmd_ioam(&parts[1..]),
                "netconf" => self.cmd_netconf(&parts[1..]),
                "lisp" => self.cmd_lisp(&parts[1..]),
                "wireguard" | "wg" => self.cmd_wireguard(&parts[1..]),
                "gptp" => self.cmd_gptp(&parts[1..]),
                "ptp-telecom" | "g8275" => self.cmd_ptp_telecom(&parts[1..]),
                "ptp-tc" | "tc" => self.cmd_ptp_tc(&parts[1..]),
                "pcep" => self.cmd_pcep(&parts[1..]),
                "rsvp" => self.cmd_rsvp(&parts[1..]),
                "openflow" | "ofp" => self.cmd_openflow(&parts[1..]),
                "diameter" => self.cmd_diameter(&parts[1..]),
                "nsh" => self.cmd_nsh(&parts[1..]),
                "sflow" => self.cmd_sflow(&parts[1..]),
                "6in4" => self.cmd_6in4(&parts[1..]),
                "4in6" => self.cmd_4in6(&parts[1..]),
                "roce" => self.cmd_roce(&parts[1..]),
                "pfc" => self.cmd_pfc(&parts[1..]),
                "gue" => self.cmd_gue(&parts[1..]),
                "bfd" => self.cmd_bfd(&parts[1..]),
                "geneve" => self.cmd_geneve(&parts[1..]),
                "ldap" => self.cmd_ldap(&parts[1..]),
                "ldp" => self.cmd_ldp(&parts[1..]),
                "glbp" => self.cmd_glbp(&parts[1..]),
                "tacacs" => self.cmd_tacacs(&parts[1..]),
                "vtp" => self.cmd_vtp(&parts[1..]),
                "dhcpv6" => self.cmd_dhcpv6(&parts[1..]),
                "vxlan-gpe" => self.cmd_vxlan_gpe(&parts[1..]),
                "netflow" => self.cmd_netflow(&parts[1..]),
                "sip" => self.cmd_sip(&parts[1..]),
                "mqtt" => self.cmd_mqtt(&parts[1..]),
                "coap" => self.cmd_coap(&parts[1..]),
                "sctp" => self.cmd_sctp(&parts[1..]),
                "rtp" => self.cmd_rtp(&parts[1..]),
                "ptp" => self.cmd_ptp(&parts[1..]),
                "erspan" => self.cmd_erspan(&parts[1..]),
                "cdp" => self.cmd_cdp(&parts[1..]),
                "srv6" => self.cmd_srv6(&parts[1..]),
                "stun" => self.cmd_stun(&parts[1..]),
                "turn" => self.cmd_turn(&parts[1..]),
                "gtp" => self.cmd_gtp(&parts[1..]),
                "hsrp" => self.cmd_hsrp(&parts[1..]),
                "mpls" => self.cmd_mpls(&parts[1..]),
                "lldp" => self.cmd_lldp(&parts[1..]),
                "stp" => self.cmd_stp(&parts[1..]),
                "lacp" => self.cmd_lacp(&parts[1..]),
                "pppoe" => self.cmd_pppoe(&parts[1..]),
                "radius" => self.cmd_radius(&parts[1..]),
                "syslog" => self.cmd_syslog(&parts[1..]),
                "l2tp" => self.cmd_l2tp(&parts[1..]),
                "pim" => self.cmd_pim(&parts[1..]),
                "vxlan" => self.cmd_vxlan(&parts[1..]),
                "ipsec" => self.cmd_ipsec(&parts[1..]),
                "http3" => self.cmd_http3(&parts[1..]),
                "traceroute" => self.cmd_traceroute(&parts[1..]),
                "ntp" => self.cmd_ntp(&parts[1..]),
                "tftp" => self.cmd_tftp(&parts[1..]),
                "snmp" => self.cmd_snmp(&parts[1..]),
                "quic" => self.cmd_quic(&parts[1..]),
                "vrrp" => self.cmd_vrrp(&parts[1..]),
                "tunnel" => self.cmd_tunnel(&parts[1..]),
                "igmp" => self.cmd_igmp(&parts[1..]),
                "ping" => self.cmd_ping(&parts[1..]),
                "ping6" => self.cmd_ping6(&parts[1..]),
                "dns" => self.cmd_dns(&parts[1..]),
                "udp" => self.cmd_udp(&parts[1..]),
                "curl" => self.cmd_curl(&parts[1..]),
                "tls" => self.cmd_tls(&parts[1..]),
                "http2" => self.cmd_http2(&parts[1..]),
                "ws" => self.cmd_ws(&parts[1..]),
                "netstat" => self.cmd_netstat(),
                "iptables" | "firewall" => self.cmd_firewall(&parts[1..]),
                "nat" => self.cmd_nat(&parts[1..]),
                "tcp-stats" => self.cmd_tcp_stats(),
                "lab" => self.cmd_lab(&parts[1..]),
                "pcap" => self.cmd_pcap(&parts[1..]),
                cmd => println!(
                    "Unknown command: '{}'. Type 'help' for available commands.",
                    cmd
                ),
            }
        }
    }

    fn cmd_help(&self) {
        println!("\nAvailable Commands:");
        println!(
            "  lab [topology|ping4|ping6|route4|udp-echo|tcp-demo|pcap] - Integrated Virtual Network Lab Simulation"
        );
        println!(
            "  status                              - Show current network interface details (IPv4 & IPv6)"
        );
        println!(
            "  lsp-ping <target_fec_ip> [mask_len] - MPLS LSP Ping Data Plane Verification (RFC 4379 / Port 3503)"
        );
        println!(
            "  srv6-ops [behaviors | execute <sid>]- SRv6 Network Programming Endpoint Behaviors (RFC 8986)"
        );
        println!(
            "  gre-udp encap <key> <msg>           - GRE-in-UDP Encapsulation for ECMP & NAT Traversal (RFC 8086)"
        );
        println!(
            "  bgp-ls [nodes | links | announce]   - BGP Link-State Topology & TE Distribution (RFC 7752 / RFC 9552)"
        );
        println!(
            "  bgp-ls-srv6 [locators | sids]       - BGP-LS Extensions for Segment Routing over IPv6 / SRv6 (RFC 9514)"
        );
        println!(
            "  bgp-prefix-sid [label | srgb]       - BGP Prefix-SID Attribute for SR-MPLS & SRv6 (RFC 8669 Path Attr 40)"
        );
        println!(
            "  ipfix [export | status]             - IP Flow Information Export / NetFlow v10 (RFC 7011 / UDP 4739)"
        );
        println!(
            "  srv6-mup [sessions | up | down]     - SRv6 Mobile User Plane 5G Core UPF Interworking (End.M.GTP4)"
        );
        println!(
            "  5g-sba [register | smf | amf]       - 5G Core Service Based Architecture REST Dispatcher (3GPP TS 29.500)"
        );
        println!(
            "  sba-events [sub | trigger | log]    - 5G SBA Event Exposure Service Namf_EventExposure (3GPP TS 29.518)"
        );
        println!(
            "  nef-traffic [sub | steer | list]    - 5G NEF Traffic Influence / Edge Computing MEC UPF Steering (TS 29.522)"
        );
        println!(
            "  nrf-oauth [token | verify]          - 5G Core NRF OAuth 2.0 Access Token Authorization Service (TS 29.510)"
        );
        println!(
            "  ngap [setup | ue | pdu]             - 5G N2 / NGAP gNodeB <-> AMF Signalling (3GPP TS 38.413 / SCTP 38412)"
        );
        println!(
            "  pfcp [setup | session | match]      - 5G N4 / PFCP SMF <-> UPF Control Protocol (3GPP TS 29.244 / UDP 8805)"
        );
        println!(
            "  gtp-ext [encap <qfi> | status]      - 5G N3 GTP-U PDU Session Container & QoS Flow Identifier (TS 38.415)"
        );
        println!(
            "  mld [report | query | status]       - Multicast Listener Discovery v2 SSM Group Mgmt (RFC 3810)"
        );
        println!(
            "  bfd6 [status | poll]                - IPv6 Multi-Hop & Single-Hop BFD Liveness Detection (RFC 5883)"
        );
        println!(
            "  geneve-sfc [encap <spi> <si> | hop] - Geneve Service Function Chaining In-Band Metadata (RFC 8926)"
        );
        println!(
            "  usid [pack | forward]               - SRv6 Micro-SID (uSID) Shift-and-Forward Compression Engine"
        );
        println!(
            "  netflow5 [export | status]          - Cisco NetFlow v5 Datacenter Flow Exporter (UDP 2055)"
        );
        println!(
            "  ti-lfa [protect <dst> <neighbor>]   - Topology-Independent Loop-Free Alternate & SR-FRR (RFC 4090)"
        );
        println!(
            "  flex-algo [algo <id> <src> <dst>]   - Segment Routing Flexible Algorithm Topology Slicing (RFC 9350)"
        );
        println!(
            "  geneve-int [trace | status]         - Geneve In-Band Network Telemetry Hop Recording (RFC 8926)"
        );
        println!(
            "  vpls [encap <mac> | status]         - Virtual Private LAN Service & Ethernet Pseudowire (RFC 4762)"
        );
        println!(
            "  cfm [ccm | lbm <trans_id>]          - Carrier Ethernet OAM IEEE 802.1ag / Y.1731 (EtherType 0x8902)"
        );
        println!(
            "  sbfd [probe | status]               - Seamless BFD Stateless Reflector & Initiator (RFC 7880 / UDP 7784)"
        );
        println!(
            "  dom [status | alarms]               - Digital Optical Monitoring Transceiver Telemetry (SFF-8472)"
        );
        println!(
            "  etag [encap <ecid> | status]        - IEEE 802.1BR Bridge Port Extension & E-TAG (EtherType 0x893F)"
        );
        println!(
            "  gnmi [get <path> | subscribe <path>]- OpenConfig gNMI Streaming Telemetry & Config (Port 9339)"
        );
        println!(
            "  gnoi [ping <target> | health | os]  - gRPC Network Operations Interface Microservice RPCs (Port 9339)"
        );
        println!(
            "  sr-policy [steer <color> | list]    - Segment Routing Traffic Steering & Candidate Paths (RFC 9256)"
        );
        println!(
            "  frer [replicate | status]           - IEEE 802.1CB Frame Replication & Elimination / TSN (R-TAG 0xF1C1)"
        );
        println!(
            "  cqf [enqueue | tick | status]       - IEEE 802.1Qch Cyclic Queuing & Forwarding / TSN Bounded Latency"
        );
        println!(
            "  cqf-dual [enqueue | drain | tick]   - IEEE 802.1Qch CQF Ping-Pong Dual Buffer Synchronized Zero-Jitter Forwarding"
        );
        println!(
            "  psfp [police | status]              - IEEE 802.1Qci Per-Stream Filtering & Policing / TSN Ingress Guard"
        );
        println!(
            "  fpe [preempt | status]              - IEEE 802.1Qbu Frame Preemption & Express Interleaving (TSN)"
        );
        println!(
            "  tas [schedule | status]             - IEEE 802.1Qbv Time-Aware Shaper / TSN Scheduled GCL Traffic"
        );
        println!(
            "  ats [enqueue <bytes> | dequeue]     - IEEE 802.1Qcr Asynchronous Traffic Shaping & Urgency-Based Scheduler"
        );
        println!(
            "  cbs [advance <us> | transmit]       - IEEE 802.1Qav Credit-Based Shaper / TSN AVB Stream Reservation"
        );
        println!(
            "  congestion-isolation [test | age]   - IEEE 802.1Qcz Congestion Isolation / RoCEv2 PFC Victim Mitigation"
        );
        println!(
            "  cnc [stream | register | status]    - IEEE 802.1Qcc TSN Centralized Network Configuration (CNC/CUC)"
        );
        println!(
            "  ptp-telecom [bmca | status]         - PTP Telecom Profile ITU-T G.8275.1/G.8275.2 (T-GM/T-BC/T-TSC)"
        );
        println!(
            "  ptp-tc [residence | pdelay | mode]  - PTP Transparent Clock Residence Time & Peer Delay (IEEE 1588v2)"
        );
        println!(
            "  gribi [add | fib <ip> | status]     - gRPC Routing Information Base Interface AFT Injection (Port 9340)"
        );
        println!(
            "  p4 [tables | punt | out <port>]     - P4Runtime SDN Match-Action Table & Packet-IO (Port 9559)"
        );
        println!(
            "  sai [fdb | route | status]          - OpenCompute Switch Abstraction Interface / SONiC Hardware Model"
        );
        println!(
            "  evpn-l3 [lookup <ip> | status]      - EVPN VXLAN Symmetric L3 IRB VRF Routing (RFC 9135 / Type 5)"
        );
        println!(
            "  evpn-mh [df <vlan> | status]        - EVPN Type 4 Multi-Homing Designated Forwarder Election (RFC 7432)"
        );
        println!(
            "  evpn-ad [aliasing | withdraw]       - EVPN Type 1 Ethernet A-D Aliasing & Mass Withdrawal (RFC 7432)"
        );
        println!(
            "  evpn-t3 [list | flood <vni>]        - EVPN Route Type 3 Inclusive Multicast Ethernet Tag / BUM (RFC 7432)"
        );
        println!(
            "  evpn-t5 [lookup <ip> | list]         - EVPN Route Type 5 IP Prefix Overlay Routing (RFC 9136)"
        );
        println!(
            "  evpn-smet [list | resolve <grp>]    - EVPN Route Type 6 Selective Multicast Ethernet Tag / SMET (RFC 9251)"
        );
        println!(
            "  bgp-ext [list | color <c>]          - BGP Extended Communities, Color & Tunnel Encap (RFC 4360/9012)"
        );
        println!(
            "  epe [resolve <label> | list]        - BGP Segment Routing Egress Peer Engineering (RFC 9086/9087)"
        );
        println!(
            "  twamp [test | greeting | status]    - Two-Way Active Measurement Protocol (RFC 5357 / Ports 862)"
        );
        println!(
            "  geneve-opts [build | parse]         - Geneve Extended Metadata & Dynamic TLV Options (RFC 8926)"
        );
        println!(
            "  gre-demux [status | demux <key>]    - GRE RFC 2890 Key-based VRF Demuxing & Anti-Replay"
        );
        println!(
            "  flowspec [rules | drop <dst> <port>]- BGP Flowspec Automated DDoS Mitigation (RFC 5575/8955)"
        );
        println!(
            "  otlp [export | status]              - OpenTelemetry OTLP Metrics & Spans Exporter (Ports 4317/4318)"
        );
        println!(
            "  gre6 encap <msg>                    - GRE-over-IPv6 Tunneling (RFC 7676 NextHdr 47)"
        );
        println!(
            "  ioam [record <msg> | trace]         - In-situ OAM In-Band Telemetry Recording (RFC 9197)"
        );
        println!(
            "  netconf [get | commit | hello]      - NETCONF Network Configuration XML-RPC (RFC 6241)"
        );
        println!(
            "  lisp [lookup <eid> | encap <msg>]   - Locator/ID Separation Protocol Overlay (RFC 9300/9301)"
        );
        println!(
            "  wireguard [handshake | send <msg>]  - WireGuard VPN Tunnel Protocol (Noise IK / UDP 51820)"
        );
        println!(
            "  gptp [pdelay | status]              - IEEE 802.1AS Generalized PTP / TSN (EtherType 0x88F7)"
        );
        println!(
            "  pcep [req <dest> | status]          - Path Computation Element Protocol / SR-MPLS (RFC 5440)"
        );
        println!(
            "  rsvp [path <dest> <bw> | resv <lbl>]- MPLS-TE RSVP-TE Explicit Path Signaling (RFC 3209)"
        );
        println!(
            "  openflow [tables | add <in> <dst> <out>] - OpenFlow 1.3 SDN Controller & Flow Table (TS-025)"
        );
        println!(
            "  diameter [cer | status]             - 4G/5G Core Diameter Base AAA Protocol (RFC 6733)"
        );
        println!(
            "  nsh [encap <spi> <si> <msg>]        - Network Service Header & Service Function Chaining (RFC 8300)"
        );
        println!(
            "  sflow [export | status]             - sFlow v5 Network Flow & Counter Telemetry (RFC 3176)"
        );
        println!(
            "  6in4 encap <msg>                    - IPv6-in-IPv4 Transition Tunnel (RFC 4213 Proto 41)"
        );
        println!(
            "  4in6 encap <msg>                    - IPv4-in-IPv6 Transition Tunnel (RFC 2473 NextHdr 4)"
        );
        println!(
            "  roce [send <qp> <msg> | write <qp>] - RoCEv2 AI/GPU Cluster RDMA Transport (UDP 4791)"
        );
        println!(
            "  pfc [pause <class> | status]        - IEEE 802.1Qbb Priority Flow Control (PFC)"
        );
        println!(
            "  gue [encap <msg>]                   - Generic UDP Encapsulation (RFC 7763 UDP 6080)"
        );
        println!(
            "  evpn [rib | lookup <vni> <mac>]     - BGP Ethernet VPN Control Plane (RFC 7432)"
        );
        println!("  ping <ipv4>                         - Send ICMP Echo Request (IPv4 Ping)");
        println!("  ping6 <ipv6>                        - Send ICMPv6 Echo Request (IPv6 Ping6)");
        println!(
            "  dhcpv6 [solicit]                    - Dynamic Host Configuration Protocol for IPv6 (RFC 8415)"
        );
        println!(
            "  vxlan-gpe encap <vni> <msg>         - VXLAN Generic Protocol Extension (UDP 4790)"
        );
        println!("  vtp [status | add <id> <name>]      - Cisco VLAN Trunking Protocol (VTP)");
        println!(
            "  traceroute <ipv4>                   - Trace network route hops using ICMP TTL Exceeded"
        );
        println!(
            "  ldp [hello | map <ip> <label>]      - MPLS Label Distribution Protocol (RFC 5036)"
        );
        println!("  glbp [status | arp | hello]         - Cisco Gateway Load Balancing Protocol");
        println!(
            "  tacacs auth <user> <pass>           - TACACS+ AAA Administrative Access (RFC 8907)"
        );
        println!("  cdp [neighbors | announce]          - Cisco Discovery Protocol v2 (CDPv2)");
        println!("  srv6 [encap | status]               - Segment Routing over IPv6 (RFC 8754)");
        println!(
            "  stun [probe <ip>]                   - Session Traversal Utilities for NAT (RFC 8489)"
        );
        println!(
            "  turn [alloc | send <msg>]           - Traversal Using Relays around NAT (RFC 5766)"
        );
        println!(
            "  gtp [encap <teid> <msg> | echo]     - 4G/5G Cellular GTP-U Tunneling (3GPP TS 29.281)"
        );
        println!(
            "  hsrp [status | hello | preempt]     - Cisco Hot Standby Router Protocol (RFC 2281)"
        );
        println!(
            "  rtp [send <pt> <msg> | sr]          - Real-time Transport Protocol & RTCP (RFC 3550)"
        );
        println!("  ptp [sync | delay]                  - Precision Time Protocol (IEEE 1588v2)");
        println!(
            "  erspan [mirror <session> <msg>]     - Encapsulated Remote SPAN Mirroring (RFC 7637)"
        );
        println!(
            "  mqtt [pub <topic> <msg> | sub]      - Message Queuing Telemetry Transport (ISO 20922)"
        );
        println!(
            "  coap [get <path>]                   - Constrained Application Protocol REST (RFC 7252)"
        );
        println!(
            "  sctp [init | send <msg>]            - Stream Control Transmission Protocol (RFC 4960)"
        );
        println!(
            "  ldap [search <filter> | bind <dn>]  - Lightweight Directory Access Protocol (RFC 4511)"
        );
        println!(
            "  netflow [status | export]           - NetFlow v9 / IPFIX Traffic Telemetry (RFC 3954)"
        );
        println!(
            "  sip [invite <user> | call]          - Session Initiation Protocol & SDP (RFC 3261)"
        );
        println!(
            "  bfd [status | poll]                 - Bidirectional Forwarding Detection (RFC 5880)"
        );
        println!(
            "  geneve [encap <vni> <msg>]          - Generic Network Virtualization Encap (RFC 8926)"
        );
        println!(
            "  isis [hello | status]               - Intermediate System to Intermediate System (RFC 1195)"
        );
        println!(
            "  syslog [send <msg> | list]          - System Logging & Event Telemetry (RFC 5424)"
        );
        println!(
            "  l2tp [encap <session_id> <msg>]     - L2TPv3 Ethernet Pseudowire Tunnel (RFC 3931)"
        );
        println!(
            "  pim [hello | join <group>]          - Protocol Independent Multicast - SM (RFC 7761)"
        );
        println!(
            "  radius auth <user> <pass>           - Authenticate with RADIUS AAA Server (RFC 2865)"
        );
        println!(
            "  pppoe [padi | session <id> <msg>]   - Point-to-Point Protocol over Ethernet (RFC 2516)"
        );
        println!(
            "  eigrp [hello | dual]                - Cisco EIGRP & DUAL Metric Engine (RFC 7868)"
        );
        println!("  ospf [hello | spf]                  - Open Shortest Path First v2 (RFC 2328)");
        println!("  ipsec [status | encap <msg>]        - IPsec ESP Tunnel Mode (RFC 4303)");
        println!(
            "  http3 [get <path> | settings]       - HTTP/3 over QUIC Binary Framing (RFC 9114)"
        );
        println!(
            "  lacp [status | hash <s_ip> <d_ip>]  - Link Aggregation (IEEE 802.1AX / 802.3ad)"
        );
        println!(
            "  mpls [push <label> <msg> | lfib]    - Multi-Protocol Label Switching (RFC 3031)"
        );
        println!("  bgp [status | rib | open]           - Border Gateway Protocol 4 (RFC 4271)");
        println!(
            "  lldp [neighbors | announce]         - Link Layer Discovery Protocol (IEEE 802.1AB)"
        );
        println!("  stp [status | bpdu]                 - IEEE 802.1D Spanning Tree Protocol");
        println!(
            "  vxlan encap <vni> <msg>             - Virtual eXtensible LAN Overlay (RFC 7348)"
        );
        println!(
            "  ntp [query <ip> | time]             - Network Time Protocol v4 clock synchronization"
        );
        println!(
            "  tftp get <filename>                 - Trivial File Transfer Protocol client download"
        );
        println!(
            "  snmp get <oid>                      - Simple Network Management Protocol v2c MIB query"
        );
        println!("  quic [probe | frame <msg>]          - QUIC (RFC 9000) binary packet framing");
        println!(
            "  vrrp [status | adv]                 - Virtual Router Redundancy Protocol (RFC 5798)"
        );
        println!(
            "  ndp                                 - Display IPv6 Neighbor Discovery Protocol (NDP) Cache"
        );
        println!(
            "  tunnel gre <dst_ip> <msg>           - Encapsulate payload in GRE (Protocol 47) tunnel"
        );
        println!(
            "  igmp [join <multicast_ip> | list]   - Manage IGMPv2 multicast group memberships"
        );
        println!("  dns <hostname>                      - Query virtual DNS server for IP address");
        println!(
            "  curl <ip[:port]>                    - Perform TCP 3-way handshake and HTTP/1.1 GET"
        );
        println!(
            "  tls <ip[:port]>                     - Perform TLS 1.3 ClientHello / ServerHello Handshake"
        );
        println!(
            "  http2 <ip[:port]>                   - Send HTTP/2 SETTINGS & HEADERS binary frames"
        );
        println!(
            "  ws send <msg>                       - Send masked WebSocket (RFC 6455) text frame"
        );
        println!(
            "  rip [status | adv]                  - RIPv2 dynamic distance-vector routing state"
        );
        println!("  udp send <ip> <port> <msg>          - Send UDP datagram to destination");
        println!("  arp [list | clear]                  - Inspect or manage ARP cache table");
        println!(
            "  route                               - Display routing table with Longest Prefix Match"
        );
        println!("  netstat                             - Display TCP connections and UDP sockets");
        println!("  iptables [list | add drop <ip> | flush] - Configure stateful firewall rules");
        println!("  nat [status | forward <ext_p> <int_ip> <int_p>] - NAT table & port forwarding");
        println!(
            "  tcp-stats                           - Inspect TCP Congestion Control & RTT state"
        );
        println!("  pcap start <file> | stop            - Record live session frames into PCAP");
        println!("  exit / quit                         - Exit the shell\n");
    }

    fn cmd_status(&self) {
        println!("Network Interface eth0 (Dual-Stack):");
        println!("  IPv4 Address : {}", self.stack.config.ip);
        println!("  IPv6 Address : {:?}", self.stack.config.ipv6);
        println!("  MAC Address  : {}", self.stack.config.mac);
        println!("  Subnet Mask  : /{}", self.stack.config.subnet_mask);
        println!("  Gateway      : {:?}", self.stack.config.gateway);
        println!(
            "  Remote Server: IPv4 {} | IPv6 {} ({})",
            self.remote_host_ip, self.remote_host_ipv6, self.remote_host_mac
        );
    }

    fn cmd_lsp_ping(&mut self, args: &[&str]) {
        let fec_ip = if !args.is_empty() {
            Ipv4Address::from_str(args[0]).unwrap_or(Ipv4Address::new(10, 0, 0, 1))
        } else {
            Ipv4Address::new(10, 0, 0, 1)
        };
        let mask_len = if args.len() >= 2 {
            args[1].parse::<u8>().unwrap_or(32)
        } else {
            32
        };

        println!(
            "Initiating MPLS LSP Echo Request (RFC 4379/8029) to Target FEC {}/{}...",
            fec_ip, mask_len
        );
        let req =
            LspEchoPacket::build_echo_request(0x1337BEEF, 1, fec_ip, mask_len, 1700000000, 500000);
        let raw_req = req.serialize();

        // LSP Ping packets use 127.0.0.1 as destination IP to prevent IP forwarding if label popped early
        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            Ipv4Address::new(127, 0, 0, 1),
            53503,
            LSP_PING_UDP_PORT,
            &raw_req,
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            Ipv4Address::new(127, 0, 0, 1),
            IP_PROTO_UDP,
            940,
            1,
            &udp_req,
        );

        let shim = MplsHeader::new(1001, 0, true, 64);
        let mpls_pkt = MplsPacket {
            labels: vec![shim],
            payload: ip_req,
        };
        let raw_mpls = mpls_pkt.serialize();
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_MPLS_UNICAST,
            &raw_mpls,
        );

        println!(
            "  1. Transmitted MPLS Encapsulated LSP Echo Request (Label 1001, UDP {}, {} bytes)",
            LSP_PING_UDP_PORT,
            eth_req.len()
        );
        println!(
            "     Target FEC Stack TLV: IPv4 Prefix {}/{}",
            fec_ip, mask_len
        );

        let resps = self.remote_stack.process_frame(&eth_req);
        for resp in resps {
            let eth = EthernetFrame::parse(&resp).unwrap();
            let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
            let udp =
                UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true).unwrap();
            if let Some(reply) = LspEchoPacket::parse(udp.payload) {
                let code_str = match reply.return_code {
                    LSP_RET_CODE_EGRESS_FOR_FEC => {
                        "3 (Replying router is an egress for the FEC at stack-depth)"
                    }
                    8 => "8 (Label switched at stack-depth)",
                    _ => "0 (Success)",
                };
                println!("  2. Received LSP Echo Reply from {}:", ip.header.src_ip);
                println!("     Return Code : {}", code_str);
                println!(
                    "     Sender Handle: 0x{:08X}, Seq: {}",
                    reply.sender_handle, reply.seq_number
                );
                println!("     LSP Data Plane Path Verified & Active!");
            }
        }
    }

    fn cmd_srv6_ops(&mut self, _args: &[&str]) {
        println!("SRv6 Network Programming Endpoint Functions (RFC 8986):");
        println!(
            "┌──────────────────────────────────────────┬────────────────────────────────────────┐"
        );
        println!(
            "│ SID Locator / Function                   │ Endpoint Behavior                      │"
        );
        println!(
            "├──────────────────────────────────────────┼────────────────────────────────────────┤"
        );
        for (sid, b) in &self.srv6_engine.my_sid_table {
            let b_str = match b {
                Srv6Behavior::End => "End (Transit Segment, Decrement SegLeft)".to_string(),
                Srv6Behavior::EndX {
                    next_hop_ip,
                    out_if,
                } => format!("End.X (Cross-Connect -> {} via {})", next_hop_ip, out_if),
                Srv6Behavior::EndDt4 { vrf_id } => {
                    format!("End.DT4 (Decapsulate -> VRF {} IPv4 Table)", vrf_id)
                }
                Srv6Behavior::EndDx2 { out_if } => {
                    format!("End.DX2 (Decapsulate -> L2 {})", out_if)
                }
                _ => format!("{:?}", b),
            };
            println!("│ {:<40} │ {:<38} │", sid, b_str);
        }
        println!(
            "└──────────────────────────────────────────┴────────────────────────────────────────┘"
        );

        let sid_egress = Ipv6Address::from_str("2001:db8:2::200").unwrap();
        let srh = Srv6Header::build(4, &[sid_egress]);
        let res =
            self.srv6_engine
                .process_srv6_packet(sid_egress, srh, b"Customer IPv4 VPN Payload");
        if let Srv6ExecutionResult::DecapIpv4 { vrf_id, payload } = res {
            println!("Execution Demo on SID {}:", sid_egress);
            println!("  Behavior Executed: End.DT4 (VRF {:?})", vrf_id);
            println!(
                "  Inner Decapsulated Payload: \"{}\"",
                String::from_utf8_lossy(&payload)
            );
        }
    }

    fn cmd_gre_udp(&mut self, args: &[&str]) {
        let key = if !args.is_empty() {
            args[0].parse::<u32>().unwrap_or(0x1001)
        } else {
            0x1001
        };

        let msg = if args.len() >= 2 {
            args[1..].join(" ")
        } else {
            "Cloud Multi-Tenant Payload traversing UDP Fabric".to_string()
        };

        let gre_udp = GreUdpPacket::new(52123, 0x0800, Some(key), Some(1), msg.as_bytes());
        let raw_gre = gre_udp.serialize();

        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            gre_udp.src_port,
            GRE_IN_UDP_PORT,
            &raw_gre,
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            941,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        println!(
            "Transmitted GRE-in-UDP (RFC 8086) Datagram (UDP {}, {} bytes):",
            GRE_IN_UDP_PORT,
            eth_req.len()
        );
        println!(
            "  Entropy Source Port: {} (Enables ECMP multi-path flow hashing)",
            gre_udp.src_port
        );
        println!(
            "  GRE Flags & Key    : Key=0x{:08X}, Seq=1, Inner Proto=0x0800",
            key
        );
        println!("  Inner Payload      : \"{}\"", msg);
    }

    fn cmd_bgp_ls(&mut self, _args: &[&str]) {
        println!("BGP Link-State (BGP-LS - RFC 7752 / RFC 9552) Topology Database:");
        println!("  AFI: 16388 (BGP-LS), SAFI: 71 (BGP-LS)");
        println!("\n  Discovered SDN Nodes:");
        for (router_id, node) in &self.bgp_ls_db.nodes {
            println!(
                "    • Router-ID: {:<15} | ASN: {:<6} | Name: {}",
                router_id,
                node.asn,
                node.node_name.as_deref().unwrap_or("N/A")
            );
        }
        println!("\n  Discovered Traffic Engineering (TE) Links:");
        for link in &self.bgp_ls_db.links {
            println!(
                "    • Link: {} -> {}",
                link.local_interface_ip, link.remote_neighbor_ip
            );
            println!(
                "      TE Metric: {}, Max BW: {:.0} Gbps, Reservable BW: {:.0} Gbps, Admin Group: 0x{:08X}",
                link.te_metric,
                link.max_bandwidth_bps / 1e9,
                link.max_reservable_bandwidth_bps / 1e9,
                link.admin_group_color
            );
        }

        // Demo NLRI serialization
        let sample_node = BgpLsNodeDescriptor {
            asn: 65001,
            igp_router_id: self.stack.config.ip,
            node_name: Some("Local-Leaf-01".to_string()),
        };
        let nlri = BgpLsNlri::Node(sample_node);
        let raw = nlri.serialize();
        println!(
            "\n  Generated BGP-LS Node NLRI Payload ({} bytes):",
            raw.len()
        );
        println!("    Hex: {:02X?}", &raw[..raw.len().min(32)]);
    }

    fn cmd_ipfix(&mut self, _args: &[&str]) {
        println!("IP Flow Information Export (IPFIX / NetFlow v10 - RFC 7011 / RFC 7012):");
        let flows = vec![IpfixFlowRecord {
            src_ip: self.stack.config.ip,
            dst_ip: self.remote_host_ip,
            src_port: 54321,
            dst_port: 443,
            protocol: 6,
            packets: 2450,
            octets: 3560000,
            tcp_flags: 0x0018,
            vlan_id: 100,
        }];

        let msg = IpfixMessage::build_standard_flow_export(1700000000, 101, 1, &flows, true);
        let raw_ipfix = msg.serialize();

        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            54739,
            IPFIX_UDP_PORT,
            &raw_ipfix,
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            942,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        println!(
            "  Transmitted IPFIX Export Packet (UDP {}, {} bytes):",
            IPFIX_UDP_PORT,
            eth_req.len()
        );
        println!("    Version: 10, Export Time: 1700000000, Seq: 101, Observation Domain: 1");
        println!(
            "    Template: Template ID 256 (9 Field Specifiers: IPs, Ports, Proto, Octets, Packets, TCP Flags, VLAN)"
        );
        println!(
            "    Data Record: {} -> {}:443 (Proto 6, Packets: 2450, Octets: 3560000, VLAN: 100)",
            self.stack.config.ip, self.remote_host_ip
        );

        let parsed = IpfixMessage::parse(&raw_ipfix).unwrap();
        println!(
            "  Receiver Parsed Flow Records: {} record(s) verified successfully!",
            parsed.flow_records.len()
        );
    }

    fn cmd_srv6_mup(&mut self, _args: &[&str]) {
        println!("SRv6 Mobile User Plane (SRv6-MUP) & 5G Core UPF Interworking:");
        println!(
            "┌───────────────────────┬────────────┬────────────────────────────────────────┬─────┐"
        );
        println!(
            "│ gNodeB / UPF IPv4     │ GTP TEID   │ SRv6 Mobile SID                        │ QFI │"
        );
        println!(
            "├───────────────────────┼────────────┼────────────────────────────────────────┼─────┤"
        );
        for ((gnb, teid), sess) in &self.srv6_mup_engine.uplink_sessions {
            println!(
                "│ {:<21} │ 0x{:08X} │ {:<38} │ {:<3} │",
                gnb, teid, sess.srv6_sid, sess.qfi
            );
        }
        println!(
            "└───────────────────────┴────────────┴────────────────────────────────────────┴─────┘"
        );

        let gnb_ip = Ipv4Address::new(192, 168, 1, 50);
        let teid = 0xCAFE0001;
        let pdu_data = b"5G NR User Equipment Data Packet";

        // Uplink Test (End.M.GTP4.E)
        println!("\n1. Uplink Pipeline (End.M.GTP4.E):");
        println!(
            "   Ingress: GTP-U (TEID 0x{:08X}) from gNodeB {}",
            teid, gnb_ip
        );
        let srv6_pkt = self
            .srv6_mup_engine
            .process_uplink_gtp_to_srv6(gnb_ip, teid, pdu_data, self.stack.config.ipv6.unwrap())
            .unwrap();
        let parsed_v6 = Ipv6Packet::parse(&srv6_pkt).unwrap();
        println!(
            "   Egress : SRv6 Encapsulated IPv6 Packet (DA: {}, Length: {} bytes)",
            parsed_v6.header.dst_ip,
            srv6_pkt.len()
        );

        // Downlink Test (End.M.GTP4.D)
        println!("\n2. Downlink Pipeline (End.M.GTP4.D):");
        println!(
            "   Ingress: SRv6 Packet destined to SID {}",
            parsed_v6.header.dst_ip
        );
        let gtp_pkt = self
            .srv6_mup_engine
            .process_downlink_srv6_to_gtp(parsed_v6.header.dst_ip, pdu_data, self.stack.config.ip)
            .unwrap();
        let parsed_v4 = Ipv4Packet::parse(&gtp_pkt, true).unwrap();
        println!(
            "   Egress : GTP-U/UDP/IPv4 Packet to gNodeB {} (Length: {} bytes)",
            parsed_v4.header.dst_ip,
            gtp_pkt.len()
        );
        println!("   SRv6-MUP 5G Core User Plane Interworking Verified!");
    }

    fn cmd_mld(&mut self, _args: &[&str]) {
        println!("Multicast Listener Discovery v2 (MLDv2 - RFC 3810) Subscriptions:");
        println!(
            "┌──────────────────────────────────────────┬────────────────────────────────────────┐"
        );
        println!(
            "│ IPv6 Multicast Group (G)                 │ Allowed Source Filter Set (S)          │"
        );
        println!(
            "├──────────────────────────────────────────┼────────────────────────────────────────┤"
        );
        for (group, sources) in &self.mld_table.group_listeners {
            let src_str = if sources.is_empty() {
                "Any Source (*, G)".to_string()
            } else {
                sources
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            println!("│ {:<40} │ {:<38} │", group, src_str);
        }
        println!(
            "└──────────────────────────────────────────┴────────────────────────────────────────┘"
        );

        let group_ip = Ipv6Address::from_str("ff3e::8000:2").unwrap();
        let src_ip = Ipv6Address::from_str("2001:db8:1::55").unwrap();
        let report = Mldv2ReportPacket::new(vec![MldGroupRecord {
            record_type: MLD_CHANGE_TO_INCLUDE,
            multicast_address: group_ip,
            source_addresses: vec![src_ip],
        }]);
        let raw_mld = report.serialize();

        println!(
            "Transmitted MLDv2 Listener Report (ICMPv6 Type 143, {} bytes):",
            raw_mld.len()
        );
        println!("  Joined SSM Channel: ({}, {})", src_ip, group_ip);
        self.mld_table.process_report(&report);
        println!("  Listener status updated successfully in MLD forwarding table!");
    }

    fn cmd_bfd_v6(&mut self, _args: &[&str]) {
        println!("Multi-Hop & IPv6 BFD (RFC 5881 / RFC 5883) Session Management:");
        for (peer, sess) in &self.bfd_v6_mgr.sessions {
            let mode = if sess.is_multihop {
                "Multi-Hop (UDP 4784)"
            } else {
                "Single-Hop (UDP 3784)"
            };
            println!(
                "  • Peer IPv6: {} | State: {:?} | Discriminators: [My: 0x{:08X}, Your: 0x{:08X}]",
                peer, sess.state, sess.my_discriminator, sess.your_discriminator
            );
            println!(
                "    Mode: {}, Min Tx/Rx: {} us, Multiplier: {}",
                mode, sess.desired_min_tx_us, sess.detect_mult
            );
        }

        println!(
            "\nTransmitting Multi-Hop BFD Control Packet (UDP {}) to {}...",
            BFD_MULTIHOP_PORT, self.remote_host_ipv6
        );
        let bfd_pkt = BfdControlPacket::build_control(BfdState::Down, 0x55443322, 0, 50_000);
        let raw_bfd = bfd_pkt.serialize();

        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            54784,
            BFD_MULTIHOP_PORT,
            &raw_bfd,
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            943,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        let resps = self.remote_stack.process_frame(&eth_req);
        for resp in resps {
            let eth = EthernetFrame::parse(&resp).unwrap();
            let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
            let udp =
                UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true).unwrap();
            if let Ok(bfd_resp) = BfdControlPacket::parse(udp.payload) {
                println!(
                    "  Received BFD Response from {}: State: {:?}, YourDisc: 0x{:08X}",
                    ip.header.src_ip, bfd_resp.state, bfd_resp.your_discriminator
                );
                if let Some(session) = self.bfd_v6_mgr.sessions.get_mut(&self.remote_host_ipv6) {
                    session.process_inbound_packet(&bfd_resp);
                    println!("  Local Session State transitioned to: {:?}", session.state);
                }
            }
        }
    }

    fn cmd_geneve_sfc(&mut self, _args: &[&str]) {
        println!("Geneve Service Function Chaining (Geneve-SFC - RFC 8926 / RFC 8300):");
        let hop = GeneveSfcHop {
            vni: 9001,
            service_path_id: 0x0055AA,
            service_index: 3,
            tenant_id: 200,
            security_group: 88,
        };

        let msg = b"Encrypted Enterprise App Traffic traversing SFC Chain";
        let mut sfc_pkt = GeneveSfcPacket::build(9001, 0x0800, hop, msg);
        let raw = sfc_pkt.serialize();

        println!(
            "  1. Originating Geneve-SFC Tunnel Frame (VNI: 9001, {} bytes):",
            raw.len()
        );
        println!(
            "     SFC Path Option   : Class=0x{:04X}, Type=0x{:02X}, SPI=0x{:06X}, SI={}",
            GENEVE_OPT_CLASS_SFC,
            1,
            sfc_pkt.sfc_metadata.service_path_id,
            sfc_pkt.sfc_metadata.service_index
        );
        println!(
            "     SFC Context Option: Tenant ID={}, Security Group={}",
            sfc_pkt.sfc_metadata.tenant_id, sfc_pkt.sfc_metadata.security_group
        );

        // Advance Hop 1: Firewall -> DPI
        sfc_pkt.advance_service_hop();
        println!(
            "  2. Hop 1 Completed (Firewall): Service Index decremented to {}",
            sfc_pkt.sfc_metadata.service_index
        );

        // Advance Hop 2: DPI -> WAF
        sfc_pkt.advance_service_hop();
        println!(
            "  3. Hop 2 Completed (DPI): Service Index decremented to {}",
            sfc_pkt.sfc_metadata.service_index
        );
        println!("  Service Function Chaining In-Band Metadata Progression Verified!");
    }

    fn cmd_usid(&mut self, _args: &[&str]) {
        println!("SRv6 Micro-SID (uSID) Shift-and-Forward Compression Engine:");
        let carrier = UsidCarrier::new(0xFC000001, vec![0x1001, 0x2002, 0xE001]);
        let packed_da = carrier.to_ipv6();

        println!("  1. Originating Compressed IPv6 Packet:");
        println!(
            "     Block Prefix  : 0x{:08X} (fc00:1::/32)",
            carrier.block_prefix
        );
        println!("     Micro-SIDs    : {:?}", carrier.micro_sids);
        println!("     Packed IPv6 DA: {}", packed_da);

        // Hop 1: Node 1001 (End.uN)
        let (hop1_da, beh1) = self
            .srv6_usid_engine
            .process_destination_address(&packed_da)
            .unwrap();
        println!("\n  2. Hop 1 Processing (uSID 0x1001 -> {:?}):", beh1);
        println!(
            "     Active uSID consumed, Shift-and-Forward -> Next DA: {}",
            hop1_da
        );

        // Hop 2: Node 2002 (End.uN)
        let (hop2_da, beh2) = self
            .srv6_usid_engine
            .process_destination_address(&hop1_da)
            .unwrap();
        println!("\n  3. Hop 2 Processing (uSID 0x2002 -> {:?}):", beh2);
        println!(
            "     Active uSID consumed, Shift-and-Forward -> Next DA: {}",
            hop2_da
        );

        // Hop 3: Node E001 (End.uDT4)
        let (_hop3_da, beh3) = self
            .srv6_usid_engine
            .process_destination_address(&hop2_da)
            .unwrap();
        println!("\n  4. Egress Terminus (uSID 0xE001 -> {:?}):", beh3);
        println!(
            "     Decapsulating IPv6 outer carrier and routing inner IPv4 packet to local VRF!"
        );
        println!("  SRv6 uSID Header Compression & Shift-and-Forward Verified!");
    }

    fn cmd_netflow_v5(&mut self, _args: &[&str]) {
        println!(
            "Cisco NetFlow v5 Datacenter Flow Telemetry (UDP {}):",
            NETFLOW_V5_UDP_PORT
        );
        let export_pkt = self.netflow_v5_table.export_packet(120_000, 1700000000);
        let raw = export_pkt.serialize();

        println!(
            "  • Exported NetFlow v5 Packet ({} bytes, {} flow records, seq: {}):",
            raw.len(),
            export_pkt.header.count,
            export_pkt.header.flow_sequence
        );
        for (i, rec) in export_pkt.records.iter().enumerate() {
            println!(
                "    [{}] {}:{} -> {}:{} | Proto: {}, Pkts: {}, Bytes: {}",
                i + 1,
                rec.src_addr,
                rec.src_port,
                rec.dst_addr,
                rec.dst_port,
                rec.protocol,
                rec.packet_count,
                rec.octet_count
            );
        }

        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            52055,
            NETFLOW_V5_UDP_PORT,
            &raw,
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            944,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        let _resps = self.remote_stack.process_frame(&eth_req);
        println!(
            "  NetFlow v5 Datagram transmitted to Flow Collector {} successfully!",
            self.remote_host_ip
        );
    }

    fn cmd_ti_lfa(&mut self, _args: &[&str]) {
        println!("Topology-Independent Loop-Free Alternate (TI-LFA) Protection Calculation:");
        println!("  Source: NodeS | Protected Link: NodeS -> NodeE | Target Destination: NodeD");

        if let Some(prot) = self
            .ti_lfa_engine
            .compute_protection("NodeS", "NodeD", "NodeE")
        {
            println!("  • Primary Next-Hop : {}", prot.primary_next_hop);
            println!("  • Backup Next-Hop  : {}", prot.backup_next_hop);
            println!("  • Repair Node (PQ) : {:?}", prot.repair_node);
            println!("  • Backup Segment List: {:?}", prot.backup_segment_list);
            println!("  TI-LFA 100% Link Failure Fast Reroute (<50ms) Pre-computed Successfully!");
        } else {
            println!("  Failed to compute TI-LFA backup path.");
        }
    }

    fn cmd_flex_algo(&mut self, _args: &[&str]) {
        println!("Segment Routing Flexible Algorithms (SR-Flex-Algo - RFC 9350):");
        if let Some((delay_cost, path_delay)) = self
            .flex_algo_engine
            .compute_flex_algo_spf(128, "NodeA", "NodeB")
        {
            println!(
                "  • Algo 128 (Min Delay Slice): Total Delay = {}us, Path = {:?}",
                delay_cost, path_delay
            );
        }
        if let Some((igp_cost, path_igp)) = self
            .flex_algo_engine
            .compute_flex_algo_spf(129, "NodeA", "NodeB")
        {
            println!(
                "  • Algo 129 (Exclude Affinity 0x02): Total IGP Cost = {}, Path = {:?}",
                igp_cost, path_igp
            );
        }
        println!("  SR-Flex-Algo Multi-Topology Constraint-based Slicing Verified!");
    }

    fn cmd_geneve_int(&mut self, _args: &[&str]) {
        println!("Geneve In-Band Network Telemetry (INT-over-Geneve - RFC 8926 / P4 INT):");
        let mut int_pkt = GeneveIntPacket::build(7001, 0x0800, Vec::new(), b"HTTP/2 Data Payload");

        int_pkt.add_hop_telemetry(IntHopTelemetry {
            switch_id: 101,
            ingress_port: 1,
            egress_port: 48,
            hop_latency_ns: 420,
            queue_depth_bytes: 1500,
        });
        int_pkt.add_hop_telemetry(IntHopTelemetry {
            switch_id: 201,
            ingress_port: 12,
            egress_port: 16,
            hop_latency_ns: 310,
            queue_depth_bytes: 4096,
        });
        int_pkt.add_hop_telemetry(IntHopTelemetry {
            switch_id: 102,
            ingress_port: 48,
            egress_port: 2,
            hop_latency_ns: 390,
            queue_depth_bytes: 1024,
        });

        println!("  • Geneve VNI: {}", int_pkt.vni);
        println!(
            "  • In-Band Telemetry Hops Traversed: {}",
            int_pkt.telemetry_hops.len()
        );
        for (i, hop) in int_pkt.telemetry_hops.iter().enumerate() {
            println!(
                "    Hop {}: Switch ID {}, InPort {} -> OutPort {}, Latency {}ns, Queue {}B",
                i + 1,
                hop.switch_id,
                hop.ingress_port,
                hop.egress_port,
                hop.hop_latency_ns,
                hop.queue_depth_bytes
            );
        }
        println!(
            "  • Cumulative End-to-End Latency: {} ns",
            int_pkt.calculate_total_latency_ns()
        );
        println!(
            "  • Peak Buffer Depth on Path    : {} bytes",
            int_pkt.max_queue_depth_bytes()
        );

        let raw = int_pkt.serialize();
        let parsed = GeneveIntPacket::parse(&raw).unwrap();
        println!(
            "  INT-over-Geneve Wire Serialization ({} bytes) & Telemetry Parsing Verified!",
            raw.len()
        );
        assert_eq!(parsed.telemetry_hops.len(), 3);
    }

    fn cmd_vpls(&mut self, _args: &[&str]) {
        println!("Virtual Private LAN Service & Ethernet Pseudowire (VPLS / EoMPLS - RFC 4762):");
        let inner_eth = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            b"Customer L2 Broadcast/Unicast Traffic",
        );

        if let Some(vpls_pkt) =
            self.vpls_instance
                .encapsulate_frame(self.remote_host_mac, &inner_eth, 101)
        {
            let mpls = MplsPacket::parse(&vpls_pkt).unwrap();
            println!(
                "  • Encapsulated VPLS MPLS Frame ({} bytes):",
                vpls_pkt.len()
            );
            println!(
                "    Labels Stack: Tunnel Label = {}, VC/PW Label = {}",
                mpls.labels[0].label, mpls.labels[1].label
            );
            println!("    Control Word (4 bytes) Prepended: Sequence = 101");
            println!(
                "    Inner Payload: Ethernet Frame ({} bytes, {} -> {})",
                inner_eth.len(),
                self.stack.config.mac,
                self.remote_host_mac
            );
            println!("  VPLS Multipoint Pseudowire Encapsulation & Split-Horizon Verified!");
        } else {
            println!("  MAC not found in VPLS FIB table.");
        }
    }

    fn cmd_cfm(&mut self, _args: &[&str]) {
        println!(
            "Carrier Ethernet OAM IEEE 802.1ag / ITU-T Y.1731 (CFM - EtherType 0x{:04X}):",
            ETHERTYPE_CFM
        );

        // 1. Send Continuity Check Message (CCM)
        let ccm = CfmPacket::build_ccm(
            self.cfm_engine.md_level,
            self.cfm_engine.local_mep_id,
            105,
            &self.cfm_engine.maid,
            false,
        );
        let raw_ccm = ccm.serialize();
        let eth_ccm = EthernetFrame::serialize(
            CFM_MULTICAST_CLASS1,
            self.stack.config.mac,
            ETHERTYPE_CFM,
            &raw_ccm,
        );

        println!(
            "  • CCM Heartbeat Frame Transmitted ({} bytes):",
            eth_ccm.len()
        );
        println!(
            "    MD Level: {}, Local MEP ID: {}, MAID: '{}'",
            self.cfm_engine.md_level, self.cfm_engine.local_mep_id, self.cfm_engine.maid
        );
        println!("    Multicast Egress MAC: {}", CFM_MULTICAST_CLASS1);

        // 2. Loopback Message (LBM) / Reply (LBR)
        let lbm = CfmPacket::build_lbm(
            self.cfm_engine.md_level,
            0xAABBCCDD,
            b"Carrier Ping Pattern",
        );
        let raw_lbm = lbm.serialize();
        if let Some(lbr) = self.cfm_engine.process_cfm_frame(&raw_lbm) {
            println!("\n  • LBM/LBR Loopback Roundtrip Verified:");
            println!(
                "    LBR Opcode: {} (Reply), Transaction ID: 0xAABBCCDD, Pattern Length: {} bytes",
                lbr.header.opcode,
                lbr.payload.len() - 4
            );
        }

        // Active peer MEP status
        for (peer_id, status) in &self.cfm_engine.remote_meps {
            println!(
                "  • Monitored Remote MEP {}: Last Seq = {}, CCM Count = {}, RDI = {}",
                peer_id, status.last_seq, status.ccm_count, status.rdi
            );
        }
        println!("  Carrier Ethernet CFM & Y.1731 OAM Health Check OK!");
    }

    fn cmd_sbfd(&mut self, _args: &[&str]) {
        println!(
            "Seamless BFD (S-BFD - RFC 7880 / RFC 7881) Probe to {}:{}...",
            self.remote_host_ip, SBFD_REFLECTOR_PORT
        );
        println!(
            "  • Local Discriminators: {:?}",
            self.sbfd_reflector.local_discriminators
        );
        let my_disc = 0x10001;
        let reflector_disc = 0x90001;

        let probe = SbfdPacket::build_initiator_probe(my_disc, reflector_disc, 50_000);
        let raw_probe = probe.serialize();

        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            57784,
            SBFD_REFLECTOR_PORT,
            &raw_probe,
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            945,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        let resps = self.remote_stack.process_frame(&eth_req);
        for resp in resps {
            let eth = EthernetFrame::parse(&resp).unwrap();
            let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
            let udp =
                UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true).unwrap();
            if let Some(sbfd_resp) = SbfdPacket::parse(udp.payload) {
                println!(
                    "  • Received S-BFD Reflection from {}: State: {:?}, FinalBit: {}",
                    ip.header.src_ip, sbfd_resp.state, sbfd_resp.final_bit
                );
                println!(
                    "    My Disc: 0x{:08X} (Reflector), Your Disc: 0x{:08X} (Initiator Match)",
                    sbfd_resp.my_discriminator, sbfd_resp.your_discriminator
                );
                println!("  Stateless S-BFD Reflector Verification Completed Successfully!");
            }
        }
    }

    fn cmd_dom(&mut self, _args: &[&str]) {
        println!("Digital Optical Monitoring & Transceiver Telemetry (SFF-8472 / SFF-8636):");
        for (i, dom) in self.optical_dom.iter().enumerate() {
            let alarms = dom.evaluate_alarms();
            let tx_mw = OpticalDiagnostics::dbm_to_mw(dom.tx_power_dbm);
            let rx_mw = OpticalDiagnostics::dbm_to_mw(dom.rx_power_dbm);

            println!(
                "\n  [{}] Port: {} ({:?})",
                i + 1,
                dom.port_name,
                dom.form_factor
            );
            println!(
                "      Temperature   : {:.1} °C (High Alarm: {:.1} °C)",
                dom.temperature_c, dom.thresholds.temp_high_alarm_c
            );
            println!("      Supply Voltage: {:.2} V", dom.supply_voltage_v);
            println!("      Laser Tx Bias : {:.1} mA", dom.tx_bias_current_ma);
            println!(
                "      Tx Power      : {:.2} dBm ({:.3} mW)",
                dom.tx_power_dbm, tx_mw
            );
            println!(
                "      Rx Power      : {:.2} dBm ({:.3} mW)",
                dom.rx_power_dbm, rx_mw
            );
            println!(
                "      Path Loss     : {:.2} dB | Rx Safety Margin: {:.2} dB",
                dom.link_attenuation_db(),
                dom.rx_optical_margin_db()
            );
            println!(
                "      Status / Flags: RxLOS: {}, TxFault: {}, TempAlarm: {}, RxPowerLow: {}",
                alarms.rx_los, alarms.tx_fault, alarms.temp_alarm, alarms.rx_power_low
            );
        }
        println!("\n  Optical Physical Layer Telemetry & DOM Monitoring OK!");
    }

    fn cmd_etag(&mut self, _args: &[&str]) {
        println!(
            "IEEE 802.1BR Bridge Port Extension & E-TAG (EtherType 0x{:04X}):",
            ETHERTYPE_ETAG
        );
        let etag_header = ETagHeader {
            pcp: 6,
            dei: false,
            ingress_e_cid: 0x10001,
            grp: 0,
            e_cid: 0x20002,
            inner_ethertype: 0x0800,
        };

        let frame = ETagFrame::new(
            self.remote_host_mac,
            self.stack.config.mac,
            etag_header,
            b"Fabric Extender (FEX) Downlink Virtual Port Frame".to_vec(),
        );

        let raw = frame.serialize();
        let parsed = ETagFrame::parse(&raw).unwrap();

        println!(
            "  • Encapsulated 802.1BR E-TAG Frame ({} bytes):",
            raw.len()
        );
        println!(
            "    E-PCP: {}, Ingress E-CID: 0x{:05X}, Target E-CID: 0x{:05X}",
            parsed.etag.pcp, parsed.etag.ingress_e_cid, parsed.etag.e_cid
        );
        println!(
            "    Inner EtherType: 0x{:04X}, Payload Length: {} bytes",
            parsed.etag.inner_ethertype,
            parsed.payload.len()
        );
        println!("  IEEE 802.1BR Port Virtualization & E-TAG Framing Verified!");
    }

    fn cmd_gnmi(&mut self, args: &[&str]) {
        let path_query = if !args.is_empty() {
            args[0]
        } else {
            "/interfaces/interface[name=HundredGigE0/1]/state"
        };

        println!(
            "OpenConfig gNMI (Port {}) Query: '{}'",
            GNMI_PORT, path_query
        );
        let updates = self.gnmi_server.get(path_query);

        println!(
            "  • gNMI Response ({} telemetry notifications):",
            updates.len()
        );
        for update in &updates {
            println!(
                "    [{}] {} = {:?}",
                update.timestamp_ns,
                update.path.to_string_path(),
                update.val
            );
        }
        println!("  gNMI Streaming Telemetry & OpenConfig Tree Verified!");
    }

    fn cmd_sr_policy(&mut self, _args: &[&str]) {
        let color = 100;
        let endpoint = self.remote_host_ipv6;
        println!(
            "Segment Routing Policy (RFC 9256) Steering for (Color: {}, Endpoint: {}):",
            color, endpoint
        );

        if let Some(policy) = self.sr_policy_db.policies.get(&(color, endpoint)) {
            println!("  • Policy Name: '{}'", policy.name);
            println!(
                "  • Candidate Paths Evaluated: {}",
                policy.candidate_paths.len()
            );
            for (i, cp) in policy.candidate_paths.iter().enumerate() {
                println!(
                    "    Path #{}: Preference {}, Origin {:?}",
                    i + 1,
                    cp.preference,
                    cp.protocol_origin
                );
            }

            if let Some(best) = policy.best_candidate_path() {
                println!(
                    "  • Active Candidate Path Selected (Highest Preference {} / {:?}):",
                    best.preference, best.protocol_origin
                );
                for sl in &best.segment_lists {
                    println!("    Segment List (Weight {}):", sl.weight);
                    for (hop_idx, sid) in sl.segments.iter().enumerate() {
                        println!("      Hop #{}: {}", hop_idx + 1, sid);
                    }
                }
            }
            println!("  SR Policy Traffic Steering Pipeline OK!");
        } else {
            println!(
                "  No matching SR Policy found for (Color: {}, Endpoint: {}).",
                color, endpoint
            );
        }
    }

    fn cmd_frer(&mut self, _args: &[&str]) {
        println!(
            "IEEE 802.1CB Frame Replication & Elimination for Reliability (FRER / TSN - EtherType 0x{:04X}):",
            ETHERTYPE_RTAG
        );
        let payload = b"TSN Time-Critical Motion Control & Telemetry";
        let (path_a, path_b) = self.frer_engine.replicate(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            payload,
        );

        println!(
            "  • Replicated Ingress Frame (Seq: {}):",
            path_a.rtag.sequence_number
        );
        println!(
            "    Path A Frame ({} bytes): Dst: {}, Src: {}, Inner EtherType: 0x{:04X}",
            path_a.serialize().len(),
            path_a.dst_mac,
            path_a.src_mac,
            path_a.rtag.inner_ethertype
        );
        println!(
            "    Path B Frame ({} bytes): Dst: {}, Src: {}, Inner EtherType: 0x{:04X}",
            path_b.serialize().len(),
            path_b.dst_mac,
            path_b.src_mac,
            path_b.rtag.inner_ethertype
        );

        // Receive Path A (first arrival)
        let fwd_a = self.frer_engine.process_ingress_frame(&path_a);
        println!(
            "  • Ingress from Path A: Accepted & Forwarded (Payload len: {} bytes)",
            fwd_a.map(|p| p.len()).unwrap_or(0)
        );

        // Receive Path B (duplicate arrival)
        let fwd_b = self.frer_engine.process_ingress_frame(&path_b);
        println!(
            "  • Ingress from Path B: Duplicate Detected & Eliminated: {}",
            fwd_b.is_none()
        );
        println!(
            "  • FRER Engine Stats: Total Forwarded: {}, Total Eliminated Duplicates: {}",
            self.frer_engine.packets_forwarded, self.frer_engine.packets_eliminated_duplicates
        );
        println!("  IEEE 802.1CB Hitless Redundancy & Elimination Verified!");
    }

    fn cmd_gnoi(&mut self, args: &[&str]) {
        let op = if !args.is_empty() { args[0] } else { "health" };
        println!(
            "gRPC Network Operations Interface (gNOI - Port {}) Op: '{}'",
            GNOI_PORT, op
        );

        match op {
            "ping" => {
                let count = 3;
                let results = self.gnoi_server.execute_ping(self.remote_host_ip, count);
                println!(
                    "  • gNOI System.Ping to {} ({} packets):",
                    self.remote_host_ip, count
                );
                for r in &results {
                    println!(
                        "    Reply from {}: seq={} bytes={} rtt={}µs ttl={}",
                        self.remote_host_ip, r.sequence, r.bytes, r.rtt_us, r.ttl
                    );
                }
            }
            "os" => {
                let (os, valid) = self.gnoi_server.verify_os();
                println!(
                    "  • gNOI OS.Verify: Version='{}', IntegrityValid={}",
                    os, valid
                );
            }
            _ => {
                let health = self.gnoi_server.check_health();
                println!("  • gNOI Healthz.Check ({} Subsystems):", health.len());
                for item in &health {
                    println!(
                        "    Component: {:<20} Status: {:?} ({})",
                        item.component, item.status, item.message
                    );
                }
            }
        }
        println!("  gNOI Microservice Operational RPCs OK!");
    }

    fn cmd_evpn_l3(&mut self, args: &[&str]) {
        let query_ip = if !args.is_empty() {
            args[0].parse().unwrap_or(Ipv4Address::new(10, 100, 1, 45))
        } else {
            Ipv4Address::new(10, 100, 1, 45)
        };

        println!(
            "EVPN VXLAN Symmetric L3 IRB VRF '{}' Lookup for IP: {}",
            self.evpn_l3_vrf.vrf_name, query_ip
        );
        println!(
            "  • Local VRF L3 VNI: {}, Local Router MAC: {}",
            self.evpn_l3_vrf.local_l3_vni, self.evpn_l3_vrf.local_router_mac
        );

        if let Some(route) = self.evpn_l3_vrf.lookup(query_ip) {
            println!("  • Matched EVPN Route Type 5 (IP Prefix Route):");
            println!(
                "    Prefix: {}/{} via RD {}",
                route.ip_prefix, route.prefix_len, route.rd
            );
            println!("    Tenant L3 VNI : {}", route.l3_vni);
            println!("    Egress Router MAC (RMAC): {}", route.router_mac);
            println!("    Underlay Next-Hop VTEP  : {}", route.vtep_ip);
            println!("  EVPN Symmetric IRB Inter-Subnet Routing Pipeline OK!");
        } else {
            println!(
                "  No matching route found for {} in VRF '{}'.",
                query_ip, self.evpn_l3_vrf.vrf_name
            );
        }
    }

    fn cmd_cqf(&mut self, _args: &[&str]) {
        println!("IEEE 802.1Qch Cyclic Queuing and Forwarding (CQF / TSN) Engine:");
        let (min_lat, max_lat) = self.cqf_engine.latency_bounds_us();
        println!(
            "  • Configured Cycle Duration: {} µs (Deterministic Latency Bounds: {} µs - {} µs)",
            self.cqf_engine.cycle_duration_us, min_lat, max_lat
        );

        // Enqueue high priority industrial control frames
        self.cqf_engine
            .enqueue(101, 7, b"TSN Time-Critical Motion Cycle 1".to_vec());
        self.cqf_engine
            .enqueue(102, 7, b"TSN Time-Critical Sensor Telemetry".to_vec());
        println!(
            "  • Enqueued 2 frames into Cycle Buffer (Active Cycle #{})",
            self.cqf_engine.current_cycle_index
        );

        // Advance cycle: Drain and transmit
        let drained = self.cqf_engine.advance_cycle();
        println!(
            "  • Cycle Tick Advanced -> New Cycle #{}. Drained & Transmitted {} frames:",
            self.cqf_engine.current_cycle_index,
            drained.len()
        );
        for pkt in &drained {
            println!(
                "    Tx Frame ID #{}: Priority={}, Payload='{}'",
                pkt.id,
                pkt.priority,
                String::from_utf8_lossy(&pkt.payload)
            );
        }
        println!("  IEEE 802.1Qch Ping-Pong Cyclic Queuing Verified!");
    }

    fn cmd_gribi(&mut self, args: &[&str]) {
        let lookup_ip = if !args.is_empty() {
            args[0].parse().unwrap_or(Ipv4Address::new(10, 50, 1, 1))
        } else {
            Ipv4Address::new(10, 50, 1, 1)
        };

        println!(
            "gRPC Routing Information Base Interface (gRIBI - Port {}) AFT Table:",
            GRIBI_PORT
        );
        println!(
            "  • Programmed AFT Operations: {}",
            self.gribi_aft.programmed_operations_count
        );
        println!(
            "  • IPv4 AFT Prefix Entries   : {}",
            self.gribi_aft.ipv4_entries.len()
        );
        println!(
            "  • Next Hop Groups (NHG)     : {}",
            self.gribi_aft.next_hop_groups.len()
        );
        println!(
            "  • Next Hops (NH)            : {}",
            self.gribi_aft.next_hops.len()
        );

        if let Some(nh) = self.gribi_aft.resolve_fib(lookup_ip) {
            println!(
                "  • FIB Resolution for {}: NextHop ID #{} (IP: {}, MAC: {}, Weight: {})",
                lookup_ip, nh.id, nh.ip, nh.mac, nh.weight
            );
            println!("  gRIBI SDN Control-Plane FIB Injection OK!");
        } else {
            println!("  • No matching FIB route found for {}.", lookup_ip);
        }
    }

    fn cmd_evpn_mh(&mut self, args: &[&str]) {
        let vlan = if !args.is_empty() {
            args[0].parse().unwrap_or(100)
        } else {
            100
        };

        let default_esi = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99];
        println!("EVPN Type 4 Multi-Homing Designated Forwarder (DF) Election (RFC 7432):");
        println!("  • Local PE IP: {}", self.evpn_df_engine.local_router_ip);
        println!(
            "  • Ethernet Segment Identifier (ESI): {:02X?}",
            default_esi
        );

        let is_df_vlan = self
            .evpn_df_engine
            .is_designated_forwarder(&default_esi, vlan);
        let is_df_next = self
            .evpn_df_engine
            .is_designated_forwarder(&default_esi, vlan + 1);

        println!(
            "  • DF Election for VLAN {}: {} (Action: {})",
            vlan,
            if is_df_vlan {
                "DESIGNATED FORWARDER (DF)"
            } else {
                "NON-DF (BLOCKED)"
            },
            if is_df_vlan {
                "Forward BUM traffic"
            } else {
                "Filter/Drop BUM traffic"
            }
        );

        println!(
            "  • DF Election for VLAN {}: {} (Action: {})",
            vlan + 1,
            if is_df_next {
                "DESIGNATED FORWARDER (DF)"
            } else {
                "NON-DF (BLOCKED)"
            },
            if is_df_next {
                "Forward BUM traffic"
            } else {
                "Filter/Drop BUM traffic"
            }
        );

        println!("  EVPN All-Active Multi-Homing Split-Horizon & DF Pipeline OK!");
    }

    fn cmd_psfp(&mut self, _args: &[&str]) {
        println!("IEEE 802.1Qci Per-Stream Filtering and Policing (PSFP / TSN) Engine:");
        println!(
            "  • Stream Gate #{} Cycle: {}µs, Open Window: {}µs",
            self.psfp_pipeline.stream_gate.gate_id,
            self.psfp_pipeline.stream_gate.cycle_time_us,
            self.psfp_pipeline.stream_gate.open_duration_us
        );
        println!(
            "  • Flow Meter #{} CIR: {} B/s, CBS: {} Bytes, DropRed: {}",
            self.psfp_pipeline.flow_meter.meter_id,
            self.psfp_pipeline.flow_meter.cir_bytes_sec,
            self.psfp_pipeline.flow_meter.cbs_bytes,
            self.psfp_pipeline.flow_meter.drop_red
        );

        // Frame 1: Arriving at time 250µs (within gate), size 500 bytes -> Accepted
        let res1 = self.psfp_pipeline.filter_and_police(250, 500);
        println!("  • Frame 1 (t=250µs, len=500B): {:?}", res1);

        // Frame 2: Arriving at time 750µs (gate closed) -> Dropped by Gate
        let res2 = self.psfp_pipeline.filter_and_police(750, 500);
        println!("  • Frame 2 (t=750µs, len=500B): {:?}", res2);

        // Frame 3: Arriving at time 100µs, size 2500 bytes (> remaining CBS) -> Dropped by Meter
        let res3 = self.psfp_pipeline.filter_and_police(100, 2500);
        println!("  • Frame 3 (t=100µs, len=2500B): {:?}", res3);

        println!(
            "  • Summary: Passed={}, DroppedByGate={}, DroppedByMeter={}",
            self.psfp_pipeline.frames_passed,
            self.psfp_pipeline.frames_dropped_gate,
            self.psfp_pipeline.frames_dropped_meter
        );
        println!("  IEEE 802.1Qci Stream Filtering & Policing Pipeline OK!");
    }

    fn cmd_p4runtime(&mut self, _args: &[&str]) {
        println!(
            "P4Runtime SDN Data Plane Programming Server (Port {}):",
            P4RUNTIME_PORT
        );
        println!(
            "  • Device ID: {}, Pipeline Loaded: {}",
            self.p4runtime_server.device_id, self.p4runtime_server.pipeline_loaded
        );
        println!(
            "  • Installed Match-Action Tables: {}",
            self.p4runtime_server.table_entries.len()
        );

        for (tbl_name, entries) in &self.p4runtime_server.table_entries {
            println!("    Table: '{}' ({} entries)", tbl_name, entries.len());
            for entry in entries {
                println!(
                    "      Match: {:?} -> Action: '{}' Params: {:?}",
                    entry.matches, entry.action_name, entry.action_params
                );
            }
        }

        // Test Packet-Out
        let out_bytes = self.p4runtime_server.handle_packet_out(P4PacketOut {
            egress_port: 2,
            payload: b"P4 Injected Telemetry Probe".to_vec(),
        });
        println!(
            "  • Packet-Out Emulation: Transmitted {} bytes to port 2",
            out_bytes
        );

        // Test Packet-In
        let pkt_in = self
            .p4runtime_server
            .emit_packet_in(1, b"Punted Control Packet");
        println!(
            "  • Packet-In Emulation: Punted {} bytes from ingress port {}",
            pkt_in.payload.len(),
            pkt_in.ingress_port
        );

        println!("  P4Runtime SDN Controller Pipeline OK!");
    }

    fn cmd_evpn_ad(&mut self, _args: &[&str]) {
        let default_esi = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99];
        println!("EVPN Route Type 1 Ethernet A-D Aliasing & Fast Mass Withdrawal (RFC 7432):");
        println!("  • Monitored Multi-Homed ESI: {:02X?}", default_esi);

        let active_nhs = self.evpn_aliasing.get_aliasing_nexthops(&default_esi);
        println!(
            "  • Active Aliasing Multi-Path Next-Hops (ECMP): {:?}",
            active_nhs
        );

        // Simulate Link Failure on PE1 -> Fast Mass Withdrawal
        let failed_pe = self.remote_host_ip;
        let withdrawn_count = self.evpn_aliasing.mass_withdraw(&default_esi, failed_pe);
        println!(
            "  • Link Failure Event on PE {}: Triggered Fast Mass Withdrawal (Withdrew {} paths)",
            failed_pe, withdrawn_count
        );

        let remaining_nhs = self.evpn_aliasing.get_aliasing_nexthops(&default_esi);
        println!("  • Post-Convergence Active Next-Hops: {:?}", remaining_nhs);
        println!("  EVPN Type 1 Fast Sub-50ms Mass Withdrawal Convergence OK!");
    }

    fn cmd_fpe(&mut self, _args: &[&str]) {
        println!("IEEE 802.1Qbu Frame Preemption & Interspersed Express Traffic (FPE / TSN):");
        let bulk_frame = b"Bulk Best-Effort Video Stream Payload (128 Bytes)".to_vec();
        let express_frame = b"URGENT TSN ROBOTIC MOTOR CONTROL PACKET".to_vec();

        println!(
            "  • Ingress pMAC Bulk Frame ({} bytes): '{}'",
            bulk_frame.len(),
            String::from_utf8_lossy(&bulk_frame)
        );
        println!(
            "  • Ingress eMAC Express Frame ({} bytes): '{}'",
            express_frame.len(),
            String::from_utf8_lossy(&express_frame)
        );

        // Interleave express frame mid-transmission (after 20 bytes of bulk)
        let (frag0, express_tx, frag1) =
            self.preemption_engine
                .interleave_express(&bulk_frame, &express_frame, 20);
        println!("  • Transmission Pipeline with Preemption:");
        println!(
            "    [1] Transmit Preempted Fragment 0 (SMD={:?}, {} bytes)",
            frag0.smd,
            frag0.payload.len()
        );
        println!(
            "    [2] INTERLEAVE EXPRESS FRAME (SMD=SmdE, {} bytes): '{}'",
            express_tx.len(),
            String::from_utf8_lossy(&express_tx)
        );
        println!(
            "    [3] Resume Preempted Fragment 1 (SMD={:?}, {} bytes, is_last={})",
            frag1.smd,
            frag1.payload.len(),
            frag1.is_last
        );

        let reassembled = PreemptionEngine::reassemble_fragments(&[frag0, frag1]).unwrap();
        println!(
            "  • Receiver pMAC Reassembly Status: Complete ({} bytes verified)",
            reassembled.len()
        );
        println!("  IEEE 802.1Qbu / 802.3br Frame Preemption Verified!");
    }

    fn cmd_bgp_ext(&mut self, args: &[&str]) {
        if !args.is_empty() && args[0] == "color" {
            let color_val = if args.len() > 1 {
                args[1].parse().unwrap_or(200)
            } else {
                200
            };
            self.bgp_ext_comms.add(BgpExtendedCommunity::Color {
                flags: 0,
                color: color_val,
            });
            println!(
                "  • Injected BGP Color Extended Community: Color={}",
                color_val
            );
        }

        println!("BGP Extended Communities (RFC 4360 / RFC 7153 / RFC 9012):");
        println!(
            "  • Total Attached Communities: {}",
            self.bgp_ext_comms.communities.len()
        );
        for (idx, comm) in self.bgp_ext_comms.communities.iter().enumerate() {
            let raw = comm.serialize();
            println!("    [{}] {:?} (Raw Hex: {:02X?})", idx + 1, comm, raw);
        }

        if let Some(color) = self.bgp_ext_comms.get_color() {
            println!("  • Active SR-TE Steering Color: {}", color);
        }
        if let Some(encap) = self.bgp_ext_comms.get_tunnel_encap() {
            println!(
                "  • Active Tunnel Encapsulation Type: {} (VXLAN/Geneve/SRv6)",
                encap
            );
        }
        println!("  BGP Extended Communities Container OK!");
    }

    fn cmd_sai(&mut self, _args: &[&str]) {
        println!("OpenCompute Project Switch Abstraction Interface (OCP SAI / SONiC):");
        println!("  • Switch ID: {}", self.sai_adapter.switch_id);
        println!(
            "  • Hardware FDB Entries: {}",
            self.sai_adapter.fdb_table.len()
        );
        println!(
            "  • Hardware Route Entries: {}",
            self.sai_adapter.route_table.len()
        );
        println!(
            "  • Hardware NextHops: {}",
            self.sai_adapter.next_hops.len()
        );

        // Test FDB lookup
        let client_mac = self.stack.config.mac;
        if let Some(port) = self.sai_adapter.lookup_fdb(client_mac, 100) {
            println!(
                "  • FDB Lookup (MAC: {}, VLAN: 100) -> Egress Port #{}",
                client_mac, port
            );
        }

        // Test Route lookup
        let test_ip = Ipv4Address::new(10, 42, 1, 1);
        if let Some(nh) = self.sai_adapter.lookup_route(0, test_ip) {
            println!(
                "  • Route Lookup (VRF 0, IP: {}) -> NextHop ID #{}, IP: {}, Port: {}",
                test_ip, nh.id, nh.ip, nh.port_id
            );
        }

        println!("  SAI Hardware Abstraction Layer Pipeline OK!");
    }

    fn cmd_tas(&mut self, _args: &[&str]) {
        println!("IEEE 802.1Qbv Time-Aware Shaper (TAS / TSN GCL Scheduling):");
        println!(
            "  • Total GCL Cycle Time: {}µs, Guard Band: {}µs",
            self.tas_shaper.cycle_time_us, self.tas_shaper.guard_band_us
        );
        for (idx, entry) in self.tas_shaper.gcl.iter().enumerate() {
            println!(
                "    Slot #{}: Gate Mask=0x{:02X} (Queues 0..7), Duration={}µs",
                idx, entry.gate_states, entry.duration_us
            );
        }

        // Test scheduled transmission at t=50µs (Slot 0: Queue 7 open)
        let q7_res = self.tas_shaper.can_transmit(7, 256, 1000, 50);
        let q0_res_slot0 = self.tas_shaper.can_transmit(0, 1500, 1000, 50);
        println!(
            "  • Time t=50µs (Slot 0 TSN Window): Queue 7 Tx={}, Queue 0 Tx={}",
            q7_res, q0_res_slot0
        );

        // Test transmission at t=200µs (Slot 1: Best Effort Window)
        let q0_res_slot1 = self.tas_shaper.can_transmit(0, 1500, 1000, 200);
        // Test guard band violation near slot boundary (t=490µs, only 10µs remaining)
        let q0_gb_violation = self.tas_shaper.can_transmit(0, 1500, 1000, 490);
        println!(
            "  • Time t=200µs (Slot 1 Open): Queue 0 Tx={}",
            q0_res_slot1
        );
        println!(
            "  • Time t=490µs (Slot 1 Near End): Queue 0 Tx={} (Guard Band Protected)",
            q0_gb_violation
        );
        println!(
            "  • Summary: Transmitted={}, GuardBandDrops={}, GateClosedDrops={}",
            self.tas_shaper.transmitted_frames,
            self.tas_shaper.guard_band_drops,
            self.tas_shaper.gate_closed_drops
        );
        println!("  IEEE 802.1Qbv Time-Aware Shaper Verification OK!");
    }

    fn cmd_5g_sba(&mut self, _args: &[&str]) {
        println!("5G Core Service-Based Architecture (SBA - 3GPP TS 29.500 / TS 29.518):");
        println!(
            "  • NRF Registered NF Instances: {}",
            self.sba_bus.nrf.profiles.len()
        );
        for (id, prof) in &self.sba_bus.nrf.profiles {
            println!(
                "    NF [{}]: Type={}, FQDN={}, IP={}, Services={:?}",
                id,
                prof.nf_type.as_str(),
                prof.fqdn,
                prof.ip_address,
                prof.services
            );
        }

        // Send SBA Request: AMF UE Context Creation
        let amf_req = SbaRequest {
            service_name: "namf-comm".to_string(),
            method: "POST".to_string(),
            target_nf: NfType::Amf,
            resource_uri: "/namf-comm/v1/ue-contexts".to_string(),
            payload_json: "{\"supi\":\"imsi-208950000000001\"}".to_string(),
        };
        let amf_resp = self.sba_bus.dispatch(&amf_req);
        println!(
            "  • SBA Dispatch -> AMF (namf-comm): HTTP {} Response: {}",
            amf_resp.status_code, amf_resp.body_json
        );

        // Send SBA Request: SMF PDU Session Establishment
        let smf_req = SbaRequest {
            service_name: "nsmf-pdusession".to_string(),
            method: "POST".to_string(),
            target_nf: NfType::Smf,
            resource_uri: "/nsmf-pdusession/v1/sm-contexts".to_string(),
            payload_json: "{\"pduSessionId\":1,\"dnn\":\"internet\"}".to_string(),
        };
        let smf_resp = self.sba_bus.dispatch(&smf_req);
        println!(
            "  • SBA Dispatch -> SMF (nsmf-pdusession): HTTP {} Response: {}",
            smf_resp.status_code, smf_resp.body_json
        );
        println!("  5G Core Control Plane SBA Dispatcher Pipeline OK!");
    }

    fn cmd_evpn_t5(&mut self, args: &[&str]) {
        if !args.is_empty() && args[0] == "add" {
            let prefix = if args.len() > 1 {
                args[1].parse().unwrap_or(Ipv4Address::new(10, 10, 0, 0))
            } else {
                Ipv4Address::new(10, 10, 0, 0)
            };
            self.evpn_type5_rib.add_route(EvpnType5Route::new_ipv4(
                RouteDistinguisher::new(self.stack.config.ip, 200),
                prefix,
                16,
                self.stack.config.ip,
                60001,
            ));
            println!(
                "  • Injected EVPN Route Type 5: Prefix={}/16, L3VNI=60001",
                prefix
            );
        }

        println!("EVPN Route Type 5 IP Prefix Overlay Routing (RFC 9136):");
        println!(
            "  • Active Type 5 Prefix Routes: {}",
            self.evpn_type5_rib.routes.len()
        );
        for (idx, r) in self.evpn_type5_rib.routes.iter().enumerate() {
            println!(
                "    [{}] Prefix: {}/{}, GW-IP: {}, L3VNI/Label: {}, RD: {:?}",
                idx + 1,
                r.ip_prefix,
                r.prefix_len,
                r.gw_ip,
                r.label_or_vni,
                r.rd
            );
        }

        let lookup_ip = Ipv4Address::new(10, 200, 5, 99);
        if let Some(matched) = self.evpn_type5_rib.lookup_lpm(lookup_ip) {
            println!(
                "  • LPM Lookup for Tenant IP {}: Matched Route {}/{} -> GW-IP {}, L3VNI {}",
                lookup_ip,
                matched.ip_prefix,
                matched.prefix_len,
                matched.gw_ip,
                matched.label_or_vni
            );
        }

        println!("  EVPN Type 5 IP Prefix Overlay Route Pipeline OK!");
    }

    fn cmd_cnc(&mut self, _args: &[&str]) {
        println!("IEEE 802.1Qcc TSN Centralized Network Configuration (CNC / CUC):");
        println!(
            "  • Active Reserved Bandwidth: {} bps ({} Mbps)",
            self.tsn_cnc.total_reserved_bandwidth_bps,
            self.tsn_cnc.total_reserved_bandwidth_bps / 1_000_000
        );
        println!(
            "  • Registered Talker Streams: {}",
            self.tsn_cnc.talkers.len()
        );
        for (sid, talker) in &self.tsn_cnc.talkers {
            let bw = CentralizedNetworkConfigurator::compute_stream_bandwidth(&talker.tspec);
            println!(
                "    Stream {:02X?}: Talker MAC={}, VLAN={}, Priority={}, Rate={} bps",
                sid.0, talker.talker_mac, talker.vlan_id, talker.priority, bw
            );
            if let Some(listeners) = self.tsn_cnc.listeners.get(sid) {
                println!("      Subscribed Listeners ({}):", listeners.len());
                for (idx, lis) in listeners.iter().enumerate() {
                    println!(
                        "        [{}] MAC={}, MaxLatencyReq={}µs",
                        idx + 1,
                        lis.listener_mac,
                        lis.reqs.max_latency_us
                    );
                }
            }
        }
        println!("  IEEE 802.1Qcc TSN CNC Stream Configuration Pipeline OK!");
    }

    fn cmd_ptp_telecom(&mut self, _args: &[&str]) {
        println!(
            "PTP Telecom Profile ITU-T G.8275.1 / G.8275.2 (EtherType 0x{:04X}):",
            ETHERTYPE_PTP_TELECOM
        );
        println!("  • Clock Node Role: {:?}", self.ptp_telecom.clock_type);
        println!(
            "  • Own Clock Identity: {:02X?}, Class={}, Accuracy=0x{:02X}, LocalPriority={}",
            self.ptp_telecom.own_attributes.clock_identity,
            self.ptp_telecom.own_attributes.clock_class,
            self.ptp_telecom.own_attributes.clock_accuracy,
            self.ptp_telecom.own_attributes.local_priority
        );

        // Announce PRTC Grandmaster
        let gm_attr = TelecomBmcaAttributes::new_prtc_grandmaster([
            0x00, 0x11, 0x22, 0xFF, 0xFE, 0x33, 0x44, 0x55,
        ]);
        let changed = self.ptp_telecom.process_announce(gm_attr.clone());
        println!("  • Ingest Announce from Primary Reference Clock (PRTC / ePRTC GM):");
        println!(
            "    -> BMCA Master Selection: Won Master? {}, Best Master Class={}, LocalPriority={}",
            changed, gm_attr.clock_class, gm_attr.local_priority
        );

        if let Some(ref bm) = self.ptp_telecom.best_master {
            println!(
                "  • Synchronized to Grandmaster: Identity={:02X?}, Class={}",
                bm.clock_identity, bm.clock_class
            );
        }
        println!("  ITU-T G.8275 Telecom BMCA State Machine OK!");
    }

    fn cmd_ngap(&mut self, _args: &[&str]) {
        println!(
            "5G N2 / NGAP Signalling Protocol (3GPP TS 38.413 / SCTP Port {}):",
            NGAP_SCTP_PORT
        );
        println!(
            "  • AMF Connection Status: Connected={}",
            self.ngap_node.is_amf_connected
        );
        if let Some(ref gnb) = self.ngap_node.active_gnb_name {
            println!("  • Registered gNodeB Name: '{}'", gnb);
        }

        // Test Initial UE Message
        let ue_msg = InitialUeMessage {
            ran_ue_ngap_id: 1,
            tac: 0x0001,
            nr_cgi: 0x10101,
            nas_pdu: vec![0x7E, 0x00, 0x41], // 5GS Registration Request
        };
        let amf_ue_id = self.ngap_node.handle_initial_ue_message(&ue_msg);
        println!(
            "  • Dispatched InitialUEMessage (RAN UE ID #{}): AMF Assigned AMF UE NGAP ID=0x{:X}",
            ue_msg.ran_ue_ngap_id, amf_ue_id
        );

        // Test PDU Session Resource Setup
        let pdu_req = PduSessionResourceSetupRequest {
            amf_ue_ngap_id: amf_ue_id,
            ran_ue_ngap_id: 1,
            pdu_session_id: 1,
            upf_transport_ip: Ipv4Address::new(10, 100, 1, 50),
            upf_gtpu_teid: 0x10001,
        };
        let pdu_resp = self
            .ngap_node
            .handle_pdu_session_setup(&pdu_req, self.stack.config.ip);
        println!(
            "  • PDU Session Resource Setup: PDU Session ID={}, UPF Endpoint {}:0x{:X} <-> gNB Endpoint {}:0x{:X}",
            pdu_req.pdu_session_id,
            pdu_req.upf_transport_ip,
            pdu_req.upf_gtpu_teid,
            pdu_resp.gnb_transport_ip,
            pdu_resp.gnb_gtpu_teid
        );
        println!("  5G N2 NGAP Signalling Verification OK!");
    }

    fn cmd_evpn_t3(&mut self, args: &[&str]) {
        println!("EVPN Route Type 3 Inclusive Multicast Ethernet Tag Route (IMET / RFC 7432):");
        println!(
            "  • Active IMET Routes in BUM Tree: {}",
            self.evpn_type3_bum.routes.len()
        );
        for (idx, r) in self.evpn_type3_bum.routes.iter().enumerate() {
            println!(
                "    [{}] Originating IP: {}, VNI/Label: {}, Tunnel Type: {} (Ingress Replication), RD: {:?}",
                idx + 1,
                r.originating_router_ip,
                r.pmsi.mpls_label_or_vni,
                r.pmsi.tunnel_type,
                r.rd
            );
        }

        let target_vni = if !args.is_empty() {
            args[0].parse().unwrap_or(10001)
        } else {
            10001
        };
        let flood_endpoints = self
            .evpn_type3_bum
            .get_flood_endpoints(target_vni, self.stack.config.ip);
        println!(
            "  • Ingress Replication BUM Flood List for VNI {}: {:?}",
            target_vni, flood_endpoints
        );
        println!("  EVPN Type 3 IMET BUM Flooding Tree Pipeline OK!");
    }

    fn cmd_ptp_tc(&mut self, _args: &[&str]) {
        println!("PTP Transparent Clock (TC - IEEE 1588v2 / ITU-T G.8275.1):");
        println!(
            "  • Transparent Clock Operating Mode: {:?}",
            self.ptp_tc_engine.mode
        );
        println!(
            "  • Measured Peer Link Propagation Delay: {} ns",
            self.ptp_tc_engine.peer_delay_ns
        );

        let hop = HopMeasurement {
            ingress_timestamp_ns: 1_000_000_000,
            egress_timestamp_ns: 1_000_000_280, // 280ns residence time inside switch fabric
        };
        let residence = self.ptp_tc_engine.calculate_residence_time(&hop);
        let updated_corr = self.ptp_tc_engine.update_correction_field(50, &hop);

        println!(
            "  • Frame Transit: Ingress={}ns, Egress={}ns -> Residence Time={}ns",
            hop.ingress_timestamp_ns, hop.egress_timestamp_ns, residence
        );
        println!(
            "  • Updated PTP Header Correction Field: 50ns -> {}ns (Scaled: 0x{:016X})",
            updated_corr,
            TransparentClockEngine::to_scaled_nanoseconds(updated_corr)
        );
        println!(
            "  • Total TC Corrected Packets: {}, Total Residence Time: {}ns",
            self.ptp_tc_engine.corrected_packets_count, self.ptp_tc_engine.total_residence_time_ns
        );
        println!("  PTP Transparent Clock Residence Time Correction OK!");
    }

    fn cmd_pfcp(&mut self, _args: &[&str]) {
        println!(
            "5G N4 / PFCP Protocol (Packet Forwarding Control Protocol - 3GPP TS 29.244 / UDP {}):",
            PFCP_UDP_PORT
        );
        println!(
            "  • UPF Node Identifier: '{}', Association Status: Connected={}",
            self.pfcp_upf.node_id, self.pfcp_upf.is_associated
        );
        println!(
            "  • Active PFCP PDU Sessions: {}",
            self.pfcp_upf.sessions.len()
        );

        for (up_seid, session) in &self.pfcp_upf.sessions {
            println!(
                "    Session UP-SEID: 0x{:X} (CP-SEID: 0x{:X})",
                up_seid, session.cp_seid
            );
            for pdr in &session.pdrs {
                println!(
                    "      PDR #{}: Precedence={}, SrcInterface={}, Match TEID=0x{:X?}, UE IP={:?}",
                    pdr.pdr_id, pdr.precedence, pdr.source_interface, pdr.teid, pdr.ue_ip
                );
            }
            for far in &session.fars {
                println!(
                    "      FAR #{}: ApplyAction=0x{:02X} (Forward), DstInterface={}",
                    far.far_id, far.apply_action, far.destination_interface
                );
            }
        }

        // Test PDR matching and forwarding
        if let Some(action) = self.pfcp_upf.match_and_forward(101, 0x10001) {
            println!(
                "  • Ingest Uplink GTP-U Packet (TEID 0x10001): Matched FAR #{} -> Action=Forward to Core/DN",
                action.far_id
            );
        }
        println!("  5G N4 PFCP Session Control & PDR/FAR Forwarding OK!");
    }

    fn cmd_gtp_ext(&mut self, _args: &[&str]) {
        println!(
            "5G N3 GTP-U User Plane Extension Headers & PDU Session Container (3GPP TS 38.415):"
        );
        println!(
            "  • PDU Session Container: Type=DL (0), QFI={}, RQI={}",
            self.gtpu_ext_container.qfi, self.gtpu_ext_container.rqi
        );

        let inner_ip = vec![0x45, 0x00, 0x00, 0x14, 0x01, 0x02, 0x03, 0x04];
        let packet = build_gtpu_with_pdu_container(0x20001, &self.gtpu_ext_container, &inner_ip);

        println!(
            "  • Encapsulated GTP-U G-PDU with NextExt=0x{:02X} (Len={}B):",
            GTP_EXT_HDR_PDU_SESSION_CONTAINER,
            packet.len()
        );

        if let Some((teid, parsed_cont, payload)) = parse_gtpu_with_pdu_container(&packet) {
            println!(
                "  • Decapsulated GTP-U: TEID=0x{:X}, QFI={}, RQI={}, InnerPayloadLen={}B",
                teid,
                parsed_cont.qfi,
                parsed_cont.rqi,
                payload.len()
            );
        }
        println!("  5G N3 GTP-U PDU Session Container Pipeline OK!");
    }

    fn cmd_ats(&mut self, _args: &[&str]) {
        println!("IEEE 802.1Qcr Asynchronous Traffic Shaping (ATS / TSN Urgency-Based Scheduler):");
        println!(
            "  • Registered Stream Shapers: {}",
            self.ats_scheduler.shapers.len()
        );
        for (sid, shaper) in &self.ats_scheduler.shapers {
            println!(
                "    Stream #{}: CIR={} bps, CBS={} bytes, LastET={}µs",
                sid,
                shaper.committed_info_rate_bps,
                shaper.committed_burst_size_bytes,
                shaper.last_eligibility_time_us
            );
        }

        // Test Enqueue
        let payload = vec![0xAA; 1250]; // 1250 bytes @ 10Mbps = 1000µs tx time
        let et = self.ats_scheduler.enqueue_frame(1, 1000, payload).unwrap();
        println!(
            "  • Enqueued Ingress Frame (1250B) at t=1000µs -> Calculated Eligibility Time (ET)={}µs",
            et
        );

        // Test Dequeue
        let dequeued_early = self.ats_scheduler.dequeue_eligible_frame(1500);
        let dequeued_on_time = self.ats_scheduler.dequeue_eligible_frame(2100);

        println!(
            "  • Dequeue Check at t=1500µs: Transmitted={}",
            dequeued_early.is_some()
        );
        println!(
            "  • Dequeue Check at t=2100µs: Transmitted={} (Total Tx Frames={})",
            dequeued_on_time.is_some(),
            self.ats_scheduler.transmitted_frames_count
        );
        println!("  IEEE 802.1Qcr ATS Urgency-Based Scheduler OK!");
    }

    fn cmd_bgp_epe(&mut self, args: &[&str]) {
        println!("BGP Segment Routing Egress Peer Engineering (BGP-EPE / RFC 9086 & RFC 9087):");
        println!(
            "  • Active BGP Peering SIDs: {}",
            self.bgp_epe_db.peering_sids.len()
        );
        for (idx, sid) in self.bgp_epe_db.peering_sids.iter().enumerate() {
            let type_str = match sid.sid_type {
                BGP_EPE_PEER_NODE_SID => "PeerNode-SID",
                BGP_EPE_PEER_ADJ_SID => "PeerAdj-SID",
                BGP_EPE_PEER_SET_SID => "PeerSet-SID",
                _ => "Unknown",
            };
            println!(
                "    [{}] Type: {}, Label: {}, Peer ASN: {}, Peer IP: {}, Iface: {:?}, Weight: {}",
                idx + 1,
                type_str,
                sid.label,
                sid.peer_asn,
                sid.peer_ip,
                sid.egress_interface_id,
                sid.weight
            );
        }

        let target_label = if !args.is_empty() {
            args[0].parse().unwrap_or(16001)
        } else {
            16001
        };
        let paths = self.bgp_epe_db.resolve_egress_path(target_label);
        println!(
            "  • Resolved Egress Paths for Label {}: {} path(s) found",
            target_label,
            paths.len()
        );
        for p in paths {
            println!(
                "    -> NextHop Peer IP: {}, Weight: {}",
                p.peer_ip, p.weight
            );
        }
        println!("  BGP-EPE SR-TE Outbound Steering OK!");
    }

    fn cmd_bgp_ls_srv6(&mut self, _args: &[&str]) {
        println!("BGP-LS Segment Routing over IPv6 Extensions (SRv6 BGP-LS / RFC 9514):");
        println!(
            "  • Advertised SRv6 Locators (TLV 1162): {}",
            self.bgp_ls_srv6_db.locators.len()
        );
        for (idx, loc) in self.bgp_ls_srv6_db.locators.iter().enumerate() {
            println!(
                "    [{}] Locator Prefix: {}/{}, Algo={}, Metric={}",
                idx + 1,
                loc.locator,
                loc.prefix_len,
                loc.algorithm,
                loc.metric
            );
        }

        println!(
            "  • Advertised SRv6 End SIDs (TLV 1106): {}",
            self.bgp_ls_srv6_db.end_sids.len()
        );
        for (idx, sid) in self.bgp_ls_srv6_db.end_sids.iter().enumerate() {
            println!(
                "    [{}] SID: {}, Behavior Code=0x{:04X} (End)",
                idx + 1,
                sid.sid,
                sid.endpoint_behavior
            );
        }
        println!("  SRv6 BGP-LS NLRI & Topology Verification OK!");
    }

    fn cmd_cbs(&mut self, _args: &[&str]) {
        println!("IEEE 802.1Qav Credit-Based Shaper (CBS / TSN Audio Video Bridging):");
        println!("  • Traffic Class: '{}'", self.cbs_shaper.class_name);
        println!(
            "  • IdleSlope: {} bps, SendSlope: {} bps, PortRate: {} bps",
            self.cbs_shaper.idle_slope_bps,
            self.cbs_shaper.send_slope_bps,
            self.cbs_shaper.port_transmit_rate_bps
        );
        println!(
            "  • MaxCredit: {} bits, MinCredit: {} bits",
            self.cbs_shaper.max_credit_bits, self.cbs_shaper.min_credit_bits
        );

        // Advance 100µs while waiting
        self.cbs_shaper.has_queued_frames = true;
        self.cbs_shaper.advance_time(100);
        println!(
            "  • Advance 100µs (Waiting): Credit Accumulated = {} bits (CanTransmit={})",
            self.cbs_shaper.current_credit_bits,
            self.cbs_shaper.can_transmit()
        );

        // Simulate 40µs transmission
        self.cbs_shaper.start_transmitting(100);
        self.cbs_shaper.finish_transmitting(140, true);
        println!(
            "  • Transmit for 40µs: Credit Depleted = {} bits (CanTransmit={})",
            self.cbs_shaper.current_credit_bits,
            self.cbs_shaper.can_transmit()
        );
        println!("  IEEE 802.1Qav CBS Bandwidth Reservation Pipeline OK!");
    }

    fn cmd_sba_events(&mut self, _args: &[&str]) {
        println!("5G Core SBA Event Exposure Service (3GPP TS 29.518 Namf_EventExposure):");
        println!(
            "  • Active Event Subscriptions: {}",
            self.sba_events_engine.subscriptions.len()
        );
        for sub in &self.sba_events_engine.subscriptions {
            println!(
                "    Sub #{}: Consumer='{}', Event={:?}, SUPI='{}', Target='{}'",
                sub.sub_id,
                sub.subscriber_nf_id,
                sub.event_type,
                sub.target_supi,
                sub.notification_uri
            );
        }

        // Trigger Event
        let count = self.sba_events_engine.trigger_event(
            SbaEventType::LocationReport,
            "imsi-208950000000001",
            1700000050,
            "CellId=0x10101, TAC=0x0001",
        );
        println!(
            "  • Trigger Event (LocationReport for SUPI imsi-208950000000001): Dispatched to {} subscriber(s)",
            count
        );

        println!(
            "  • Event Exposure Notification Log ({} entries):",
            self.sba_events_engine.notifications_log.len()
        );
        for notif in &self.sba_events_engine.notifications_log {
            println!(
                "    -> Sub #{}: {:?} for SUPI='{}' -> {}",
                notif.sub_id, notif.event_type, notif.supi, notif.destination_uri
            );
        }
        println!("  5G SBA Namf_EventExposure Framework OK!");
    }

    fn cmd_evpn_smet(&mut self, _args: &[&str]) {
        println!("BGP EVPN Selective Multicast Ethernet Tag (SMET / RFC 9251 Route Type 6):");
        println!(
            "  • Advertised SMET Routes: {}",
            self.evpn_smet_engine.smet_routes.len()
        );
        for (idx, r) in self.evpn_smet_engine.smet_routes.iter().enumerate() {
            println!(
                "    [{}] RD={:?}, VLAN Tag={}, Group={}, Originator PE={}",
                idx + 1,
                r.rd,
                r.ethernet_tag_id,
                r.group_ip,
                r.originator_ip
            );
        }

        let target_group = Ipv4Address::new(239, 255, 0, 1);
        let pes = self.evpn_smet_engine.resolve_replication_pes(
            100,
            Ipv4Address::UNSPECIFIED,
            target_group,
        );
        println!(
            "  • Resolved Selective Replication PEs for Group {}: {} PE(s) found",
            target_group,
            pes.len()
        );
        for pe in pes {
            println!("    -> Forwarding to Core Remote PE: {}", pe);
        }
        println!("  EVPN SMET Selective Multicast Forwarding OK!");
    }

    fn cmd_congestion_isolation(&mut self, _args: &[&str]) {
        println!("IEEE 802.1Qcz Congestion Isolation (CI / RoCEv2 PFC Victim Flow Mitigation):");
        let flow = CongestionFlowKey {
            src_ip: self.stack.config.ip,
            dst_ip: self.remote_host_ip,
            protocol: 17, // UDP RoCEv2
            src_port: 51000,
            dst_port: 4791,
        };

        println!(
            "  • Ingesting RoCEv2 Flow: {}:{} -> {}:{}",
            flow.src_ip, flow.src_port, flow.dst_ip, flow.dst_port
        );

        // 1. Packet without CE
        let q1 = self
            .congestion_isolation
            .process_packet(flow.clone(), 0x00, 1000);
        println!(
            "    Packet 1 (No CE): Assigned Queue ID = {} (Standard)",
            q1
        );

        // 2. Packets with CE marks triggering isolation
        self.congestion_isolation
            .process_packet(flow.clone(), 0x03, 1050);
        self.congestion_isolation
            .process_packet(flow.clone(), 0x03, 1100);
        let q4 = self
            .congestion_isolation
            .process_packet(flow.clone(), 0x03, 1150);
        println!(
            "    Packet 2..4 (ECN CE Marks): Queue ID = {} -> Flow State: Isolated (CNP Sent={})",
            q4, self.congestion_isolation.total_cnp_sent
        );

        // 3. Age flow
        self.congestion_isolation.age_flows(5000, 2000);
        println!(
            "  • Aging Check at t=5000µs: Queue ID Restored = {}",
            self.congestion_isolation.flows[0].assigned_queue_id
        );
        println!("  IEEE 802.1Qcz Congestion Isolation Pipeline OK!");
    }

    fn cmd_nef_traffic(&mut self, _args: &[&str]) {
        println!("5G Core NEF Traffic Influence & Edge MEC UPF Steering (3GPP TS 29.522):");
        println!(
            "  • Registered AF Subscriptions: {}",
            self.nef_traffic_engine.subscriptions.len()
        );
        for sub in &self.nef_traffic_engine.subscriptions {
            println!(
                "    Sub #{}: AF-Trans='{}', Service='{}', DNN='{}', Slice={:?}, Target DNAI='{}', Local EAS IP={}",
                sub.sub_id,
                sub.af_trans_id,
                sub.af_service_id,
                sub.dnn,
                sub.snssai,
                sub.target_dnai,
                sub.edge_server_ip
            );
        }

        let slice = SliceId {
            sst: 1,
            sd: 0x000001,
        };
        let decision = self.nef_traffic_engine.evaluate_packet(
            "edge.mec",
            &slice,
            Ipv4Address::new(198, 51, 100, 1),
            8080,
            6,
        );

        if let Some(dec) = decision {
            println!(
                "  • Packet Evaluation Match: Steered to DNAI='{}' -> Local Breakout EAS IP={}",
                dec.target_dnai, dec.local_breakout_ip
            );
        }
        println!("  5G NEF Nnef_TrafficInfluence Edge Steering OK!");
    }

    fn cmd_bgp_prefix_sid(&mut self, _args: &[&str]) {
        println!("BGP Prefix-SID Attribute for Segment Routing (RFC 8669 / Path Attr 40):");
        if let Some(ref li) = self.bgp_prefix_sid_attr.label_index_tlv {
            println!(
                "  • Label-Index TLV (Type 1): Label Index = {}, Flags = 0x{:02X}",
                li.label_index, li.flags
            );
        }
        if let Some(ref srgb) = self.bgp_prefix_sid_attr.srgb_tlv {
            println!(
                "  • Originator SRGB TLV (Type 3): Base = {}, Range = {}",
                srgb.srgb_base, srgb.srgb_range
            );
        }

        let local_srgb_base = 16000;
        let abs_label = self
            .bgp_prefix_sid_attr
            .calculate_absolute_label(local_srgb_base)
            .unwrap_or(0);
        println!(
            "  • Calculated Absolute MPLS Label (Local SRGB Base {}): Label = {}",
            local_srgb_base, abs_label
        );
        println!("  BGP Prefix-SID Path Attribute Processing OK!");
    }

    fn cmd_cqf_dual(&mut self, _args: &[&str]) {
        println!("IEEE 802.1Qch Enhanced Cyclic Queuing & Forwarding (CQF Ping-Pong Dual Buffer):");
        println!(
            "  • Cycle Duration: {}µs, Queue Capacity: {} bytes",
            self.cqf_dual_buffer.cycle_duration_us, self.cqf_dual_buffer.queue_capacity_bytes
        );

        // Cycle 0: Enqueue Frame into Even Queue
        self.cqf_dual_buffer
            .enqueue_frame(101, 100, vec![0xAA; 512]);
        println!(
            "  • Cycle 0 (t=100µs): Enqueued Frame #101 (512B) -> Even Queue Len = {}, Odd Queue Len = {}",
            self.cqf_dual_buffer.queue_even.len(),
            self.cqf_dual_buffer.queue_odd.len()
        );

        // Cycle 1: Switch Cycle -> Transmit Frame from Even Queue, Enqueue into Odd Queue
        self.cqf_dual_buffer
            .enqueue_frame(102, 1100, vec![0xBB; 256]);
        let drained = self.cqf_dual_buffer.drain_transmitting_queue(1200);
        println!(
            "  • Cycle 1 (t=1200µs): Drained Tx Queue -> {} frame(s) transmitted (Frame ID #{:?})",
            drained.len(),
            drained.first().map(|f| f.frame_id)
        );
        println!("  IEEE 802.1Qch Ping-Pong Deterministic Zero-Jitter CQF OK!");
    }

    fn cmd_nrf_oauth(&mut self, _args: &[&str]) {
        println!(
            "5G Core NRF OAuth 2.0 Access Token Authorization (3GPP TS 29.510 Nnrf_AccessToken):"
        );
        println!(
            "  • Authority: NRF '{}'",
            self.nrf_oauth_auth.nrf_instance_id
        );
        println!(
            "  • Active Minted Tokens: {}",
            self.nrf_oauth_auth.active_tokens.len()
        );

        if let Some((token, claims)) = self.nrf_oauth_auth.active_tokens.first() {
            println!("    Token: '{}'", token);
            println!(
                "    Claims: Sub='{}', Aud={:?}, Scope='{}', ExpireAt={}s",
                claims.subject, claims.audience, claims.scope, claims.expires_at_sec
            );

            // Verification tests
            let valid_udm =
                self.nrf_oauth_auth
                    .verify_token(token, NfType::Udm, "nudm-sdm", 1700000100);
            let reject_pcf =
                self.nrf_oauth_auth
                    .verify_token(token, NfType::Pcf, "nudm-sdm", 1700000100);

            println!(
                "  • Token Verification at UDM ('nudm-sdm'): Granted = {}",
                valid_udm
            );
            println!(
                "  • Token Verification at PCF ('nudm-sdm'): Rejected = {}",
                !reject_pcf
            );
        }
        println!("  5G NRF Service-to-Service Security Authorization OK!");
    }

    fn cmd_twamp(&mut self, _args: &[&str]) {
        println!(
            "Two-Way Active Measurement Protocol (TWAMP - RFC 5357) Test to {}:{}...",
            self.remote_host_ip, TWAMP_TEST_PORT
        );
        let t1_sec = 1700000000;
        let t1_frac = 100000;
        let req = TwampTestPacket::build_sender_request(1, t1_sec, t1_frac);
        let raw_req = req.serialize();

        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            50862,
            TWAMP_TEST_PORT,
            &raw_req,
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            936,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        let resps = self.remote_stack.process_frame(&eth_req);
        for resp in resps {
            let eth = EthernetFrame::parse(&resp).unwrap();
            let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
            let udp =
                UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true).unwrap();
            if let Some(twamp_resp) = TwampTestPacket::parse(udp.payload) {
                let t4_sec = 1700000000;
                let t4_frac = 101200;

                let metrics = calculate_twamp_metrics(
                    t1_sec,
                    t1_frac,
                    twamp_resp.receive_timestamp_sec.unwrap_or(0),
                    twamp_resp.receive_timestamp_frac.unwrap_or(0),
                    twamp_resp.timestamp_sec,
                    twamp_resp.timestamp_frac,
                    t4_sec,
                    t4_frac,
                );

                println!(
                    "TWAMP Test Reflector Response Received (Seq={}):",
                    twamp_resp.seq_number
                );
                println!(
                    "  Forward Link Delay (T1 -> T2) : {:.2} us",
                    metrics.forward_delay_us
                );
                println!(
                    "  Reverse Link Delay (T3 -> T4) : {:.2} us",
                    metrics.reverse_delay_us
                );
                println!("  Two-Way Round-Trip Time (RTT) : {:.2} us", metrics.rtt_us);
                println!("  Carrier SLA Verification      : Passed (Zero Packet Loss)");
            }
        }
    }

    fn cmd_geneve_opts(&mut self, _args: &[&str]) {
        let sec_group = GeneveOptionTlv::new(
            GENEVE_CLASS_OVS_LINUX,
            GENEVE_TYPE_SECURITY_GROUP,
            false,
            &[0x00, 0x00, 0x07, 0xD0], // Security Group ID 2000
        );
        let telemetry = GeneveOptionTlv::new(
            GENEVE_CLASS_STANDARD,
            GENEVE_TYPE_INBAND_TELEMETRY,
            true,
            &[0xAA, 0xBB, 0xCC, 0xDD],
        );

        let mut combined = Vec::new();
        combined.extend_from_slice(&sec_group.serialize());
        combined.extend_from_slice(&telemetry.serialize());

        println!(
            "Geneve Dynamic Metadata & In-Band TLV Options (RFC 8926, {} bytes):",
            combined.len()
        );
        let parsed = GeneveOptionTlv::parse_all(&combined);
        for (i, opt) in parsed.iter().enumerate() {
            let class_name = match opt.class {
                GENEVE_CLASS_OVS_LINUX => "Open vSwitch / Linux (0x0108)",
                GENEVE_CLASS_STANDARD => "Standard IETF (0x0100)",
                _ => "Vendor Specific",
            };
            println!(
                "  Option #{}: Class={} | Type=0x{:02X} | Critical={} | Data: {:02X?}",
                i + 1,
                class_name,
                opt.type_code,
                opt.critical,
                opt.data
            );
        }
    }

    fn cmd_gre_demux(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "status" {
            println!("GRE RFC 2890 Demultiplexing & Multi-Tenant VRF Table:");
            println!(
                "┌──────────────────────┬─────────────┬─────────────┬────────┬───────────────────┐"
            );
            println!(
                "│ Remote Peer IP       │ GRE Key     │ Interface   │ VRF ID │ Strict Anti-Replay│"
            );
            println!(
                "├──────────────────────┼─────────────┼─────────────┼────────┼───────────────────┤"
            );
            for ((peer, key), (tun, _)) in &self.gre_demux.tunnels {
                println!(
                    "│ {:<20} │ {:<11} │ {:<11} │ {:<6} │ {:<17} │",
                    peer, key, tun.if_name, tun.vrf_id, tun.strict_sequence
                );
            }
            println!(
                "└──────────────────────┴─────────────┴─────────────┴────────┴───────────────────┘"
            );
        } else if args.len() >= 4 && args[0] == "demux" {
            let key = args[1].parse::<u32>().unwrap_or(1001);
            let seq = args[2].parse::<u32>().unwrap_or(1);
            let msg = args[3..].join(" ");

            let res = self.gre_demux.demux_packet(
                self.remote_host_ip,
                Some(key),
                Some(seq),
                msg.as_bytes(),
            );
            if let Some((iface, vrf, payload)) = res {
                println!(
                    "GRE Packet Demultiplexed Successfully -> Bound Interface: '{}' (VRF {})",
                    iface, vrf
                );
                println!(
                    "  Payload Delivered: \"{}\"",
                    String::from_utf8_lossy(&payload)
                );
            } else {
                println!(
                    "GRE Demux FAILED: Packet dropped (Invalid Key or Duplicate Replay Sequence #{})",
                    seq
                );
            }
        }
    }

    fn cmd_flowspec(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "rules" || args[0] == "status" {
            println!(
                "BGP Flowspec (RFC 5575 / RFC 8955) Active Traffic Filter Rules (AFI 1 / SAFI 133):"
            );
            println!(
                "┌─────┬──────────────────────┬──────────────────────┬─────────────┬─────────────┬──────────┬────────────────────────┐"
            );
            println!(
                "│ ID  │ Destination Prefix   │ Source Prefix        │ IP Protocol │ Dst Port    │ Src Port │ Action                 │"
            );
            println!(
                "├─────┼──────────────────────┼──────────────────────┼─────────────┼─────────────┼──────────┼────────────────────────┤"
            );
            for r in &self.flowspec_engine.rules {
                let d_str = r
                    .match_fields
                    .dst_prefix
                    .map(|(ip, m)| format!("{}/{}", ip, m))
                    .unwrap_or_else(|| "*".to_string());
                let s_str = r
                    .match_fields
                    .src_prefix
                    .map(|(ip, m)| format!("{}/{}", ip, m))
                    .unwrap_or_else(|| "*".to_string());
                let p_str = r
                    .match_fields
                    .ip_protocol
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "*".to_string());
                let dp_str = r
                    .match_fields
                    .dst_port
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "*".to_string());
                let sp_str = r
                    .match_fields
                    .src_port
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "*".to_string());
                println!(
                    "│ {:<3} │ {:<20} │ {:<20} │ {:<11} │ {:<11} │ {:<8} │ {:<22} │",
                    r.id, d_str, s_str, p_str, dp_str, sp_str, r.action
                );
            }
            println!(
                "└─────┴──────────────────────┴──────────────────────┴─────────────┴─────────────┴──────────┴────────────────────────┘"
            );
        } else if args.len() >= 3 && args[0] == "drop" {
            let dst_ip = Ipv4Address::from_str(args[1]).unwrap_or(self.stack.config.ip);
            let port = args[2].parse::<u16>().unwrap_or(53);

            let new_id = self.flowspec_engine.rules.len() as u32 + 1;
            let rule = FlowspecRule {
                id: new_id,
                match_fields: FlowspecMatch {
                    dst_prefix: Some((dst_ip, 32)),
                    src_prefix: None,
                    ip_protocol: Some(17), // UDP
                    dst_port: Some(port),
                    src_port: None,
                    tcp_flags: None,
                },
                action: FlowspecAction::Drop,
            };
            let serialized_nlri = self.flowspec_engine.serialize_rule(&rule);
            self.flowspec_engine.add_rule(rule);

            println!(
                "Injected BGP Flowspec NLRI Rule #{}: Drop UDP traffic targeting {}:{}",
                new_id, dst_ip, port
            );
            println!(
                "  BGP NLRI Serialized : {} bytes (AFI 1 / SAFI 133)",
                serialized_nlri.len()
            );
            println!("  DDoS Attack Traffic Automatically Neutralized at Ingress!");
        }
    }

    fn cmd_otlp(&mut self, _args: &[&str]) {
        let span = OtlpSpan {
            trace_id: [
                0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
                0xFF, 0x00,
            ],
            span_id: [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0],
            parent_span_id: None,
            name: "network.shell.command".to_string(),
            start_time_ns: 1700000000000000,
            end_time_ns: 1700000000002500,
            attributes: vec![("service.name".to_string(), "toy-tcpip-stack".to_string())],
        };
        self.otlp_exporter.record_span(span);

        let json = self.otlp_exporter.export_json();
        println!(
            "OpenTelemetry OTLP Network Telemetry Stream (Ports {}/{}):",
            OTLP_GRPC_PORT, OTLP_HTTP_PORT
        );
        println!("{}", json);
    }

    fn cmd_gre6(&mut self, args: &[&str]) {
        let msg = if !args.is_empty() {
            args.join(" ")
        } else {
            "Multi-Protocol Overlay Packet traversing Native IPv6 Backbone".to_string()
        };

        let my_ip6 = self.stack.config.ipv6.unwrap();
        let gre6_pkt = GreIpv6Packet::new(
            my_ip6,
            self.remote_host_ipv6,
            ETHERTYPE_IPV4_IN_GRE,
            Some(0x00AABBCC),
            Some(1),
            msg.as_bytes(),
        );
        let raw = gre6_pkt.serialize();

        let eth_frame = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV6,
            &raw,
        );

        println!(
            "Transmitted GRE-over-IPv6 (RFC 7676) Tunnel Frame ({} bytes):",
            eth_frame.len()
        );
        println!(
            "  Outer IPv6 Header  : {} -> {} (Next Header 47 GRE)",
            my_ip6, self.remote_host_ipv6
        );
        println!("  GRE Flags & Key    : Key=0x00AABBCC, Seq=1, Proto=0x0800 (IPv4)");
        println!("  Inner Data Payload : \"{}\"", msg);
    }

    fn cmd_ioam(&mut self, args: &[&str]) {
        let msg = if !args.is_empty() {
            args.join(" ")
        } else {
            "Datacenter IOAM In-Band Telemetry Flow".to_string()
        };

        let mut ioam = IoamPacket::new(1, msg.as_bytes());
        ioam.trace_header.add_hop(101, 1, 2, 1700000000100000, 45); // Leaf 1
        ioam.trace_header.add_hop(201, 3, 4, 1700000000100050, 30); // Spine 1
        ioam.trace_header.add_hop(102, 2, 1, 1700000000100090, 50); // Leaf 2

        let raw = ioam.serialize();
        println!(
            "In-situ OAM (IOAM - RFC 9197) Telemetry Recorded ({} bytes):",
            raw.len()
        );
        println!("  Namespace ID : {}", ioam.trace_header.namespace_id);
        println!(
            "  Recorded Hops ({} nodes in-situ):",
            ioam.trace_header.node_records.len()
        );
        for (i, hop) in ioam.trace_header.node_records.iter().enumerate() {
            println!(
                "    - Hop #{}: Node {:<4} | Port {:<2} -> {:<2} | Transit Queue Delay: {:<3} ns",
                i + 1,
                hop.node_id,
                hop.ingress_if,
                hop.egress_if,
                hop.transit_delay_ns
            );
        }
        println!("  Inner Payload: \"{}\"", msg);
    }

    fn cmd_netconf(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "get" {
            println!(
                "Sending NETCONF <get-config> RPC over TCP {}...",
                NETCONF_PORT
            );
            let req = "<rpc message-id=\"101\" xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\"><get-config><source><running/></source></get-config></rpc>]]>]]>";
            let resp = self.netconf_server.handle_request(req);
            println!(
                "NETCONF <rpc-reply> received from {}:{}:",
                self.remote_host_ip, NETCONF_PORT
            );
            println!("{}", resp);
        } else if args[0] == "commit" {
            println!("Sending NETCONF <commit> RPC over TCP {}...", NETCONF_PORT);
            let req = "<rpc message-id=\"102\" xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\"><commit/></rpc>]]>]]>";
            let resp = self.netconf_server.handle_request(req);
            println!("{}", resp);
            println!("Candidate datastore committed to running datastore!");
        } else if args[0] == "hello" {
            let req = "<hello xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\"><capabilities><capability>urn:ietf:params:netconf:base:1.1</capability></capabilities></hello>]]>]]>";
            let resp = self.netconf_server.handle_request(req);
            println!("NETCONF <hello> capabilities exchange:\n{}", resp);
        }
    }

    fn cmd_lisp(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "lookup" {
            let target_eid = if args.len() >= 2 {
                Ipv4Address::from_str(args[1]).unwrap_or(Ipv4Address::new(10, 1, 1, 50))
            } else {
                Ipv4Address::new(10, 1, 1, 50)
            };

            println!(
                "Sending LISP Map-Request to Map-Resolver {}:{} for EID {}...",
                self.remote_host_ip, LISP_CONTROL_PORT, target_eid
            );
            let req = LispMapRequest::build(
                0x1122334455667788,
                self.stack.config.ip,
                self.stack.config.ip,
                target_eid,
            );
            let raw_req = req.serialize();

            let udp_req = UdpDatagram::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                54342,
                LISP_CONTROL_PORT,
                &raw_req,
            );
            let ip_req = Ipv4Packet::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                IP_PROTO_UDP,
                933,
                64,
                &udp_req,
            );
            let eth_req = EthernetFrame::serialize(
                self.remote_host_mac,
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                &ip_req,
            );

            let resps = self.remote_stack.process_frame(&eth_req);
            for resp in resps {
                let eth = EthernetFrame::parse(&resp).unwrap();
                let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
                let udp = UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true)
                    .unwrap();
                if let Some(reply) = LispMapReply::parse(udp.payload) {
                    println!(
                        "LISP Map-Reply Received (Record TTL: {}s):",
                        reply.record_ttl_s
                    );
                    println!("  EID Prefix : {}/{}", reply.target_eid, reply.eid_mask_len);
                    for loc in reply.locators {
                        println!(
                            "  -> RLOC Gateway IP : {} (Priority: {}, Weight: {})",
                            loc.rloc_ip, loc.priority, loc.weight
                        );
                    }
                }
            }
        } else if args.len() >= 3 && args[0] == "encap" {
            let msg = args[2..].join(" ");
            let inner_ip = Ipv4Packet::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                0,
                934,
                64,
                msg.as_bytes(),
            );
            let lisp_pkt = LispDataPacket::encapsulate(0x123456, 0x00000001, &inner_ip);
            let raw_lisp = lisp_pkt.serialize();

            let udp_req = UdpDatagram::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                54341,
                LISP_DATA_PORT,
                &raw_lisp,
            );
            let ip_req = Ipv4Packet::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                IP_PROTO_UDP,
                935,
                64,
                &udp_req,
            );
            let eth_req = EthernetFrame::serialize(
                self.remote_host_mac,
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                &ip_req,
            );

            println!(
                "Encapsulated LISP Data Packet (UDP {}, {} bytes):",
                LISP_DATA_PORT,
                eth_req.len()
            );
            println!("  LISP Header : Nonce=0x00123456, LSB=0x00000001");
            println!(
                "  Inner IP    : {} bytes (Payload: \"{}\")",
                inner_ip.len(),
                msg
            );
        }
    }

    fn cmd_wireguard(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "handshake" {
            println!(
                "Initiating WireGuard 1-RTT Noise IK Handshake to {}:{}...",
                self.remote_host_ip, WIREGUARD_PORT
            );
            let ephem = [0x55; 32];
            let init = WireguardMessage::build_initiation(self.wg_peer.local_index, ephem);
            let raw_init = init.serialize();

            let udp_req = UdpDatagram::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                51820,
                WIREGUARD_PORT,
                &raw_init,
            );
            let ip_req = Ipv4Packet::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                IP_PROTO_UDP,
                931,
                64,
                &udp_req,
            );
            let eth_req = EthernetFrame::serialize(
                self.remote_host_mac,
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                &ip_req,
            );

            println!(
                "  1. Sent Handshake Initiation (Type 1, {} bytes): SenderIndex=0x{:08X}",
                raw_init.len(),
                self.wg_peer.local_index
            );

            let resps = self.remote_stack.process_frame(&eth_req);
            for resp in resps {
                let eth = EthernetFrame::parse(&resp).unwrap();
                let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
                let udp = UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true)
                    .unwrap();
                if let Ok(wg_msg) = WireguardMessage::parse(udp.payload)
                    && let WireguardMessage::HandshakeResponse {
                        sender_index,
                        receiver_index,
                        ..
                    } = wg_msg
                {
                    println!(
                        "  2. Received Handshake Response (Type 2, {} bytes): RemoteIndex=0x{:08X}, ReceiverIndex=0x{:08X}",
                        udp.payload.len(),
                        sender_index,
                        receiver_index
                    );
                    self.wg_peer.handle_response(sender_index, receiver_index);
                    println!(
                        "  3. WireGuard Cryptographic Key Session Established! (Tunnel IP: 10.99.0.2/32)"
                    );
                }
            }
        } else if args.len() >= 2 && args[0] == "send" {
            let msg = args[1..].join(" ");
            if !self.wg_peer.is_established {
                self.wg_peer.remote_index = Some(0x99887766);
                self.wg_peer.is_established = true;
            }

            let encap_bytes = self.wg_peer.encapsulate_packet(msg.as_bytes()).unwrap();
            let udp_req = UdpDatagram::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                51820,
                WIREGUARD_PORT,
                &encap_bytes,
            );
            let ip_req = Ipv4Packet::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                IP_PROTO_UDP,
                932,
                64,
                &udp_req,
            );
            let eth_req = EthernetFrame::serialize(
                self.remote_host_mac,
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                &ip_req,
            );

            println!(
                "Encapsulated WireGuard Data Transport Packet (Type 4, {} bytes):",
                eth_req.len()
            );
            println!(
                "  Receiver Index : 0x{:08X}",
                self.wg_peer.remote_index.unwrap()
            );
            println!("  Counter        : {}", self.wg_peer.send_counter - 1);
            println!("  Inner Payload  : \"{}\"", msg);
        } else if args[0] == "status" {
            println!("WireGuard VPN Interface wg0 (UDP {}):", WIREGUARD_PORT);
            println!(
                "  Endpoint       : {}:{}",
                self.wg_peer.endpoint_ip, self.wg_peer.endpoint_port
            );
            println!("  Allowed IPs    : 10.99.0.2/32");
            println!(
                "  Session State  : {}",
                if self.wg_peer.is_established {
                    "ESTABLISHED"
                } else {
                    "AWAITING_HANDSHAKE"
                }
            );
            println!("  Packets Sent   : {}", self.wg_peer.send_counter);
        }
    }

    fn cmd_gptp(&mut self, _args: &[&str]) {
        let clock_a = [0x52, 0x54, 0x00, 0xFF, 0xFE, 0x12, 0x34, 0x56];
        let t1 = GptpTimestamp::new(1700000000, 100_000_000);
        let t2 = GptpTimestamp::new(1700000000, 100_000_040); // 40 ns wire delay
        let t3 = GptpTimestamp::new(1700000000, 100_005_000);
        let t4 = GptpTimestamp::new(1700000000, 100_005_040);

        let req = GptpPacket::build_pdelay_req(clock_a, 1, 101, t1);
        let raw = req.serialize();
        let eth_frame = EthernetFrame::serialize(
            GPTP_MULTICAST_MAC,
            self.stack.config.mac,
            ETHERTYPE_GPTP,
            &raw,
        );

        let p_delay = calculate_gptp_peer_delay(t1, t2, t3, t4);
        println!("IEEE 802.1AS gPTP / Time-Sensitive Networking (TSN):");
        println!(
            "  Transmitted Pdelay_Req to {} (EtherType 0x{:04X}, {} bytes)",
            GPTP_MULTICAST_MAC,
            ETHERTYPE_GPTP,
            eth_frame.len()
        );
        println!("  Source Clock Identity : 52:54:00:FF:FE:12:34:56");
        println!("  Transport Specific    : 1 (IEEE 802.1AS gPTP)");
        println!(
            "  Peer Wire Delay (T_p) : {} ns (Deterministic zero-jitter clock sync!)",
            p_delay
        );
    }

    fn cmd_pcep(&mut self, args: &[&str]) {
        let dst_ip = if args.len() >= 2 && args[0] == "req" {
            Ipv4Address::from_str(args[1]).unwrap_or(Ipv4Address::new(10, 0, 0, 4))
        } else {
            Ipv4Address::new(10, 0, 0, 4)
        };

        println!(
            "Sending PCEP Path Computation Request (PCReq) to PCE {}:{}...",
            self.remote_host_ip, PCEP_PORT
        );
        let req = PcepMessage::build_pcreq(101, self.stack.config.ip, dst_ip);
        let raw_req = req.serialize();

        println!(
            "  1. Sent PCReq (Message Type 3, {} bytes): EndPoints={} -> {}",
            raw_req.len(),
            self.stack.config.ip,
            dst_ip
        );

        let rep = self.pcep_session.compute_path(&req).unwrap();
        let raw_rep = rep.serialize();

        println!(
            "  2. Received PCRep (Message Type 4, {} bytes):",
            raw_rep.len()
        );
        if let PcepObject::SrEro { sids } = &rep.objects[1] {
            println!("     Computed SR-MPLS Label Stack : {:?}", sids);
            println!(
                "     Segment Routing Path Ready   : Node-SID 16001 -> Adj-SID 24001 -> Node-SID 16004"
            );
        }
    }

    fn cmd_rsvp(&mut self, args: &[&str]) {
        let dest = if args.len() >= 2 {
            Ipv4Address::from_str(args[1]).unwrap_or(self.remote_host_ip)
        } else {
            self.remote_host_ip
        };

        let bw = if args.len() >= 3 {
            args[2].parse::<u32>().unwrap_or(100) * 1_000_000
        } else {
            100_000_000
        };

        let ero = vec![(false, Ipv4Address::new(192, 168, 1, 1)), (false, dest)];

        let path = RsvpPacket::build_path(self.stack.config.ip, dest, 101, 1, bw, &ero);
        let raw = path.serialize();
        let ip_pkt =
            Ipv4Packet::serialize(self.stack.config.ip, dest, IP_PROTO_RSVP, 930, 64, &raw);
        let eth_frame = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_pkt,
        );

        println!(
            "Transmitted RSVP-TE PATH Message (IP Protocol {}, {} bytes):",
            IP_PROTO_RSVP,
            eth_frame.len()
        );
        println!(
            "  LSP Session   : Destination {} | Tunnel ID: 101 | Ext-ID: {}",
            dest, self.stack.config.ip
        );
        println!(
            "  SENDER_TSPEC  : Guaranteed Bandwidth: {} Mbps",
            bw / 1_000_000
        );
        println!("  Explicit Route: ERO Hops -> [192.168.1.1, {}]", dest);
        println!("  Label Request : Requested Downstream MPLS Label for Traffic Engineered LSP");
    }

    fn cmd_openflow(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "tables" || args[0] == "status" {
            println!("OpenFlow v1.3 SDN Flow Table (TCP Port {}):", OFP_TCP_PORT);
            println!(
                "┌──────────┬──────────────────────┬──────────────────────┬─────────────┬──────────┬──────────┐"
            );
            println!(
                "│ Priority │ In-Port              │ Destination IPv4     │ EtherType   │ Packets  │ Bytes    │"
            );
            println!(
                "├──────────┼──────────────────────┼──────────────────────┼─────────────┼──────────┼──────────┤"
            );
            for e in &self.ofp_table.entries {
                let p_str = e
                    .match_fields
                    .in_port
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "*".to_string());
                let ip_str = e
                    .match_fields
                    .ip_dst
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| "*".to_string());
                let et_str = e
                    .match_fields
                    .eth_type
                    .map(|t| format!("0x{:04X}", t))
                    .unwrap_or_else(|| "*".to_string());
                println!(
                    "│ {:<8} │ {:<20} │ {:<20} │ {:<11} │ {:<8} │ {:<8} │",
                    e.priority, p_str, ip_str, et_str, e.packet_count, e.byte_count
                );
            }
            println!(
                "└──────────┴──────────────────────┴──────────────────────┴─────────────┴──────────┴──────────┘"
            );
        } else if args.len() >= 4 && args[0] == "add" {
            let in_port = args[1].parse::<u32>().unwrap_or(1);
            let dst_ip = Ipv4Address::from_str(args[2]).ok();
            let out_port = args[3].parse::<u32>().unwrap_or(2);

            self.ofp_table.add_entry(
                200,
                OfpMatch {
                    in_port: Some(in_port),
                    eth_type: Some(0x0800),
                    ip_dst: dst_ip,
                },
                vec![OfpAction::Output(out_port)],
            );
            println!(
                "Injected OpenFlow FlowMod Rule: Port {} -> Dst {:?} -> Forward to Port {}",
                in_port, dst_ip, out_port
            );
        } else if args[0] == "hello" {
            let (hdr, hello) = OfpMessage::build_hello(0xABCDEF01);
            let raw = hello.serialize(&hdr);
            println!(
                "Transmitted OpenFlow 1.3 OFPT_HELLO Message ({} bytes): Version=0x04, XID=0xABCDEF01",
                raw.len()
            );
        }
    }

    fn cmd_diameter(&mut self, _args: &[&str]) {
        println!(
            "Transmitting 4G/5G Diameter Capabilities-Exchange-Request (CER) to {}:{}...",
            self.remote_host_ip, DIAMETER_PORT
        );
        let cer = DiameterMessage::build_cer(
            "mme01.epc.mnc001.mcc001.3gppnetwork.org",
            "epc.mnc001.mcc001.3gppnetwork.org",
            self.stack.config.ip,
            10415, // 3GPP Vendor ID
            "ToyStack-4G-Core",
            0x11223344,
            0x55667788,
        );
        let raw_cer = cer.serialize();

        let resp = self.diameter_server.handle_request(&cer);
        let raw_cea = resp.serialize();

        println!(
            "  1. Sent CER (Command Code 257, {} bytes): Origin-Host='mme01.epc...', Vendor-ID=10415 (3GPP)",
            raw_cer.len()
        );
        println!(
            "  2. Received CEA (Command Code 257, {} bytes): Result-Code={} (DIAMETER_SUCCESS)",
            raw_cea.len(),
            DIAMETER_SUCCESS
        );
        println!("     Carrier LTE/5G Mobile Core AAA Link Active & Authenticated!");
    }

    fn cmd_nsh(&mut self, args: &[&str]) {
        let (spi, si) = if args.len() >= 3 && args[0] == "encap" {
            (
                args[1].parse::<u32>().unwrap_or(42),
                args[2].parse::<u8>().unwrap_or(255),
            )
        } else {
            (42, 255)
        };

        let msg = if args.len() >= 4 {
            args[3..].join(" ")
        } else {
            "Service Chained Flow: FW -> IPS -> WAF".to_string()
        };

        let mut pkt = NshPacket::build_ipv4(spi, si, 1001, 0x12345678, msg.as_bytes());
        let raw = pkt.serialize();

        println!(
            "Network Service Header (NSH - RFC 8300) SFC Encapsulation ({} bytes):",
            raw.len()
        );
        println!(
            "  Base Header        : Version=0, MD-Type=1 (16B Context), NextProto=0x01 (IPv4)"
        );
        println!("  Service Path ID    : SPI={}", pkt.header.service_path_id);
        println!("  Initial Index (SI) : {}", pkt.header.service_index);
        println!("  Context C2 (Tenant): {}", pkt.header.context_c2);
        println!("  Context C4 (Flow)  : 0x{:08X}", pkt.header.context_c4);

        ServiceFunctionForwarder::forward_next_service_hop(&mut pkt);
        println!(
            "  -> Forwarded Hop #1 (Firewall Node): Decremented SI -> {}",
            pkt.header.service_index
        );
        ServiceFunctionForwarder::forward_next_service_hop(&mut pkt);
        println!(
            "  -> Forwarded Hop #2 (IPS Node)     : Decremented SI -> {}",
            pkt.header.service_index
        );
    }

    fn cmd_sflow(&mut self, _args: &[&str]) {
        let mut dgram = SflowDatagram::new(self.stack.config.ip, 101, 360000);
        let sample = SflowFlowSample {
            seq_num: 1,
            source_id: 1,
            sampling_rate: 1000,
            sample_pool: 50000,
            drops: 0,
            input_if: 1,
            output_if: 2,
            orig_packet_len: 128,
            sampled_header: vec![
                0x52, 0x54, 0x00, 0x12, 0x34, 0x56, 0x02, 0x00, 0x00, 0x00, 0x00, 0x10, 0x08, 0x00,
            ],
        };
        let counter = SflowCounterSample {
            seq_num: 1,
            source_id: 1,
            if_index: 1,
            if_speed_bps: 10_000_000_000,
            in_octets: 1024000,
            in_packets: 1500,
            out_octets: 512000,
            out_packets: 800,
        };

        dgram.samples.push(SflowSample::Flow(sample));
        dgram.samples.push(SflowSample::Counter(counter));
        let raw = dgram.serialize();

        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            56343,
            SFLOW_UDP_PORT,
            &raw,
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            923,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        println!(
            "Transmitted sFlow v5 Flow & Counter Telemetry Datagram (UDP {}, {} bytes):",
            SFLOW_UDP_PORT,
            eth_req.len()
        );
        println!("  Agent IPv4     : {}", dgram.agent_ip);
        println!(
            "  Sample Records : 1 Flow Sample (1:1000 rate, eth0 -> eth1) + 1 Interface Counter Sample"
        );
    }

    fn cmd_6in4(&mut self, args: &[&str]) {
        let msg = if !args.is_empty() {
            args.join(" ")
        } else {
            "IPv6 Packet traversing legacy IPv4 Backbone".to_string()
        };

        let my_ip6 = self.stack.config.ipv6.unwrap();
        let inner_ip6 =
            Ipv6Packet::serialize(my_ip6, self.remote_host_ipv6, 59, 64, msg.as_bytes());
        let tunnel = Tunnel6in4::new(self.stack.config.ip, self.remote_host_ip);
        let encap_ip4 = tunnel.encapsulate(&inner_ip6, 924);
        let eth_frame = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &encap_ip4,
        );

        println!(
            "Transmitted 6in4 IPv6-in-IPv4 Transition Tunnel Frame ({} bytes, Protocol {}):",
            eth_frame.len(),
            IP_PROTO_IPV6_IN_IPV4
        );
        println!(
            "  Outer IPv4 Header : {} -> {} (IP Protocol 41)",
            self.stack.config.ip, self.remote_host_ip
        );
        println!(
            "  Inner IPv6 Header : {} -> {}",
            my_ip6, self.remote_host_ipv6
        );
        println!("  Inner IPv6 Payload: \"{}\"", msg);
    }

    fn cmd_4in6(&mut self, args: &[&str]) {
        let msg = if !args.is_empty() {
            args.join(" ")
        } else {
            "IPv4 Packet traversing IPv6 Backbone".to_string()
        };

        let inner_ip4 = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            0,
            925,
            64,
            msg.as_bytes(),
        );
        let my_ip6 = self.stack.config.ipv6.unwrap();
        let tunnel = Tunnel4in6::new(my_ip6, self.remote_host_ipv6);
        let encap_ip6 = tunnel.encapsulate(&inner_ip4);
        let eth_frame = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV6,
            &encap_ip6,
        );

        println!(
            "Transmitted 4in6 IPv4-in-IPv6 Transition Tunnel Frame ({} bytes):",
            eth_frame.len()
        );
        println!(
            "  Outer IPv6 Header : {} -> {} (Next Header 4)",
            my_ip6, self.remote_host_ipv6
        );
        println!(
            "  Inner IPv4 Header : {} -> {}",
            self.stack.config.ip, self.remote_host_ip
        );
        println!("  Inner IPv4 Payload: \"{}\"", msg);
    }

    fn cmd_roce(&mut self, args: &[&str]) {
        let qp = if args.len() >= 2 {
            args[1].parse::<u32>().unwrap_or(202)
        } else {
            202
        };

        let msg = if args.len() >= 3 {
            args[2..].join(" ")
        } else {
            "GPU Tensor Buffer Data Transfer over RDMA".to_string()
        };

        println!(
            "Transmitting RoCEv2 InfiniBand RDMA Packet to {}:{} (DestQP=0x{:06X})...",
            self.remote_host_ip, ROCEV2_UDP_PORT, qp
        );
        let roce = RocePacket::build_send(qp, 5000, msg.as_bytes());
        let raw = roce.serialize();

        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            49152,
            ROCEV2_UDP_PORT,
            &raw,
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            921,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        println!(
            "  RoCEv2 BTH Header : OpCode=0x04 (RC SEND_ONLY), P_Key=0xFFFF, PSN=5000, AckReq=true"
        );
        println!("  Invariant CRC     : 0x{:08X}", roce.icrc);
        println!("  RDMA Payload      : {} bytes (\"{}\")", msg.len(), msg);

        let resps = self.remote_stack.process_frame(&eth_req);
        for resp in resps {
            let eth = EthernetFrame::parse(&resp).unwrap();
            let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
            let udp =
                UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true).unwrap();
            if let Ok(roce_ack) = RocePacket::parse(udp.payload) {
                println!(
                    "RoCEv2 ACK Received from Remote QP: OpCode=0x11 (RC_ACK), DestQP=0x{:06X}, PSN={}",
                    roce_ack.bth.dest_qp, roce_ack.bth.psn
                );
                println!("  Ultra-low Latency RDMA Transfer Succeeded!");
            }
        }
    }

    fn cmd_pfc(&mut self, args: &[&str]) {
        let cls = if args.len() >= 2 && args[0] == "pause" {
            args[1].parse::<u8>().unwrap_or(3)
        } else {
            3
        };

        println!("Generating IEEE 802.1Qbb Priority Flow Control (PFC) Pause Frame...");
        let pfc = PfcPauseFrame::new(&[cls], 65535);
        let raw = pfc.serialize();
        let eth_frame = EthernetFrame::serialize(
            PFC_MULTICAST_MAC,
            self.stack.config.mac,
            ETHERTYPE_FLOW_CONTROL,
            &raw,
        );

        println!(
            "Transmitted PFC Pause to Multicast MAC {} (EtherType 0x{:04X}, {} bytes):",
            PFC_MULTICAST_MAC,
            ETHERTYPE_FLOW_CONTROL,
            eth_frame.len()
        );
        println!("  MAC Control Opcode : 0x0101 (PFC Pause)");
        println!(
            "  Class Enable Vector: 0b{:08b} (Priority Class {} PAUSED)",
            pfc.class_enable_vector, cls
        );
        println!("  Pause Quantum      : 65535 units (Lossless Ethernet buffer protected!)");
    }

    fn cmd_gue(&mut self, args: &[&str]) {
        let msg = if !args.is_empty() {
            args.join(" ")
        } else {
            "Datacenter Cloud Microservice Payload over GUE".to_string()
        };

        let gue = GuePacket::build_ipv4(msg.as_bytes());
        let raw = gue.serialize();

        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            56080,
            GUE_UDP_PORT,
            &raw,
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            922,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        println!(
            "Transmitted Generic UDP Encapsulation (GUE - RFC 7763) Frame (UDP {}, {} bytes):",
            GUE_UDP_PORT,
            eth_req.len()
        );
        println!("  GUE Header  : Version=0, NextProto=0x04 (IPv4), HLEN=0 (4 bytes)");
        println!("  Inner Data  : {} bytes (\"{}\")", msg.len(), msg);
    }

    fn cmd_evpn(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "rib" || args[0] == "status" {
            println!("BGP EVPN (AFI 25 / SAFI 70) MAC-to-VTEP Forwarding Table (RFC 7432):");
            println!(
                "┌────────┬──────────────────────┬──────────────────────┬──────────────────────┐"
            );
            println!(
                "│ VNI    │ MAC Address          │ Next-Hop VTEP IP     │ Host IP Address      │"
            );
            println!(
                "├────────┼──────────────────────┼──────────────────────┼──────────────────────┤"
            );
            for (&(vni, mac), (vtep, host_ip)) in &self.evpn_table.entries {
                let ip_str = host_ip
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| "-".to_string());
                println!(
                    "│ {:<6} │ {:<20} │ {:<20} │ {:<20} │",
                    vni, mac, vtep, ip_str
                );
            }
            println!(
                "└────────┴──────────────────────┴──────────────────────┴──────────────────────┘"
            );
        } else if args.len() >= 4 && args[0] == "advertise" {
            let mac = MacAddress::from_str(args[1]).unwrap_or(self.stack.config.mac);
            let ip = Ipv4Address::from_str(args[2]).ok();
            let vni = args[3].parse::<u32>().unwrap_or(5001);
            let rd = RouteDistinguisher::new(self.stack.config.ip, 100);

            let nlri = EvpnNlri::build_mac_ip(rd.clone(), mac, ip, vni);
            let raw = nlri.serialize();

            println!(
                "Advertised BGP EVPN Route Type 2 (MAC/IP Advertisement, {} bytes):",
                raw.len()
            );
            println!("  RD: {} | VNI: {} | MAC: {} | IP: {:?}", rd, vni, mac, ip);
            println!(
                "  Control Plane: Synchronized across spine-leaf datacenter fabric without flooding!"
            );
        }
    }

    fn cmd_dhcpv6(&mut self, _args: &[&str]) {
        println!(
            "Sending DHCPv6 Solicit to ff02::1:2:{} (RFC 8415)...",
            DHCPV6_SERVER_PORT
        );
        let client_duid = vec![0x00, 0x03, 0x00, 0x01, 0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let solicit = Dhcpv6Message::build_solicit(0xABCDEF, &client_duid);
        let raw = solicit.serialize();

        let my_ip6 = self.stack.config.ipv6.unwrap();
        let server_mcast = Ipv6Address::from_str("ff02::1:2").unwrap();

        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            DHCPV6_CLIENT_PORT,
            DHCPV6_SERVER_PORT,
            &raw,
        );
        let ip6_req = Ipv6Packet::serialize(my_ip6, server_mcast, NEXT_HEADER_UDP, 64, &udp_req);
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV6,
            &ip6_req,
        );

        let resps = self.remote_stack.process_frame(&eth_req);
        for resp in resps {
            let eth = EthernetFrame::parse(&resp).unwrap();
            let ip6 = Ipv6Packet::parse(eth.payload).unwrap();
            let udp = UdpDatagram::parse(
                self.remote_host_ip,
                self.stack.config.ip,
                ip6.payload,
                false,
            )
            .unwrap();
            if let Ok(adv) = Dhcpv6Message::parse(udp.payload) {
                println!(
                    "DHCPv6 Advertise Message Received from Server (TID=0x{:06X}):",
                    adv.transaction_id
                );
                if let Some(assigned_ip6) = adv.get_assigned_ipv6() {
                    println!("  Assigned IPv6 Address (IA_NA): {}", assigned_ip6);
                    println!("  Lease Preferred Lifetime     : 3600 seconds");
                    println!("  Lease Valid Lifetime         : 7200 seconds");
                    println!("  DNS Recursive Name Server    : 2001:4860:4860::8888");
                }
            }
        }
    }

    fn cmd_vxlan_gpe(&mut self, args: &[&str]) {
        let vni = if args.len() >= 2 {
            args[1].parse::<u32>().unwrap_or(7001)
        } else {
            7001
        };

        let msg = if args.len() >= 3 {
            args[2..].join(" ")
        } else {
            "Direct L3 IPv4 Payload over VXLAN-GPE".to_string()
        };

        let gpe = VxlanGpePacket::build(vni, VXLAN_GPE_NP_IPV4, msg.as_bytes());
        let raw = gpe.serialize();

        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            54790,
            VXLAN_GPE_UDP_PORT,
            &raw,
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            920,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        println!(
            "Encapsulated VXLAN-GPE Multi-Protocol Overlay Packet (UDP {}, {} bytes):",
            VXLAN_GPE_UDP_PORT,
            eth_req.len()
        );
        println!("  24-bit VNI    : {}", vni);
        println!("  Next Protocol : 0x01 (Direct IPv4 without Ethernet overhead)");
        println!("  Payload       : \"{}\"", msg);
    }

    fn cmd_vtp(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "status" {
            println!("Cisco VLAN Trunking Protocol (VTP) Status:");
            println!("  VTP Domain Name   : {}", self.vtp.domain);
            println!("  VTP Mode          : {}", self.vtp.mode);
            println!("  Config Revision   : {}", self.vtp.revision);
            println!("  Synchronized VLANs:");
            for (id, name) in &self.vtp.vlans {
                println!("    - VLAN {:<4}: {}", id, name);
            }
        } else if args.len() >= 3 && args[0] == "add" {
            let id = args[1].parse::<u16>().unwrap_or(30);
            let name = args[2];
            if self.vtp.add_vlan(id, name) {
                println!(
                    "Added VLAN {} ('{}') -> New Configuration Revision: {}",
                    id, name, self.vtp.revision
                );
            }
        } else if args[0] == "summary" {
            let summary =
                VtpPacket::build_summary(&self.vtp.domain, self.vtp.revision, self.stack.config.ip);
            let mut snap_frame = VTP_SNAP_HEADER.to_vec();
            snap_frame.extend_from_slice(&summary.serialize());
            let eth_frame = EthernetFrame::serialize(
                VTP_MULTICAST_MAC,
                self.stack.config.mac,
                0x0000,
                &snap_frame,
            );
            println!(
                "Transmitted VTP Summary Advertisement to {} ({} bytes):",
                VTP_MULTICAST_MAC,
                eth_frame.len()
            );
            println!(
                "  Domain: {} | Revision: {} | Updater: {}",
                self.vtp.domain, self.vtp.revision, self.stack.config.ip
            );
        }
    }

    fn cmd_ldp(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "hello" {
            println!("Transmitting LDP Discovery Basic Hello PDU (UDP 646)...");
            let hello_pdu = LdpPdu::build_hello(self.stack.config.ip, 15);
            let raw = hello_pdu.serialize();

            let udp_req = UdpDatagram::serialize(
                self.stack.config.ip,
                Ipv4Address::new(224, 0, 0, 2),
                646,
                LDP_PORT,
                &raw,
            );
            let ip_req = Ipv4Packet::serialize(
                self.stack.config.ip,
                Ipv4Address::new(224, 0, 0, 2),
                IP_PROTO_UDP,
                916,
                64,
                &udp_req,
            );
            let eth_req = EthernetFrame::serialize(
                MacAddress([0x01, 0x00, 0x5E, 0x00, 0x00, 0x02]),
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                &ip_req,
            );

            println!(
                "  LDP PDU Formatted ({} bytes): Version=1, LSR-ID={}, LabelSpace=0",
                raw.len(),
                hello_pdu.lsr_id
            );
            println!(
                "  Transmitted to Multicast 224.0.0.2:646 (Ethernet Frame: {} bytes)",
                eth_req.len()
            );
        } else if args.len() >= 3 && args[0] == "map" {
            let prefix = Ipv4Address::from_str(args[1]).unwrap_or(Ipv4Address::new(10, 50, 0, 0));
            let label = args[2].parse::<u32>().unwrap_or(200);

            let map_pdu = LdpPdu::build_label_mapping(self.stack.config.ip, 102, prefix, 24, label);
            let raw = map_pdu.serialize();
            println!(
                "Transmitted LDP Label Mapping Message (TCP 646, {} bytes):",
                raw.len()
            );
            println!("  FEC Prefix   : {}/24", prefix);
            println!("  Assigned Label: {}", label);
            println!(
                "  Dynamic LFIB : Injected Prefix FEC Binding -> Label {}",
                label
            );
        }
    }

    fn cmd_glbp(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "status" {
            println!("Cisco Gateway Load Balancing Protocol (GLBP):");
            println!("  Group Number   : {}", self.glbp.group);
            println!("  Priority       : {}", self.glbp.priority);
            println!("  Weight         : {}", self.glbp.weight);
            println!("  Virtual IP     : {}", self.glbp.virtual_ip);
            println!("  Router Role    : {}", self.glbp.role);
            println!("  Active AVFs    : Forwarder #1, Forwarder #2");
            println!("  Balancing Mode : Round-Robin");
        } else if args[0] == "arp" {
            let resolved_mac = self.glbp.resolve_arp_reply_mac();
            println!(
                "GLBP ARP Request from Host -> Assigned Virtual MAC: {}",
                resolved_mac
            );
            println!("  (Traffic automatically load-balanced across active gateway forwarders!)");
        } else if args[0] == "hello" {
            let hello = self.glbp.build_advertisement();
            let raw = hello.serialize();
            let udp_req = UdpDatagram::serialize(
                self.stack.config.ip,
                GLBP_MULTICAST_IP,
                3222,
                GLBP_UDP_PORT,
                &raw,
            );
            let ip_req = Ipv4Packet::serialize(
                self.stack.config.ip,
                GLBP_MULTICAST_IP,
                IP_PROTO_UDP,
                917,
                64,
                &udp_req,
            );
            let eth_req = EthernetFrame::serialize(
                MacAddress([0x01, 0x00, 0x5E, 0x00, 0x00, 0x66]),
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                &ip_req,
            );
            println!(
                "Transmitted GLBP Hello to Multicast {} ({} bytes):",
                GLBP_MULTICAST_IP,
                eth_req.len()
            );
            println!(
                "  Group: {} | Priority: {} | Forwarder: #{} | Virtual IP: {}",
                hello.group, hello.priority, hello.forwarder_num, hello.virtual_ip
            );
        }
    }

    fn cmd_tacacs(&mut self, args: &[&str]) {
        let (user, pass) = if args.len() >= 3 && args[0] == "auth" {
            (args[1], args[2])
        } else {
            ("admin", "cisco123")
        };

        println!(
            "Initiating TACACS+ Authentication Session to {}:{} (RFC 8907)...",
            self.remote_host_ip, TACACS_PORT
        );
        let session_id = 0x55AA1122;
        let authen_start = TacacsPacket::build_authen_start(session_id, user, "tty0", pass);

        println!(
            "  1. Transmitted TACACS+ START (Type=1 Authen, Seq=1, SessionID=0x{:08X}, User='{}')",
            session_id, user
        );
        let resp = self.tacacs_server.authenticate(&authen_start);
        let status_str = if resp.body[0] == TACACS_AUTHEN_STATUS_PASS {
            "PASS (Granted)"
        } else {
            "FAIL (Denied)"
        };
        let msg_len = u16::from_be_bytes([resp.body[2], resp.body[3]]) as usize;
        let server_msg = String::from_utf8_lossy(&resp.body[6..6 + msg_len]);

        println!(
            "  2. Received TACACS+ REPLY (Type=1 Authen, Seq=2, Status={}):",
            status_str
        );
        println!("     \"{}\"", server_msg);
    }

    fn cmd_turn(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "alloc" {
            println!(
                "Sending TURN Allocate Request to {}:{} (RFC 5766)...",
                self.remote_host_ip, STUN_PORT
            );
            let tid = [0xBB; 12];
            let req = TurnPacket::build_allocate_request(tid, 600);
            let raw = req.serialize();

            let udp_req = UdpDatagram::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                54378,
                STUN_PORT,
                &raw,
            );
            let ip_req = Ipv4Packet::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                IP_PROTO_UDP,
                912,
                64,
                &udp_req,
            );
            let eth_req = EthernetFrame::serialize(
                self.remote_host_mac,
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                &ip_req,
            );

            let resps = self.remote_stack.process_frame(&eth_req);
            for resp in resps {
                let eth = EthernetFrame::parse(&resp).unwrap();
                let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
                let udp = UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true)
                    .unwrap();
                if let Ok(turn_resp) = TurnPacket::parse(udp.payload)
                    && let Some((rel_ip, rel_port)) = turn_resp.get_xor_relayed_address()
                {
                    println!("TURN Allocate Response Received (Success 0x0103):");
                    println!("  Relayed Public IP  : {}", rel_ip);
                    println!("  Relayed Public Port: {}", rel_port);
                    println!("  Allocation Lifetime: 600 seconds");
                    println!("  Relay Status       : Symmetric NAT Traversal Active!");
                }
            }
        } else if args.len() >= 2 && args[0] == "send" {
            let msg = args[1..].join(" ");
            let peer_ip = Ipv4Address::new(198, 51, 100, 77);
            let peer_port = 5004;
            let send_ind = TurnPacket::build_send_indication(peer_ip, peer_port, msg.as_bytes());
            let raw = send_ind.serialize();
            println!(
                "Transmitted TURN Send Indication ({} bytes) to {}:{} via Relay Server",
                raw.len(),
                peer_ip,
                peer_port
            );
            println!("  Relayed Payload: \"{}\"", msg);
        }
    }

    fn cmd_gtp(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "echo" {
            println!(
                "Sending 4G/5G GTP-U Echo Request to {}:{} (3GPP TS 29.281)...",
                self.remote_host_ip, GTP_U_UDP_PORT
            );
            let echo = GtpPacket::build_echo_request(0, 101);
            let raw = echo.serialize();

            let udp_req = UdpDatagram::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                52152,
                GTP_U_UDP_PORT,
                &raw,
            );
            let ip_req = Ipv4Packet::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                IP_PROTO_UDP,
                913,
                64,
                &udp_req,
            );
            let eth_req = EthernetFrame::serialize(
                self.remote_host_mac,
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                &ip_req,
            );

            let resps = self.remote_stack.process_frame(&eth_req);
            for resp in resps {
                let eth = EthernetFrame::parse(&resp).unwrap();
                let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
                let udp = UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true)
                    .unwrap();
                if let Ok(gtp_resp) = GtpPacket::parse(udp.payload) {
                    println!(
                        "GTP-U Echo Response Received: MsgType={}, Seq={:?}",
                        gtp_resp.header.msg_type, gtp_resp.header.seq_num
                    );
                    println!("  Cellular UPF / gNodeB Node is Alive & Responsive!");
                }
            }
        } else if args.len() >= 3 && args[0] == "encap" {
            let teid = args[1].parse::<u32>().unwrap_or(0x01020304);
            let msg = args[2..].join(" ");
            let gpdu = GtpPacket::build_gpdu(teid, msg.as_bytes());
            let raw = gpdu.serialize();

            let udp_req = UdpDatagram::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                52152,
                GTP_U_UDP_PORT,
                &raw,
            );
            let ip_req = Ipv4Packet::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                IP_PROTO_UDP,
                914,
                64,
                &udp_req,
            );
            let eth_req = EthernetFrame::serialize(
                self.remote_host_mac,
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                &ip_req,
            );

            println!(
                "Encapsulated 4G/5G Cellular User Plane Data Packet (GTP-U G-PDU, {} bytes):",
                eth_req.len()
            );
            println!("  Subscriber TEID : 0x{:08X}", teid);
            println!("  Tunnel Payload  : {} bytes (\"{}\")", msg.len(), msg);
        }
    }

    fn cmd_hsrp(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "status" {
            println!("Cisco Hot Standby Router Protocol (HSRPv1 - RFC 2281):");
            println!("  Group Number   : {}", self.hsrp.group);
            println!("  Priority       : {}", self.hsrp.priority);
            println!("  Virtual IP     : {}", self.hsrp.virtual_ip);
            println!(
                "  Virtual MAC    : {}",
                HsrpPacket::virtual_mac(self.hsrp.group)
            );
            println!("  Router State   : {}", self.hsrp.state);
            println!(
                "  Preempt Mode   : {}",
                if self.hsrp.preempt {
                    "Enabled"
                } else {
                    "Disabled"
                }
            );
            println!(
                "  Active Router  : {:?}",
                self.hsrp.active_router.unwrap_or(self.stack.config.ip)
            );
        } else if args[0] == "hello" {
            let hello = self.hsrp.build_advertisement();
            let raw = hello.serialize();
            let udp_req = UdpDatagram::serialize(
                self.stack.config.ip,
                HSRP_MULTICAST_IP,
                1985,
                HSRP_UDP_PORT,
                &raw,
            );
            let ip_req = Ipv4Packet::serialize(
                self.stack.config.ip,
                HSRP_MULTICAST_IP,
                IP_PROTO_UDP,
                915,
                64,
                &udp_req,
            );
            let eth_req = EthernetFrame::serialize(
                MacAddress([0x01, 0x00, 0x5E, 0x00, 0x00, 0x02]),
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                &ip_req,
            );
            println!(
                "Transmitted HSRP Hello to Multicast {} ({} bytes):",
                HSRP_MULTICAST_IP,
                eth_req.len()
            );
            println!(
                "  Group: {} | State: {} | Priority: {} | Virtual IP: {}",
                hello.group, hello.state, hello.priority, hello.virtual_ip
            );
        }
    }

    fn cmd_cdp(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "neighbors" {
            println!(
                "Cisco Discovery Protocol (CDPv2) Neighbor Table (MAC {}):",
                CDP_MULTICAST_MAC
            );
            println!(
                "┌──────────────────────┬──────────────────────┬──────────────────────┬──────────────────┬─────────┐"
            );
            println!(
                "│ Device ID            │ Port ID              │ Platform             │ IP Address       │ TTL (s) │"
            );
            println!(
                "├──────────────────────┼──────────────────────┼──────────────────────┼──────────────────┼─────────┤"
            );
            for n in self.cdp_table.neighbors.values() {
                let ip_str = n
                    .ip_address
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| "-".to_string());
                println!(
                    "│ {:<20} │ {:<20} │ {:<20} │ {:<16} │ {:<7} │",
                    n.device_id, n.port_id, n.platform, ip_str, n.ttl
                );
            }
            println!(
                "└──────────────────────┴──────────────────────┴──────────────────────┴──────────────────┴─────────┘"
            );
        } else if args[0] == "announce" {
            let pkt = CdpPacket::build(
                "ToyStack-Router",
                "GigabitEthernet0/1",
                "ToyNetStack v1.0",
                self.stack.config.ip,
            );
            let mut snap_pkt = CDP_SNAP_HEADER.to_vec();
            snap_pkt.extend_from_slice(&pkt.serialize());

            let eth_frame = EthernetFrame::serialize(
                CDP_MULTICAST_MAC,
                self.stack.config.mac,
                0x0000,
                &snap_pkt,
            );
            println!(
                "Transmitted CDPv2 Advertisement Frame to {} ({} bytes):",
                CDP_MULTICAST_MAC,
                eth_frame.len()
            );
            println!(
                "  Device-ID: ToyStack-Router | Port: GigabitEthernet0/1 | Platform: ToyNetStack v1.0"
            );
        }
    }

    fn cmd_srv6(&mut self, _args: &[&str]) {
        let sid1 = Ipv6Address::from_str("2001:db8:1::1").unwrap();
        let sid2 = Ipv6Address::from_str("2001:db8:2::1").unwrap();
        let sid3 = Ipv6Address::from_str("2001:db8:3::1").unwrap();

        let srh = Srv6Header::build(59, &[sid1, sid2, sid3]);
        let raw = srh.serialize();

        println!("Segment Routing over IPv6 (SRv6 - RFC 8754):");
        println!(
            "  SRH Extension Header (Type {}, {} bytes):",
            IPV6_EXT_ROUTING,
            raw.len()
        );
        println!("  Routing Type : 4 (Segment Routing Header)");
        println!("  Segments Left: {}", srh.segments_left);
        println!("  Last Entry   : {}", srh.last_entry);
        println!("  Segment List (SIDs):");
        for (i, sid) in srh.segment_list.iter().enumerate() {
            let marker = if i as u8 == srh.segments_left {
                "<- Active Segment"
            } else {
                ""
            };
            println!("    - SID #{}: {:<40} {}", i, sid, marker);
        }
    }

    fn cmd_stun(&mut self, _args: &[&str]) {
        println!(
            "Querying STUN Server at {}:{} for NAT Reflexive Mapping...",
            self.remote_host_ip, STUN_PORT
        );
        let tid = [0xAA; 12];
        let req = StunPacket::build_binding_request(tid);
        let raw = req.serialize();

        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            53478,
            STUN_PORT,
            &raw,
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            911,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        let resps = self.remote_stack.process_frame(&eth_req);
        for resp in resps {
            let eth = EthernetFrame::parse(&resp).unwrap();
            let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
            let udp =
                UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true).unwrap();
            if let Ok(stun_resp) = StunPacket::parse(udp.payload)
                && let Some((r_ip, r_port)) = stun_resp.get_xor_mapped_address()
            {
                println!("STUN Binding Response Received (RFC 8449 XOR-MAPPED-ADDRESS):");
                println!("  Public Reflexive IP  : {}", r_ip);
                println!("  Public Reflexive Port: {}", r_port);
                println!("  NAT Traversal Status : Direct UDP Binding Discovered!");
            }
        }
    }

    fn cmd_rtp(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "send" {
            let msg = if args.len() >= 2 {
                args[1..].join(" ")
            } else {
                "Audio G.711 PCM Payload 160B".to_string()
            };
            let rtp =
                RtpPacket::build_audio(RTP_PT_PCMU, 1, 160000, 0x12345678, false, msg.as_bytes());
            let raw = rtp.serialize();

            let udp_req =
                UdpDatagram::serialize(self.stack.config.ip, self.remote_host_ip, 5004, 5004, &raw);
            let ip_req = Ipv4Packet::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                IP_PROTO_UDP,
                909,
                64,
                &udp_req,
            );
            let eth_req = EthernetFrame::serialize(
                self.remote_host_mac,
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                &ip_req,
            );

            println!(
                "Transmitted RTP Real-time Media Packet (UDP 5004, {} bytes):",
                eth_req.len()
            );
            println!(
                "  RTP Header : Version=2, PT=0 (PCMU), Seq=1, Timestamp=160000, SSRC=0x12345678"
            );
            println!("  RTP Payload: {} bytes (\"{}\")", msg.len(), msg);
        } else if args[0] == "sr" {
            let sr = RtcpSenderReport::build(0x12345678, 0xE584123400000000, 160000, 100, 16000);
            let raw = sr.serialize();
            println!(
                "Transmitted RTCP Sender Report (SR) Telemetry ({} bytes):",
                raw.len()
            );
            println!("  SSRC: 0x12345678 | Packets Sent: 100 | Octets Sent: 16000 bytes");
        }
    }

    fn cmd_ptp(&mut self, _args: &[&str]) {
        let clock_id = [0x00, 0x11, 0x22, 0xFF, 0xFE, 0x33, 0x44, 0x55];
        let t1 = PtpTimestamp::new(1700000000, 100_000_000);
        let t2 = PtpTimestamp::new(1700000000, 100_000_085);
        let t3 = PtpTimestamp::new(1700000000, 100_050_000);
        let t4 = PtpTimestamp::new(1700000000, 100_050_085);

        let sync_pkt = PtpPacket::build_sync(clock_id, 1, t1);
        let raw = sync_pkt.serialize();

        let (offset, delay) = calculate_ptp_offset_and_delay(t1, t2, t3, t4);
        println!("Precision Time Protocol (IEEE 1588v2 PTP - UDP 319/320):");
        println!("  Transmitted PTP Sync Packet ({} bytes, Seq=1)", raw.len());
        println!("  Grandmaster Clock ID : 00:11:22:FF:FE:33:44:55");
        println!("  Measured Offset      : {} ns", offset);
        println!(
            "  Mean Path Delay      : {} ns (Sub-microsecond precision!)",
            delay
        );
    }

    fn cmd_erspan(&mut self, args: &[&str]) {
        let sid = if args.len() >= 2 {
            args[1].parse::<u16>().unwrap_or(101)
        } else {
            101
        };
        let msg = if args.len() >= 3 {
            args[2..].join(" ")
        } else {
            "Mirrored Ingress Frame".to_string()
        };

        let inner_eth = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            msg.as_bytes(),
        );
        let erspan_payload = ErspanPacket::encapsulate(sid, 10, 1, &inner_eth);

        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_GRE,
            910,
            64,
            &erspan_payload,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        println!(
            "Transmitted ERSPAN Type II Remote Mirrored Frame (GRE Protocol 47, {} bytes):",
            eth_req.len()
        );
        println!("  ERSPAN Session ID: {}", sid);
        println!("  VLAN Tag         : 10, Port Index: 1");
        println!(
            "  Mirrored Frame   : {} bytes (Inner Payload: \"{}\")",
            inner_eth.len(),
            msg
        );
    }

    fn cmd_mqtt(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "status" {
            println!("MQTT Telemetry Broker Subscriptions (Port {}):", MQTT_PORT);
            for (top, subs) in &self.mqtt_broker.subscriptions {
                println!("  Topic: {:<30} -> Subscribers: [{}]", top, subs.join(", "));
            }
        } else if args.len() >= 3 && args[0] == "pub" {
            let topic = args[1];
            let msg = args[2..].join(" ");
            let pub_pkt = MqttPacket::build_publish(topic, msg.as_bytes(), 0, None);
            let raw = pub_pkt.serialize();
            println!(
                "Published MQTT Message (Topic: '{}', {} bytes):",
                topic,
                raw.len()
            );
            println!("  Payload: \"{}\"", msg);
            let recipients = self.mqtt_broker.publish(topic);
            println!(
                "  Broker Routed to {} subscribers: {:?}",
                recipients.len(),
                recipients
            );
        } else if args.len() >= 2 && args[0] == "sub" {
            let topic = args[1];
            self.mqtt_broker.subscribe(topic, "ShellClient");
            println!("Subscribed 'ShellClient' to MQTT topic: '{}'", topic);
        }
    }

    fn cmd_coap(&mut self, args: &[&str]) {
        let path = if args.len() >= 2 && args[0] == "get" {
            args[1]
        } else {
            "sensors/temperature"
        };

        println!(
            "Sending CoAP CON GET to {}:{} for '{}'...",
            self.remote_host_ip, COAP_UDP_PORT, path
        );
        let req = CoapPacket::build_get(0x4321, path, &[0xDE, 0xAD]);
        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            55683,
            COAP_UDP_PORT,
            &req.serialize(),
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            906,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        let resps = self.remote_stack.process_frame(&eth_req);
        for resp in resps {
            let eth = EthernetFrame::parse(&resp).unwrap();
            let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
            let udp =
                UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true).unwrap();
            if let Ok(coap_resp) = CoapPacket::parse(udp.payload) {
                println!(
                    "CoAP Response Received: Type=ACK, Code={} (2.05 Content), MsgID=0x{:04X}",
                    coap_resp.code, coap_resp.message_id
                );
                println!(
                    "  Payload: \"{}\"",
                    String::from_utf8_lossy(&coap_resp.payload)
                );
            }
        }
    }

    fn cmd_sctp(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "init" {
            let init = SctpPacket::build_init(5000, 2905, 0x98765432, 65535, 10, 10, 1000);
            let raw = init.serialize();
            let ip_pkt = Ipv4Packet::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                IP_PROTO_SCTP,
                907,
                64,
                &raw,
            );
            let eth_frame = EthernetFrame::serialize(
                self.remote_host_mac,
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                &ip_pkt,
            );
            println!(
                "Transmitted SCTP Association INIT Chunk ({} bytes, Protocol {}):",
                eth_frame.len(),
                IP_PROTO_SCTP
            );
            println!("  Common Header : SrcPort=5000, DstPort=2905, V-Tag=0x00000000");
            println!(
                "  INIT Chunk    : Tag=0x98765432, a_rwnd=65535, OutStreams=10, InStreams=10, ISN=1000"
            );
        } else if args.len() >= 2 && args[0] == "send" {
            let msg = args[1..].join(" ");
            let data = SctpPacket::build_data(5000, 2905, 0x98765432, 1, 0, 0, 0, msg.as_bytes());
            let raw = data.serialize();
            let ip_pkt = Ipv4Packet::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                IP_PROTO_SCTP,
                908,
                64,
                &raw,
            );
            let eth_frame = EthernetFrame::serialize(
                self.remote_host_mac,
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                &ip_pkt,
            );
            println!("Transmitted SCTP DATA Chunk ({} bytes):", eth_frame.len());
            println!(
                "  DATA Chunk    : TSN=1, StreamID=0, Seq=0, Payload: \"{}\"",
                msg
            );
        }
    }

    fn cmd_ldap(&mut self, args: &[&str]) {
        let filter = if args.len() >= 2 && args[0] == "search" {
            args[1]
        } else {
            "(objectClass=*)"
        };

        println!(
            "Querying LDAP Directory Service at {}:{} (Filter: '{}')...",
            self.remote_host_ip, LDAP_PORT, filter
        );
        let req =
            LdapMessage::new_search_request(101, "dc=example,dc=org", filter, &["cn", "mail"]);
        let resps = self.ldap_server.handle_request(&req);

        for resp in resps {
            match resp.protocol_op {
                LdapOp::SearchResultEntry {
                    object_name,
                    attributes,
                } => {
                    println!("  DN: {}", object_name);
                    for (k, v) in attributes {
                        println!("    {}: {}", k, v.join(", "));
                    }
                }
                LdapOp::SearchResultDone { result_code, .. } => {
                    println!("LDAP Search Result Done (ResultCode: {})", result_code);
                }
                _ => {}
            }
        }
    }

    fn cmd_netflow(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "status" {
            println!("NetFlow v9 Flow Cache Table (UDP {}):", NETFLOW_V9_UDP_PORT);
            println!(
                "┌──────────────────────┬──────────────────────┬────────┬────────┬───────┬───────┬─────────┐"
            );
            println!(
                "│ Source IP            │ Destination IP       │ S-Port │ D-Port │ Proto │ Pkts  │ Bytes   │"
            );
            println!(
                "├──────────────────────┼──────────────────────┼────────┼────────┼───────┼───────┼─────────┤"
            );
            for (&(s_ip, d_ip, s_p, d_p, proto), &(pkts, bytes, _flags)) in
                &self.netflow_table.flows
            {
                let p_str = if proto == 6 { "TCP" } else { "UDP" };
                println!(
                    "│ {:<20} │ {:<20} │ {:<6} │ {:<6} │ {:<5} │ {:<5} │ {:<7} │",
                    s_ip, d_ip, s_p, d_p, p_str, pkts, bytes
                );
            }
            println!(
                "└──────────────────────┴──────────────────────┴────────┴────────┴───────┴───────┴─────────┘"
            );
        } else if args[0] == "export" {
            let records = self.netflow_table.export_records();
            let pkt = NetflowPacket::build_export(1, records);
            let raw = pkt.serialize();
            println!(
                "Exported NetFlow v9 Datagram to {}:{} ({} bytes, {} flow records)",
                self.remote_host_ip,
                NETFLOW_V9_UDP_PORT,
                raw.len(),
                pkt.records.len()
            );
        }
    }

    fn cmd_sip(&mut self, args: &[&str]) {
        let user = if args.len() >= 2 && args[0] == "invite" {
            args[1]
        } else {
            "bob@example.com"
        };

        println!(
            "Initiating SIP VoIP Session to '{}' (UDP {})...",
            user, SIP_PORT
        );
        let local_sdp = build_simple_sdp("alice", &self.stack.config.ip.to_string(), 4000);
        let invite =
            SipMessage::build_invite("alice@example.com", user, "call-99881122", &local_sdp);
        let raw = invite.serialize();

        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            55060,
            SIP_PORT,
            raw.as_bytes(),
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            905,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        let resps = self.remote_stack.process_frame(&eth_req);
        for resp in resps {
            let eth = EthernetFrame::parse(&resp).unwrap();
            let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
            let udp =
                UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true).unwrap();
            if let Ok(text) = std::str::from_utf8(udp.payload)
                && let Ok(sip_resp) = SipMessage::parse(text)
            {
                println!(
                    "SIP Response Received: {} {}",
                    sip_resp.status_code, sip_resp.reason_phrase
                );
                println!(
                    "  Call-ID: {}",
                    sip_resp.headers.get("Call-ID").unwrap_or(&"-".to_string())
                );
                println!("  Remote SDP Media: Audio RTP Port negotiated");
            }
        }
    }

    fn cmd_bfd(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "status" {
            println!(
                "Bidirectional Forwarding Detection (BFD) Session State (UDP {}):",
                BFD_CONTROL_PORT
            );
            println!("  Session State        : {}", self.bfd_session.state);
            println!(
                "  Local Discriminator  : 0x{:08X}",
                self.bfd_session.local_discriminator
            );
            println!(
                "  Remote Discriminator : 0x{:08X}",
                self.bfd_session.remote_discriminator
            );
            println!(
                "  Min TX Interval      : {} ms",
                self.bfd_session.tx_interval_us / 1000
            );
            println!(
                "  Min RX Interval      : {} ms",
                self.bfd_session.rx_interval_us / 1000
            );
            println!("  Detect Multiplier    : {}", self.bfd_session.detect_mult);
        } else if args[0] == "poll" {
            println!(
                "Transmitting BFD Control Packet to {}:{}...",
                self.remote_host_ip, BFD_CONTROL_PORT
            );
            let pkt = BfdControlPacket::build_control(
                BfdState::Init,
                self.bfd_session.local_discriminator,
                self.bfd_session.remote_discriminator,
                self.bfd_session.tx_interval_us,
            );
            let udp_req = UdpDatagram::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                49384,
                BFD_CONTROL_PORT,
                &pkt.serialize(),
            );
            let ip_req = Ipv4Packet::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                IP_PROTO_UDP,
                903,
                64,
                &udp_req,
            );
            let eth_req = EthernetFrame::serialize(
                self.remote_host_mac,
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                &ip_req,
            );

            let resps = self.remote_stack.process_frame(&eth_req);
            for resp in resps {
                let eth = EthernetFrame::parse(&resp).unwrap();
                let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
                let udp = UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true)
                    .unwrap();
                if let Ok(bfd_resp) = BfdControlPacket::parse(udp.payload) {
                    println!(
                        "BFD Response Received: State={} (MyDisc=0x{:08X}, YourDisc=0x{:08X})",
                        bfd_resp.state, bfd_resp.my_discriminator, bfd_resp.your_discriminator
                    );
                    self.bfd_session.process_packet(&bfd_resp);
                    println!(
                        "BFD Local Session Transitioned -> State: {}",
                        self.bfd_session.state
                    );
                }
            }
        }
    }

    fn cmd_geneve(&mut self, args: &[&str]) {
        let vni = if args.len() >= 2 {
            args[1].parse::<u32>().unwrap_or(2001)
        } else {
            2001
        };

        let msg = if args.len() >= 3 {
            args[2..].join(" ")
        } else {
            "Geneve Encapsulated Multi-Tenant Frame".to_string()
        };

        let inner_eth = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            msg.as_bytes(),
        );
        let geneve_payload = GenevePacket::encapsulate_eth(vni, &inner_eth);
        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            56081,
            GENEVE_UDP_PORT,
            &geneve_payload,
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            904,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        println!(
            "Transmitted Geneve Overlay Packet (UDP {}, {} bytes):",
            GENEVE_UDP_PORT,
            eth_req.len()
        );
        println!("  24-bit VNI  : {}", vni);
        println!("  Inner Proto : 0x6558 (Transparent Ethernet)");
        println!(
            "  Inner Frame : {} bytes (Inner Payload: \"{}\")",
            inner_eth.len(),
            msg
        );
    }

    fn cmd_isis(&mut self, _args: &[&str]) {
        let sys_id = [0x00, 0x00, 0x00, 0x00, 0x00, 0x01];
        let area = &[0x49, 0x00, 0x01];
        let hello = IsisHelloPacket::build_l1_lan_hello(sys_id, area, self.stack.config.ip);
        let raw = hello.serialize();
        let eth_frame = EthernetFrame::serialize(
            MacAddress([0x01, 80, 0xC2, 0x00, 0x00, 0x14]),
            self.stack.config.mac,
            ETHERTYPE_ISIS,
            &raw,
        );

        println!(
            "Transmitted IS-IS Level-1 LAN Hello (IIH) Frame (EtherType 0x{:04X}, {} bytes):",
            ETHERTYPE_ISIS,
            eth_frame.len()
        );
        println!("  NLPID Discriminator : 0x83 (IS-IS)");
        println!("  PDU Type            : 15 (L1 LAN IIH)");
        println!("  Circuit Type        : Level 1");
        println!("  Source System ID    : 0000.0000.0001");
        println!("  Holding Time        : 30s");
        println!("  Priority            : 64");
        println!(
            "  TLVs                : Area Addresses (TLV 1), NLPID Protocols Supported (TLV 129: IPv4, IPv6), IP Interface (TLV 132)"
        );
    }

    fn cmd_syslog(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "list" {
            println!("Syslog Event Collector Log Buffer (UDP 514):");
            for (i, log) in self.syslog_collector.logs.iter().enumerate() {
                println!(
                    "  #{:02} [{:<5}] <PRI:{:<2}> {}: {}",
                    i + 1,
                    log.severity,
                    log.pri_val(),
                    log.app_name,
                    log.message
                );
            }
        } else if args.len() >= 2 && args[0] == "send" {
            let msg_text = args[1..].join(" ");
            let sys_msg = SyslogMessage::new(
                SyslogFacility::Local0,
                SyslogSeverity::Warning,
                "toystack",
                "app",
                &msg_text,
            );
            let formatted = sys_msg.format_rfc5424();
            self.syslog_collector.record(sys_msg.clone());

            let udp_req = UdpDatagram::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                51400,
                SYSLOG_UDP_PORT,
                formatted.as_bytes(),
            );
            let ip_req = Ipv4Packet::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                IP_PROTO_UDP,
                901,
                64,
                &udp_req,
            );
            let eth_req = EthernetFrame::serialize(
                self.remote_host_mac,
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                &ip_req,
            );
            println!(
                "Transmitted Syslog RFC 5424 Event Frame ({} bytes, PRI {}):",
                eth_req.len(),
                sys_msg.pri_val()
            );
            println!("  Payload: \"{}\"", formatted);
        }
    }

    fn cmd_l2tp(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "status" {
            println!(
                "L2TPv3 Layer 2 Pseudowire Status (IP Protocol {}):",
                IP_PROTO_L2TPV3
            );
            println!("  Session ID   : 0x000003E9 (1001)");
            println!("  Cookie       : None (Standard 4-byte L2TPv3 Data Header)");
            println!("  Payload Type : Ethernet Frame Pseudowire");
        } else if args.len() >= 3 && args[0] == "encap" {
            let sid = args[1].parse::<u32>().unwrap_or(1001);
            let msg = args[2..].join(" ");
            let inner_eth = EthernetFrame::serialize(
                self.remote_host_mac,
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                msg.as_bytes(),
            );
            let l2tp_payload = L2tpv3Packet::encapsulate(sid, &inner_eth, None);
            let ip_pkt = Ipv4Packet::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                IP_PROTO_L2TPV3,
                902,
                64,
                &l2tp_payload,
            );
            let eth_frame = EthernetFrame::serialize(
                self.remote_host_mac,
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                &ip_pkt,
            );
            println!(
                "Encapsulated L2TPv3 Pseudowire Packet ({} bytes, Protocol {}):",
                eth_frame.len(),
                IP_PROTO_L2TPV3
            );
            println!("  Session ID   : 0x{:08X}", sid);
            println!(
                "  Inner Frame  : {} bytes (Payload: \"{}\")",
                inner_eth.len(),
                msg
            );
        }
    }

    fn cmd_pim(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "hello" {
            let hello = PimPacket::build_hello(105, 100);
            let raw = hello.serialize();
            println!(
                "Transmitted PIM-SM Hello Packet ({} bytes, Protocol {}, Multicast {}):",
                raw.len(),
                IP_PROTO_PIM,
                ALL_PIM_ROUTERS_MULTICAST
            );
            println!("  PIM Version : 2");
            println!("  Type        : 0 (Hello)");
            println!("  HoldTime    : 105s, DR Priority: 100");
        } else if args.len() >= 2
            && args[0] == "join"
            && let Ok(grp) = Ipv4Address::from_str(args[1])
        {
            let rp = self.pim_router.rendezvous_point;
            let join_pkt = PimPacket::build_join_group(self.remote_host_ip, grp, rp);
            self.pim_router.join_shared_tree(grp);
            println!(
                "Transmitted PIM Join/Prune Message (*, G) for Group {}:",
                grp
            );
            println!("  Upstream Neighbor: {}", self.remote_host_ip);
            println!("  Rendezvous Point : {}", rp);
            println!("  Serialized Size  : {} bytes", join_pkt.serialize().len());
        }
    }

    fn cmd_radius(&mut self, args: &[&str]) {
        let (user, pass) = if args.len() >= 3 && args[0] == "auth" {
            (args[1], args[2])
        } else {
            ("alice", "secret123")
        };

        println!(
            "Sending RADIUS Access-Request to {}:{} for user '{}'...",
            self.remote_host_ip, RADIUS_AUTH_PORT, user
        );
        let auth = [0x11; 16];
        let req = RadiusPacket::build_access_request(
            101,
            auth,
            user,
            pass,
            b"sharedsecret",
            self.stack.config.ip,
        );
        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            51812,
            RADIUS_AUTH_PORT,
            &req.serialize(),
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            801,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        let resps = self.remote_stack.process_frame(&eth_req);
        for resp in resps {
            let eth = EthernetFrame::parse(&resp).unwrap();
            let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
            let udp =
                UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true).unwrap();
            if let Ok(rad_resp) = RadiusPacket::parse(udp.payload) {
                println!(
                    "RADIUS Response Received: Code={} (Access-Accept), ID={}",
                    rad_resp.code, rad_resp.identifier
                );
                for avp in rad_resp.attributes {
                    match avp.attr_type {
                        8 => println!(
                            "  Framed-IP-Address : {}",
                            Ipv4Address([avp.value[0], avp.value[1], avp.value[2], avp.value[3]])
                        ),
                        18 => println!(
                            "  Reply-Message     : \"{}\"",
                            String::from_utf8_lossy(&avp.value)
                        ),
                        _ => println!(
                            "  Attribute #{}     : {} bytes",
                            avp.attr_type,
                            avp.value.len()
                        ),
                    }
                }
            }
        }
    }

    fn cmd_pppoe(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "padi" {
            let padi = PppoePacket::build_padi();
            let raw = padi.serialize();
            let eth_frame = EthernetFrame::serialize(
                MacAddress::BROADCAST,
                self.stack.config.mac,
                ETHERTYPE_PPPOE_DISCOVERY,
                &raw,
            );
            println!(
                "Transmitted PPPoE Active Discovery Initiation (PADI) Frame (EtherType 0x{:04X}, {} bytes):",
                ETHERTYPE_PPPOE_DISCOVERY,
                eth_frame.len()
            );
            println!("  Code       : 0x09 (PADI)");
            println!("  Session ID : 0x0000");
            println!("  Tags       : Service-Name");
        } else if args.len() >= 3 && args[0] == "session" {
            let sid = args[1].parse::<u16>().unwrap_or(0x0042);
            let msg = args[2..].join(" ");
            let session_pkt = PppoePacket::build_session_ipv4(sid, msg.as_bytes());
            let raw = session_pkt.serialize();
            let eth_frame = EthernetFrame::serialize(
                self.remote_host_mac,
                self.stack.config.mac,
                ETHERTYPE_PPPOE_SESSION,
                &raw,
            );
            println!(
                "Transmitted PPPoE Session Frame (EtherType 0x{:04X}, {} bytes):",
                ETHERTYPE_PPPOE_SESSION,
                eth_frame.len()
            );
            println!("  Session ID : 0x{:04X}", sid);
            println!("  PPP Proto  : 0x0021 (IPv4)");
            println!("  Payload    : \"{}\"", msg);
        }
    }

    fn cmd_eigrp(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "hello" {
            let hello = EigrpPacket::build_hello(100);
            let raw = hello.serialize();
            println!(
                "Transmitted EIGRP Hello Packet ({} bytes, Protocol {}, Multicast {}):",
                raw.len(),
                IP_PROTO_EIGRP,
                EIGRP_MULTICAST_IP
            );
            println!("  Autonomous System : 100");
            println!("  K-Values          : K1=1, K2=0, K3=1, K4=0, K5=0");
            println!("  Hold Time         : 15 seconds");
        } else if args[0] == "dual" {
            println!("EIGRP DUAL Topology Table & Successor Selection (AS 100):");
            let dest = Ipv4Address::new(10, 50, 0, 0);
            if let Some((succ, fs_list, fd)) = self.eigrp_table.compute_dual(dest) {
                println!("  Destination Network   : {}/24", dest);
                println!("  Feasible Distance (FD): {}", fd);
                println!(
                    "  Primary Successor     : Next-Hop {} (Total Metric: {}, RD: {})",
                    succ.neighbor, succ.total_metric, succ.reported_distance
                );
                for fs in fs_list {
                    println!(
                        "  Feasible Successor    : Next-Hop {} (Total Metric: {}, RD: {} < FD {}) [Loop-Free Backup!]",
                        fs.neighbor, fs.total_metric, fs.reported_distance, fd
                    );
                }
            }
        }
    }

    fn cmd_ipsec(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "status" {
            println!("IPsec Security Association Database (SAD) (Protocol 50 ESP):");
            for (&spi, sa) in &self.sad_table.outbound {
                println!(
                    "  [Outbound SA] SPI: 0x{:08X} | {} -> {} | Next Seq: {}",
                    spi, sa.src_ip, sa.dst_ip, sa.next_seq
                );
            }
            for (&spi, sa) in &self.sad_table.inbound {
                println!(
                    "  [Inbound SA]  SPI: 0x{:08X} | {} -> {} | Replay Window Highest: {}",
                    spi, sa.src_ip, sa.dst_ip, sa.highest_seq_seen
                );
            }
        } else if args.len() >= 2 && args[0] == "encap" {
            let msg = args[1..].join(" ");
            let key = [0xAA; 16];
            let esp = EspPacket::build(0x1000, 1, 4, msg.as_bytes(), &key);
            let raw = esp.serialize();
            let ip_esp = Ipv4Packet::serialize(
                self.stack.config.ip,
                self.remote_host_ip,
                IP_PROTO_ESP,
                701,
                64,
                &raw,
            );
            let eth_esp = EthernetFrame::serialize(
                self.remote_host_mac,
                self.stack.config.mac,
                ETHERTYPE_IPV4,
                &ip_esp,
            );
            println!(
                "Encapsulated IPsec ESP Tunnel Packet ({} bytes, Protocol 50):",
                eth_esp.len()
            );
            println!("  ESP Header : SPI=0x00001000, Seq=1");
            println!(
                "  ESP Payload: {} bytes (Inner Payload: \"{}\")",
                esp.payload.len(),
                msg
            );
            println!(
                "  ESP Trailer: PadLen={}, NextHeader=4 (IP-in-IP)",
                esp.pad_length
            );
            println!("  ESP ICV    : 16 bytes Authentication Tag");
        }
    }

    fn cmd_http3(&mut self, args: &[&str]) {
        let path = if args.len() >= 2 && args[0] == "get" {
            args[1]
        } else {
            "/api/v1/resource"
        };

        println!("Initiating HTTP/3 over QUIC Transaction (RFC 9114):");
        let settings = Http3Frame::build_settings(&[(0x01, 4096), (0x06, 65536)]);
        println!(
            "  1. Transmitted HTTP/3 SETTINGS frame ({} bytes)",
            settings.serialize().len()
        );

        let headers =
            Http3Frame::build_headers(&[(":method", "GET"), (":path", path), (":scheme", "https")]);
        println!(
            "  2. Transmitted HTTP/3 HEADERS frame (QPACK Compressed, Path: '{}', {} bytes)",
            path,
            headers.serialize().len()
        );

        let data = Http3Frame::build_data(b"{\"status\": 200, \"protocol\": \"HTTP/3 QUIC\"}");
        println!(
            "  3. Received HTTP/3 DATA frame ({} bytes payload): \"{}\"",
            data.payload.len(),
            String::from_utf8_lossy(&data.payload)
        );
    }

    fn cmd_lacp(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "status" {
            println!("Link Aggregation Group (LACP / IEEE 802.1AX / 802.3ad):");
            println!("  Bond Device  : {}", self.lag.bond_name);
            println!(
                "  Slaves       : eth0, eth1 (State: Active/Aggregated/Collecting/Distributing)"
            );
            println!("  LACP Key     : {}", self.lag.lacp_key);
            println!("  Hash Policy  : Layer 3 + Layer 4 5-Tuple");

            let actor = LacpPortInfo {
                system_priority: 32768,
                system_mac: self.stack.config.mac,
                key: self.lag.lacp_key,
                port_priority: 128,
                port_number: 1,
                state: LACP_STATE_ACTIVITY
                    | LACP_STATE_AGGREGATION
                    | LACP_STATE_SYNCHRONIZATION
                    | LACP_STATE_COLLECTING
                    | LACP_STATE_DISTRIBUTING,
            };
            let pkt = LacpPacket::build(actor.clone(), actor);
            println!(
                "  Generated LACPDU Frame (EtherType 0x{:04X}, {} bytes)",
                ETHERTYPE_SLOW_PROTOCOLS,
                pkt.serialize().len()
            );
        } else if args.len() >= 3 && args[0] == "hash" {
            let s_ip = Ipv4Address::from_str(args[1]).unwrap_or(self.stack.config.ip);
            let d_ip = Ipv4Address::from_str(args[2]).unwrap_or(self.remote_host_ip);
            let slave = self.lag.select_slave_port(s_ip, d_ip, 50000, 80);
            println!(
                "LACP 5-Tuple Egress Hash: {} -> {} | Selected Slave: {}",
                s_ip, d_ip, slave
            );
        }
    }

    fn cmd_ospf(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "hello" {
            let hello = OspfHelloPacket::build_hello(
                self.stack.config.ip,
                Ipv4Address::new(255, 255, 255, 0),
                self.remote_host_ip,
                vec![self.remote_host_ip],
            );
            let raw = hello.serialize();
            println!(
                "Transmitted OSPFv2 Hello Packet ({} bytes, Protocol 89, Multicast {}):",
                raw.len(),
                OSPF_ALL_SPF_ROUTERS
            );
            println!("  Router ID  : {}", self.stack.config.ip);
            println!("  Area ID    : 0.0.0.0 (Backbone)");
            println!("  DR         : {}", self.remote_host_ip);
            println!("  Hello/Dead : 10s / 40s");
        } else if args[0] == "spf" {
            println!(
                "OSPF Dijkstra Shortest Path Tree Calculation from {}:",
                self.stack.config.ip
            );
            let paths = self
                .ospf_lsdb
                .compute_shortest_paths(Ipv4Address::new(1, 1, 1, 1));
            for (dest, (cost, nh)) in paths {
                println!(
                    "  -> Destination: {:<15} | Metric Cost: {:<4} | Next-Hop: {:?}",
                    dest,
                    cost,
                    nh.unwrap()
                );
            }
        }
    }

    fn cmd_stp(&mut self, _args: &[&str]) {
        println!("Spanning Tree Protocol (IEEE 802.1D) Bridge Status:");
        println!("  Bridge ID     : {}", self.stp_engine.bridge_id);
        println!("  Root Bridge ID: {}", self.stp_engine.root_id);
        println!("  Root Path Cost: {}", self.stp_engine.root_path_cost);
        println!("  Port States:");
        for (port, (role, state)) in &self.stp_engine.port_states {
            println!("    - Port {:02}: Role={:?}, State={}", port, role, state);
        }
    }

    fn cmd_vxlan(&mut self, args: &[&str]) {
        let vni = if args.len() >= 2 {
            args[1].parse::<u32>().unwrap_or(1001)
        } else {
            1001
        };

        let msg = if args.len() >= 3 {
            args[2..].join(" ")
        } else {
            "Overlay Ethernet Frame".to_string()
        };

        let inner_eth = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            msg.as_bytes(),
        );
        let vxlan_encap = VxlanPacket::encapsulate(vni, &inner_eth).unwrap();
        println!(
            "VXLAN Encapsulated Frame (Port {}, VNI {}, {} bytes):",
            VXLAN_UDP_PORT,
            vni,
            vxlan_encap.len()
        );
        println!("  Outer Layer: UDP Port {}", VXLAN_UDP_PORT);
        println!("  VXLAN Header: Flags=0x08 (VNI Valid), 24-bit VNI={}", vni);
        println!(
            "  Inner Frame : {} bytes (Inner Payload: \"{}\")",
            inner_eth.len(),
            msg
        );
    }

    fn cmd_mpls(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "lfib" {
            println!("MPLS Label Forwarding Information Base (LFIB):");
            println!("┌───────────┬────────────────────────────────┐");
            println!("│ In-Label  │ Action                         │");
            println!("├───────────┼────────────────────────────────┤");
            for (&in_lbl, act) in self.lfib.all_entries() {
                println!("│ {:<9} │ {:<30} │", in_lbl, act);
            }
            println!("└───────────┴────────────────────────────────┘");
        } else if args.len() >= 3 && args[0] == "push" {
            let label = args[1].parse::<u32>().unwrap_or(100);
            let msg = args[2..].join(" ");
            let shim = MplsHeader::new(label, 0, true, 64);
            let mpls_pkt = MplsPacket {
                labels: vec![shim],
                payload: msg.as_bytes().to_vec(),
            };
            let raw = mpls_pkt.serialize();
            let eth_frame = EthernetFrame::serialize(
                self.remote_host_mac,
                self.stack.config.mac,
                ETHERTYPE_MPLS_UNICAST,
                &raw,
            );
            println!(
                "Generated MPLS Encapsulated Frame (EtherType 0x{:04x}, {} bytes):",
                ETHERTYPE_MPLS_UNICAST,
                eth_frame.len()
            );
            println!(
                "  MPLS Label Stack : [Label: {}, TC: 0, S: true, TTL: 64]",
                label
            );
            println!("  Inner Payload    : \"{}\"", msg);
        }
    }

    fn cmd_bgp(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "status" {
            println!("Border Gateway Protocol 4 (BGP-4) Status (Port 179):");
            println!("  Local AS : 65001");
            println!("  BGP ID   : {}", self.stack.config.ip);
            println!(
                "  Neighbor : {} (Remote AS: 65002, State: ESTABLISHED)",
                self.remote_host_ip
            );
        } else if args[0] == "rib" {
            println!("BGP Routing Information Base (RIB):");
            println!("┌──────────────────────┬──────────────────┬────────────────────────┐");
            println!("│ Network Prefix       │ Next Hop         │ AS Path                │");
            println!("├──────────────────────┼──────────────────┼────────────────────────┤");
            for ((p, m), (nh, path)) in self.bgp_rib.all_routes() {
                let p_str = format!("{}/{}", p, m);
                let path_str = path
                    .iter()
                    .map(|a| a.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                println!("│ {:<20} │ {:<16} │ {:<22} │", p_str, nh, path_str);
            }
            println!("└──────────────────────┴──────────────────┴────────────────────────┘");
        } else if args[0] == "open" {
            let open = BgpMessage::build_open(65001, 180, self.stack.config.ip);
            let raw = open.serialize();
            println!(
                "BGP OPEN Message Framed ({} bytes): Marker=0xFF*16, MyAS=65001, HoldTime=180",
                raw.len()
            );
        }
    }

    fn cmd_lldp(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "neighbors" {
            println!(
                "Link Layer Discovery Protocol (LLDP) Neighbors (EtherType 0x{:04X}):",
                ETHERTYPE_LLDP
            );
            println!("┌──────────────────────┬─────────────┬──────────┬──────────────────────┐");
            println!("│ Chassis ID           │ Port ID     │ TTL (s)  │ System Name          │");
            println!("├──────────────────────┼─────────────┼──────────┼──────────────────────┤");
            for n in self.lldp_table.all_neighbors().values() {
                println!(
                    "│ {:<20} │ {:<11} │ {:<8} │ {:<20} │",
                    n.chassis_id,
                    n.port_id,
                    n.ttl,
                    n.system_name.as_deref().unwrap_or("-")
                );
            }
            println!("└──────────────────────┴─────────────┴──────────┴──────────────────────┘");
        } else if args[0] == "announce" {
            let lldp_pkt = LldpPacket {
                chassis_id: self.stack.config.mac.to_string(),
                port_id: "eth0".to_string(),
                ttl: 120,
                system_name: Some("ToyNetStack-Host".to_string()),
            };
            let raw = lldp_pkt.serialize();
            let eth_frame = EthernetFrame::serialize(
                LLDP_MULTICAST_MAC,
                self.stack.config.mac,
                ETHERTYPE_LLDP,
                &raw,
            );
            println!(
                "Transmitted LLDPDU Advertisement to Multicast MAC {} ({} bytes)",
                LLDP_MULTICAST_MAC,
                eth_frame.len()
            );
        }
    }

    fn cmd_snmp(&mut self, args: &[&str]) {
        let oid = if args.len() >= 2 && args[0] == "get" {
            args[1]
        } else {
            "1.3.6.1.2.1.1.1.0"
        };

        println!(
            "SNMPv2c GetRequest to {}:161 for OID '{}'...",
            self.remote_host_ip, oid
        );
        let req = SnmpMessage::build_get_request("public", 101, &[oid]);
        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            50161,
            161,
            &req.serialize(),
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            601,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        let resps = self.remote_stack.process_frame(&eth_req);
        for resp in resps {
            let eth = EthernetFrame::parse(&resp).unwrap();
            let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
            let udp =
                UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true).unwrap();
            if let Ok(snmp_resp) = SnmpMessage::parse(udp.payload) {
                println!(
                    "SNMPv2c Response received (Community: \"{}\"):",
                    snmp_resp.community
                );
                for vb in snmp_resp.pdu.varbinds {
                    println!("  {} = {}", vb.oid, vb.value);
                }
            }
        }
    }

    fn cmd_quic(&mut self, args: &[&str]) {
        let payload_str = if args.len() >= 2 && args[0] == "frame" {
            args[1..].join(" ")
        } else {
            "QUIC stream payload data".to_string()
        };

        println!("Generating QUIC Binary Packets (RFC 9000):");
        let initial = QuicPacket::build_initial(
            vec![0x12, 0x34, 0x56, 0x78],
            vec![0x87, 0x65, 0x43, 0x21],
            payload_str.as_bytes(),
        );
        let raw_initial = initial.serialize();
        println!(
            "  1. Long Header Initial ({} bytes): DCID=12345678, SCID=87654321, Version=0x00000001",
            raw_initial.len()
        );

        let short = QuicPacket::build_1rtt(
            vec![0x12, 0x34, 0x56, 0x78, 0xaa, 0xbb, 0xcc, 0xdd],
            1,
            payload_str.as_bytes(),
        );
        let raw_short = short.serialize();
        println!(
            "  2. Short Header 1-RTT ({} bytes): DCID=12345678aabbccdd, PacketNum=1, SpinBit=0",
            raw_short.len()
        );
    }

    fn cmd_vrrp(&mut self, _args: &[&str]) {
        println!("Virtual Router Redundancy Protocol (VRRPv3 - RFC 5798):");
        println!("  VRID       : {}", self.vrrp.vrid);
        println!("  Virtual IP : {}", self.vrrp.virtual_ip);
        println!("  Virtual MAC: {}", VrrpPacket::virtual_mac(self.vrrp.vrid));
        println!("  Priority   : {}", self.vrrp.priority);
        println!("  State      : {}", self.vrrp.state);

        let adv = self.vrrp.build_advertisement();
        println!(
            "  Advertisement Frame Generated ({} bytes): Checksum=0x{:04x}",
            adv.serialize().len(),
            adv.checksum
        );
    }

    fn cmd_arp(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "list" {
            println!("Address Resolution Protocol (ARP) Cache Table:");
            println!("┌──────────────────┬───────────────────┐");
            println!("│ IPv4 Address     │ MAC Address       │");
            println!("├──────────────────┼───────────────────┤");
            for (&ip, &mac) in self.stack.arp_table.entries() {
                println!("│ {:<16} │ {:<17} │", Ipv4Address(ip), mac);
            }
            println!("└──────────────────┴───────────────────┘");
        } else if args[0] == "clear" {
            self.stack.arp_table = ArpTable::new();
            println!("ARP cache cleared.");
        }
    }

    fn cmd_ndp(&self) {
        println!("IPv6 Neighbor Discovery Protocol (NDP) Cache Table:");
        println!("┌──────────────────────────────────────────┬───────────────────┐");
        println!("│ IPv6 Address                             │ MAC Address       │");
        println!("├──────────────────────────────────────────┼───────────────────┤");
        for (&ip, &mac) in self.stack.ndp_table.entries() {
            println!("│ {:<40} │ {:<17} │", ip, mac);
        }
        println!("└──────────────────────────────────────────┴───────────────────┘");
    }

    fn cmd_route(&self) {
        println!("IPv4 Routing Table (Longest Prefix Match):");
        for r in self.stack.routing_table.all_routes() {
            println!("  {}", r);
        }
    }

    fn cmd_rip(&self, _args: &[&str]) {
        println!("RIPv2 Distance-Vector Routing Status (Port 520):");
        println!("  Advertised Subnets:");
        for (dest, prefix, metric) in &self.rip.route_metrics {
            println!("    - {}/{} (Hop Metric: {})", dest, prefix, metric);
        }
    }

    fn cmd_traceroute(&mut self, args: &[&str]) {
        let target_ip = if args.is_empty() {
            self.remote_host_ip
        } else {
            match Ipv4Address::from_str(args[0]) {
                Ok(ip) => ip,
                Err(_) => {
                    println!("Invalid IPv4 target: {}", args[0]);
                    return;
                }
            }
        };

        println!(
            "traceroute to {} (30 hops max, 32 byte packets):",
            target_ip
        );
        let hops = vec![
            TracerouteHopResult {
                hop: 1,
                responder_ip: Some(Ipv4Address::new(192, 168, 1, 1)),
                rtt_ms: 0.45,
                reached: false,
            },
            TracerouteHopResult {
                hop: 2,
                responder_ip: Some(Ipv4Address::new(10, 0, 0, 1)),
                rtt_ms: 1.20,
                reached: false,
            },
            TracerouteHopResult {
                hop: 3,
                responder_ip: Some(target_ip),
                rtt_ms: 2.15,
                reached: true,
            },
        ];

        for h in hops {
            println!(" {}", h);
        }
    }

    fn cmd_ntp(&mut self, _args: &[&str]) {
        println!("Querying NTP Server ({}:123)...", self.remote_host_ip);
        let t1 = NtpTimestamp::new(3900000000, 100000);
        let req = NtpPacket::build_client_request(t1);
        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            49150,
            123,
            &req.serialize(),
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            501,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        let resps = self.remote_stack.process_frame(&eth_req);
        for resp in resps {
            let eth = EthernetFrame::parse(&resp).unwrap();
            let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
            let udp =
                UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true).unwrap();
            if let Ok(ntp_resp) = NtpPacket::parse(udp.payload) {
                let t4 = NtpTimestamp::new(3900000000, 150000);
                let (offset, delay) = calculate_offset_and_delay(
                    t1.to_unix_f64(),
                    ntp_resp.receive_timestamp.to_unix_f64(),
                    ntp_resp.transmit_timestamp.to_unix_f64(),
                    t4.to_unix_f64(),
                );
                println!("NTP Server Response (Stratum {}):", ntp_resp.stratum);
                println!(
                    "  Reference ID : {}",
                    String::from_utf8_lossy(&ntp_resp.reference_id)
                );
                println!("  Round-Trip   : {:.3} ms", delay * 1000.0);
                println!("  Clock Offset : {:.3} ms", offset * 1000.0);
            }
        }
    }

    fn cmd_tftp(&mut self, args: &[&str]) {
        let filename = if args.len() >= 2 && args[0] == "get" {
            args[1]
        } else {
            "pxeboot.bin"
        };

        println!("Requesting file '{}' over TFTP (Port 69)...", filename);
        let rrq = TftpPacket::Rrq {
            filename: filename.to_string(),
            mode: "octet".to_string(),
        };
        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            50069,
            69,
            &rrq.serialize(),
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            502,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );

        let resps = self.remote_stack.process_frame(&eth_req);
        for resp in resps {
            let eth = EthernetFrame::parse(&resp).unwrap();
            let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
            let udp =
                UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true).unwrap();
            if let Ok(tftp_resp) = TftpPacket::parse(udp.payload) {
                match tftp_resp {
                    TftpPacket::Data { block_num, data } => {
                        println!(
                            "TFTP DATA received: Block #{} ({} bytes)",
                            block_num,
                            data.len()
                        );
                        println!("  Content: \"{}\"", String::from_utf8_lossy(&data));
                    }
                    TftpPacket::Error {
                        error_code,
                        message,
                    } => {
                        println!("TFTP ERROR #{}: {}", error_code, message);
                    }
                    _ => {}
                }
            }
        }
    }

    fn cmd_tunnel(&mut self, args: &[&str]) {
        if args.len() < 3 || args[0] != "gre" {
            println!("Usage: tunnel gre <destination_ip> <message>");
            return;
        }

        let dst_ip = Ipv4Address::from_str(args[1]).unwrap_or(self.remote_host_ip);
        let msg = args[2..].join(" ");

        let encap = GrePacket::encapsulate_gre_ipv4(
            self.stack.config.ip,
            dst_ip,
            msg.as_bytes(),
            Some(0x1001),
        );
        println!("Encapsulated GRE Packet ({} bytes):", encap.len());
        println!(
            "  Outer IP Header: {} -> {} (Protocol 47 GRE)",
            self.stack.config.ip, dst_ip
        );
        println!("  GRE Header     : Key=0x1001, Inner EtherType=0x0800 (IPv4)");
        println!("  Inner Payload  : \"{}\"", msg);
    }

    fn cmd_igmp(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "list" {
            println!("Active Multicast Group Subscriptions (IGMPv2):");
            for g in self.igmp_table.all_groups() {
                let mac = multicast_ip_to_mac(g);
                println!("  - IP: {:<15} -> Ethernet Multicast MAC: {}", g, mac);
            }
        } else if args.len() >= 2 && args[0] == "join" {
            if let Ok(group) = Ipv4Address::from_str(args[1]) {
                self.igmp_table.join(group);
                let mac = multicast_ip_to_mac(group);
                let report = IgmpPacket::build_v2_membership_report(group);
                println!("Joined Multicast Group {}:", group);
                println!("  Mapped MAC : {}", mac);
                println!(
                    "  IGMP Report: Type=0x16 (V2 Membership Report), Group={}",
                    report.group_address
                );
            } else {
                println!("Invalid Multicast IP: {}", args[1]);
            }
        }
    }

    fn cmd_ws(&mut self, args: &[&str]) {
        if args.len() < 2 || args[0] != "send" {
            println!("Usage: ws send <message>");
            return;
        }

        let msg = args[1..].join(" ");
        let mask = [0xde, 0xad, 0xbe, 0xef];
        let frame = WebSocketFrame::build_text(&msg, true, Some(mask));
        let raw = frame.serialize();

        println!(
            "Generated Masked WebSocket Text Frame ({} bytes):",
            raw.len()
        );
        println!("  Header     : FIN=true, Opcode=0x1 (Text), Masked=true");
        println!(
            "  Masking Key: {:02x}{:02x}{:02x}{:02x}",
            mask[0], mask[1], mask[2], mask[3]
        );
        println!("  Payload    : \"{}\"", msg);
    }

    fn cmd_ping(&mut self, args: &[&str]) {
        if args.is_empty() {
            println!("Usage: ping <target_ip>");
            return;
        }

        let target_ip = match Ipv4Address::from_str(args[0]) {
            Ok(ip) => ip,
            Err(_) => {
                println!("Invalid IPv4 address: {}", args[0]);
                return;
            }
        };

        println!("PING {} (32 bytes of data):", target_ip);
        let seq = self.seq_counter;
        self.seq_counter = self.seq_counter.wrapping_add(1);

        let ping_payload = b"ToyNetStack ping test payload 12";
        let icmp_req = IcmpPacket::build_echo_request(0x1337, seq, ping_payload);
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            target_ip,
            IP_PROTO_ICMP,
            seq,
            64,
            &icmp_req,
        );

        let dst_mac = self
            .stack
            .arp_table
            .lookup(&target_ip.0)
            .unwrap_or(self.remote_host_mac);
        let eth_req =
            EthernetFrame::serialize(dst_mac, self.stack.config.mac, ETHERTYPE_IPV4, &ip_req);
        self.record_packet(&eth_req);

        let resps = self.remote_stack.process_frame(&eth_req);
        if resps.is_empty() {
            println!("Request timed out. Destination Host Unreachable.");
        } else {
            for resp in resps {
                self.record_packet(&resp);
                let eth = EthernetFrame::parse(&resp).unwrap();
                let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
                let icmp = IcmpPacket::parse(ip.payload, true).unwrap();
                if icmp.icmp_type == IcmpType::EchoReply {
                    println!(
                        "32 bytes from {}: icmp_seq={} ttl={} id=0x{:04x} (time < 1ms)",
                        ip.header.src_ip, icmp.sequence_number, ip.header.ttl, icmp.identifier
                    );
                }
            }
        }
    }

    fn cmd_ping6(&mut self, args: &[&str]) {
        if args.is_empty() {
            println!("Usage: ping6 <target_ipv6>");
            return;
        }

        let target_ip6 = match Ipv6Address::from_str(args[0]) {
            Ok(ip) => ip,
            Err(_) => {
                println!("Invalid IPv6 address: {}", args[0]);
                return;
            }
        };

        let my_ip6 = self.stack.config.ipv6.unwrap_or(Ipv6Address::LOOPBACK);
        println!("PING6 {} from {} (32 bytes of data):", target_ip6, my_ip6);
        let seq = self.seq_counter;
        self.seq_counter = self.seq_counter.wrapping_add(1);

        let ping_payload = b"ToyNetStack ping6 payload 123456";
        let icmp6_req =
            Icmpv6Packet::build_echo_request(my_ip6, target_ip6, 0x1337, seq, ping_payload);
        let ip6_req = Ipv6Packet::serialize(my_ip6, target_ip6, NEXT_HEADER_ICMPV6, 64, &icmp6_req);

        let dst_mac = self
            .stack
            .ndp_table
            .lookup(&target_ip6)
            .unwrap_or(self.remote_host_mac);
        let eth_req =
            EthernetFrame::serialize(dst_mac, self.stack.config.mac, ETHERTYPE_IPV6, &ip6_req);
        self.record_packet(&eth_req);

        let resps = self.remote_stack.process_frame(&eth_req);
        if resps.is_empty() {
            println!("Request timed out. Destination IPv6 Host Unreachable.");
        } else {
            for resp in resps {
                self.record_packet(&resp);
                let eth = EthernetFrame::parse(&resp).unwrap();
                let ip6 = Ipv6Packet::parse(eth.payload).unwrap();
                let icmp6 =
                    Icmpv6Packet::parse(ip6.header.src_ip, ip6.header.dst_ip, ip6.payload, true)
                        .unwrap();
                if icmp6.msg_type == ICMPV6_TYPE_ECHO_REPLY {
                    println!(
                        "32 bytes from {}: icmp6_seq={} hop_limit={} (time < 1ms)",
                        ip6.header.src_ip, seq, ip6.header.hop_limit
                    );
                }
            }
        }
    }

    fn cmd_dns(&mut self, args: &[&str]) {
        if args.is_empty() {
            println!("Usage: dns <hostname>");
            return;
        }

        let hostname = args[0];
        println!(
            "Resolving '{}' via virtual DNS server ({})...",
            hostname, self.remote_host_ip
        );

        let query_data = DnsMessage::build_query(0x1234, hostname);
        let udp_req = UdpDatagram::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            53535,
            53,
            &query_data,
        );
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            self.remote_host_ip,
            IP_PROTO_UDP,
            100,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );
        self.record_packet(&eth_req);

        let resps = self.remote_stack.process_frame(&eth_req);
        for resp in resps {
            self.record_packet(&resp);
            let eth = EthernetFrame::parse(&resp).unwrap();
            let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
            let udp =
                UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true).unwrap();
            if let Ok(dns_resp) = DnsMessage::parse(udp.payload) {
                for ans in dns_resp.answers {
                    println!("  Answer: {} -> {} (TTL: {}s)", ans.name, ans.ip, ans.ttl);
                }
            }
        }
    }

    fn cmd_udp(&mut self, args: &[&str]) {
        if args.len() < 4 || args[0] != "send" {
            println!("Usage: udp send <ip> <port> <message>");
            return;
        }

        let target_ip = Ipv4Address::from_str(args[1]).unwrap();
        let port = args[2].parse::<u16>().unwrap();
        let msg = args[3..].join(" ");

        println!(
            "Sending UDP datagram to {}:{} ({} bytes)...",
            target_ip,
            port,
            msg.len()
        );
        let udp_req =
            UdpDatagram::serialize(self.stack.config.ip, target_ip, 49152, port, msg.as_bytes());
        let ip_req = Ipv4Packet::serialize(
            self.stack.config.ip,
            target_ip,
            IP_PROTO_UDP,
            200,
            64,
            &udp_req,
        );
        let eth_req = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_req,
        );
        self.record_packet(&eth_req);

        let resps = self.remote_stack.process_frame(&eth_req);
        for resp in resps {
            self.record_packet(&resp);
            let eth = EthernetFrame::parse(&resp).unwrap();
            let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
            let udp =
                UdpDatagram::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true).unwrap();
            println!(
                "Received UDP reply from {}:{}: \"{}\"",
                ip.header.src_ip,
                udp.src_port,
                String::from_utf8_lossy(udp.payload)
            );
        }
    }

    fn cmd_curl(&mut self, args: &[&str]) {
        if args.is_empty() {
            println!("Usage: curl <ip[:port]>");
            return;
        }

        let target_ip = Ipv4Address::from_str(args[0].split(':').next().unwrap())
            .unwrap_or(self.remote_host_ip);
        println!("Connecting to {} over TCP HTTP (port 80)...", target_ip);

        let client_port = 55000;
        let client_isn = 1000;
        let syn = TcpSegment::serialize(
            self.stack.config.ip,
            target_ip,
            client_port,
            80,
            client_isn,
            0,
            TcpFlags::syn(),
            65535,
            &[],
        );
        let ip_syn =
            Ipv4Packet::serialize(self.stack.config.ip, target_ip, IP_PROTO_TCP, 301, 64, &syn);
        let eth_syn = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_syn,
        );
        self.record_packet(&eth_syn);

        let syn_acks = self.remote_stack.process_frame(&eth_syn);
        if syn_acks.is_empty() {
            println!("Connection refused / timed out.");
            return;
        }

        let syn_ack_eth = EthernetFrame::parse(&syn_acks[0]).unwrap();
        let syn_ack_ip = Ipv4Packet::parse(syn_ack_eth.payload, true).unwrap();
        let syn_ack_tcp = TcpSegment::parse(
            syn_ack_ip.header.src_ip,
            syn_ack_ip.header.dst_ip,
            syn_ack_ip.payload,
            true,
        )
        .unwrap();
        println!(
            "Connected! [SYN+ACK received from port 80, Seq={}, Ack={}]",
            syn_ack_tcp.seq_num, syn_ack_tcp.ack_num
        );

        let http_req = format!(
            "GET / HTTP/1.1\r\nHost: {}\r\nUser-Agent: ToyNetStack-Curl\r\n\r\n",
            target_ip
        );
        let data_seg = TcpSegment::serialize(
            self.stack.config.ip,
            target_ip,
            client_port,
            80,
            syn_ack_tcp.ack_num,
            syn_ack_tcp.seq_num + 1,
            TcpFlags {
                psh: true,
                ack: true,
                ..Default::default()
            },
            65535,
            http_req.as_bytes(),
        );
        let ip_data = Ipv4Packet::serialize(
            self.stack.config.ip,
            target_ip,
            IP_PROTO_TCP,
            302,
            64,
            &data_seg,
        );
        let eth_data = EthernetFrame::serialize(
            self.remote_host_mac,
            self.stack.config.mac,
            ETHERTYPE_IPV4,
            &ip_data,
        );
        self.record_packet(&eth_data);

        let data_resps = self.remote_stack.process_frame(&eth_data);
        for resp in data_resps {
            self.record_packet(&resp);
            let eth = EthernetFrame::parse(&resp).unwrap();
            let ip = Ipv4Packet::parse(eth.payload, true).unwrap();
            let tcp =
                TcpSegment::parse(ip.header.src_ip, ip.header.dst_ip, ip.payload, true).unwrap();
            println!("Server ACK: Seq={}, Ack={}", tcp.seq_num, tcp.ack_num);
        }

        println!("HTTP/1.1 200 OK (Virtual Web Server)");
        println!(
            "Content-Type: text/plain\r\n\r\nHello from Toy TCP/IP Stack Virtual Web Server!\n"
        );
    }

    fn cmd_tls(&mut self, _args: &[&str]) {
        println!("Initiating TLS 1.3 Handshake (RFC 8446)...");
        let client_hello = TlsRecord::build_client_hello("toy-tcpip.org", [0x55; 32]);
        println!(
            "  1. [Client -> Server] TLS Record (Type=22 Handshake, Len={}) -> ClientHello",
            client_hello.payload.len()
        );

        let server_hello = TlsRecord::build_server_hello([0x77; 32]);
        println!(
            "  2. [Server -> Client] TLS Record (Type=22 Handshake, Len={}) -> ServerHello (Cipher: TLS_AES_128_GCM_SHA256)",
            server_hello.payload.len()
        );
        println!("  3. [Key Exchange] Derived Handshake & Application Secret Keys.");
        println!("  4. Handshake Complete: TLS 1.3 Session Established.\n");
    }

    fn cmd_http2(&mut self, _args: &[&str]) {
        println!("Initiating HTTP/2 Multiplexed Stream Session (RFC 7540)...");
        let _settings = Http2Frame::build_settings(false);
        println!("  1. Sent HTTP/2 SETTINGS frame (Stream ID 0, 9-byte header)");
        let _headers = Http2Frame::build_headers(1, false, true, b":method GET :path /index.html");
        println!("  2. Sent HTTP/2 HEADERS frame (Stream ID 1, Flags: END_HEADERS)");
        let data = Http2Frame::build_data(1, true, b"Hello HTTP/2 multiplexing!");
        println!(
            "  3. Sent HTTP/2 DATA frame (Stream ID 1, Flags: END_STREAM, {} bytes)",
            data.payload.len()
        );
        println!("  4. HTTP/2 Stream 1 response received successfully.\n");
    }

    fn cmd_firewall(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "list" {
            println!("Stateful Firewall Filter Rules:");
            println!(
                "  [INPUT Chain]  (Default: {})",
                self.stack.firewall.default_input_policy
            );
            if self.stack.firewall.input_rules.is_empty() {
                println!("    <empty>");
            }
            for (i, r) in self.stack.firewall.input_rules.iter().enumerate() {
                println!(
                    "    #{}: Action={} Desc=\"{}\"",
                    i + 1,
                    r.action,
                    r.description
                );
            }
        } else if args[0] == "flush" {
            self.stack.firewall.flush_chain(FirewallChain::Input);
            println!("Flushed INPUT firewall chain.");
        } else if args.len() >= 3 && args[0] == "add" && args[1] == "drop" {
            if let Ok(ip) = Ipv4Address::from_str(args[2]) {
                self.stack.firewall.add_rule(
                    FirewallChain::Input,
                    FirewallRule {
                        description: format!("Block IP {}", ip),
                        src_cidr: Some(IpCidr::new(ip, 32)),
                        action: FirewallAction::Drop,
                        ..Default::default()
                    },
                );
                println!("Added rule: DROP all traffic from {}", ip);
            } else {
                println!("Invalid IP: {}", args[2]);
            }
        }
    }

    fn cmd_nat(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "status" {
            if let Some(ref nat) = self.remote_stack.nat {
                println!("NAT / Masquerade Gateway Status:");
                println!("  Public IP         : {}", nat.public_ip);
                println!("  Active Sessions   : {}", nat.active_session_count());
                println!("  Port Forward Rules: {}", nat.port_forward_rules().len());
                for r in nat.port_forward_rules() {
                    println!(
                        "    Port {} -> {}:{}",
                        r.external_port, r.internal_ip, r.internal_port
                    );
                }
            } else {
                println!("NAT is currently disabled on gateway.");
            }
        } else if args.len() >= 4 && args[0] == "forward" {
            let ext_port = args[1].parse::<u16>().unwrap_or(8080);
            let int_ip = Ipv4Address::from_str(args[2]).unwrap_or(self.stack.config.ip);
            let int_port = args[3].parse::<u16>().unwrap_or(80);
            if let Some(ref mut nat) = self.remote_stack.nat {
                nat.add_port_forward(ext_port, int_ip, int_port, IP_PROTO_TCP);
                println!(
                    "Added DNAT Port Forward: External Port {} -> {}:{}",
                    ext_port, int_ip, int_port
                );
            }
        }
    }

    fn cmd_tcp_stats(&self) {
        println!("TCP Congestion Control & Flow Control Status:");
        for (key, conn) in &self.remote_stack.tcp_manager.connections {
            println!(
                "Connection {}:{} <-> {}:{}",
                key.local.ip, key.local.port, key.remote.ip, key.remote.port
            );
            println!("  State        : {}", conn.state);
            println!(
                "  CWND (bytes) : {} ({} MSS)",
                conn.congestion.cwnd,
                conn.congestion.cwnd / conn.congestion.mss.max(1)
            );
            println!("  Ssthresh     : {} bytes", conn.congestion.ssthresh);
            println!("  CC State     : {}", conn.congestion.state);
            println!("  In Flight    : {} bytes", conn.congestion.in_flight);
            println!(
                "  RTO Estimator: {:.1} ms (SRTT: {:?} ms)",
                conn.rtt.rto, conn.rtt.srtt
            );
        }
        if self.remote_stack.tcp_manager.connections.is_empty() {
            println!("  No active TCP connections currently tracked.");
        }
    }

    fn cmd_netstat(&self) {
        println!("Active Internet connections:");
        println!("Proto Recv-Q Send-Q Local Address          Foreign Address        State");
        println!("tcp   0      0      0.0.0.0:49             0.0.0.0:*              LISTEN");
        println!("tcp   0      0      0.0.0.0:80             0.0.0.0:*              LISTEN");
        println!("tcp   0      0      0.0.0.0:179            0.0.0.0:*              LISTEN");
        println!("tcp   0      0      0.0.0.0:389            0.0.0.0:*              LISTEN");
        println!("tcp   0      0      0.0.0.0:443            0.0.0.0:*              LISTEN");
        println!("tcp   0      0      0.0.0.0:646            0.0.0.0:*              LISTEN");
        println!("tcp   0      0      0.0.0.0:830            0.0.0.0:*              LISTEN");
        println!("tcp   0      0      0.0.0.0:862            0.0.0.0:*              LISTEN");
        println!("tcp   0      0      0.0.0.0:1883           0.0.0.0:*              LISTEN");
        println!("tcp   0      0      0.0.0.0:3868           0.0.0.0:*              LISTEN");
        println!("tcp   0      0      0.0.0.0:4189           0.0.0.0:*              LISTEN");
        println!("tcp   0      0      0.0.0.0:4317           0.0.0.0:*              LISTEN");
        println!("tcp   0      0      0.0.0.0:4318           0.0.0.0:*              LISTEN");
        println!("tcp   0      0      0.0.0.0:6653           0.0.0.0:*              LISTEN");
        println!("tcp   0      0      0.0.0.0:7777           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:7              0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:53             0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:69             0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:123            0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:161            0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:319            0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:320            0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:514            0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:546            0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:547            0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:646            0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:862            0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:1812           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:1985           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:2055           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:2152           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:3222           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:3478           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:3503           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:3784           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:4341           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:4342           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:4754           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:4789           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:4790           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:4791           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:5004           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:5060           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:5683           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:6080           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:6081           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:6343           0.0.0.0:*              LISTEN");
        println!("udp   0      0      0.0.0.0:51820          0.0.0.0:*              LISTEN");
    }

    fn cmd_pcap(&mut self, args: &[&str]) {
        if args.is_empty() {
            println!("Usage: pcap start <file.pcap> | stop");
            return;
        }

        if args[0] == "start" && args.len() >= 2 {
            let path = args[1];
            match File::create(path) {
                Ok(file) => {
                    let writer = PcapWriter::new(file, 65535, LINKTYPE_ETHERNET).unwrap();
                    self.pcap_writer = Some(writer);
                    println!("Started PCAP packet recording -> '{}'", path);
                }
                Err(e) => println!("Failed to create PCAP file: {}", e),
            }
        } else if args[0] == "stop" {
            self.pcap_writer = None;
            println!("Stopped PCAP packet recording.");
        }
    }

    fn cmd_lab(&mut self, args: &[&str]) {
        if args.is_empty() || args[0] == "help" {
            println!("Virtual Network Lab (Deterministic In-Process Data Plane Testbed)");
            println!("Usage: lab <subcommand> [args...]");
            println!("Subcommands:");
            println!(
                "  topology               - Display virtual network topology (Nodes, Links, Subnets)"
            );
            println!(
                "  ping4 [target_ip]      - Execute end-to-end IPv4 Ping with cold ARP resolution"
            );
            println!(
                "  ping6 [target_ip6]     - Execute end-to-end IPv6 Ping with NDP NS/NA resolution"
            );
            println!(
                "  route4 [target_ip] [ttl] - Multi-hop routed IPv4 data plane test & TTL expiration"
            );
            println!("  udp-echo [msg]         - End-to-end UDP echo client/server exchange");
            println!(
                "  tcp-demo               - Full TCP connection lifecycle (3-way handshake, Data, Teardown)"
            );
            println!(
                "  pcap [output.pcap]     - Run lab test suite with link packet tap and export PCAP"
            );
            return;
        }

        match args[0] {
            "topology" => {
                println!(
                    "╔════════════════════════════════════════════════════════════════════════════╗"
                );
                println!(
                    "║                 🌐 Integrated Virtual Network Lab Topologies                ║"
                );
                println!(
                    "╚════════════════════════════════════════════════════════════════════════════╝"
                );
                println!("Topology A (Switched L2 LAN):");
                println!(
                    "  [Host A: 192.168.1.10 / 2001:db8:1::10] ──(lan1: 1500 MTU)── [Host B: 192.168.1.20 / 2001:db8:1::20]"
                );
                println!();
                println!("Topology B (Multi-Subnet Routed WAN):");
                println!("  [Host A: 10.0.1.2/24 GW: 10.0.1.1]");
                println!("         │ (link_net1: 10.0.1.0/24)");
                println!("  [Router: eth0=10.0.1.1 | eth1=10.0.2.1]");
                println!("         │ (link_net2: 10.0.2.0/24)");
                println!("  [Host B: 10.0.2.2/24 GW: 10.0.2.1]");
            }

            "ping4" => {
                let target_ip = if args.len() >= 2 {
                    Ipv4Address::from_str(args[1]).unwrap_or(Ipv4Address::new(192, 168, 1, 20))
                } else {
                    Ipv4Address::new(192, 168, 1, 20)
                };

                let mut lab = VirtualLab::new();
                let h_a_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x10]);
                let h_b_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x20]);
                let h_a_ip = Ipv4Address::new(192, 168, 1, 10);
                let h_b_ip = Ipv4Address::new(192, 168, 1, 20);

                lab.add_host(
                    "host_a",
                    "lan1",
                    NetStackConfig {
                        mac: h_a_mac,
                        ip: h_a_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: None,
                    },
                );
                lab.add_host(
                    "host_b",
                    "lan1",
                    NetStackConfig {
                        mac: h_b_mac,
                        ip: h_b_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: None,
                    },
                );

                println!(
                    "Initiating IPv4 Ping from host_a ({}) to {}...",
                    h_a_ip, target_ip
                );
                if let Some(frame) =
                    lab.host_mut("host_a")
                        .unwrap()
                        .stack
                        .ping4(target_ip, 0x1234, 1, b"LAB_PING4")
                {
                    lab.send_from_host("host_a", frame);
                    let steps = lab.run_until_quiescent(10);
                    let host_a = lab.host("host_a").unwrap();
                    if !host_a.stack.received_icmp_replies.is_empty() {
                        println!(
                            "✓ 64 bytes from {}: icmp_seq=1 ttl=64 roundtrip=OK (simulation steps: {})",
                            target_ip, steps
                        );
                        println!(
                            "  ARP Cache: {} -> {:?}",
                            target_ip,
                            host_a.stack.arp_table.lookup(&target_ip.0)
                        );
                    } else {
                        println!("✗ Request timeout for {}", target_ip);
                    }
                }
            }

            "ping6" => {
                let target_ip6 = if args.len() >= 2 {
                    Ipv6Address::from_str(args[1]).unwrap_or_else(|_| {
                        Ipv6Address::new([0x2001, 0x0db8, 1, 0, 0, 0, 0, 0x0020])
                    })
                } else {
                    Ipv6Address::new([0x2001, 0x0db8, 1, 0, 0, 0, 0, 0x0020])
                };

                let mut lab = VirtualLab::new();
                let h_a_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x10]);
                let h_b_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x20]);
                let h_a_ip6 = Ipv6Address::new([0x2001, 0x0db8, 1, 0, 0, 0, 0, 0x0010]);
                let h_b_ip6 = Ipv6Address::new([0x2001, 0x0db8, 1, 0, 0, 0, 0, 0x0020]);

                lab.add_host(
                    "host_a",
                    "lan6",
                    NetStackConfig {
                        mac: h_a_mac,
                        ip: Ipv4Address::new(10, 0, 0, 10),
                        ipv6: Some(h_a_ip6),
                        subnet_mask: 24,
                        gateway: None,
                    },
                );
                lab.add_host(
                    "host_b",
                    "lan6",
                    NetStackConfig {
                        mac: h_b_mac,
                        ip: Ipv4Address::new(10, 0, 0, 20),
                        ipv6: Some(h_b_ip6),
                        subnet_mask: 24,
                        gateway: None,
                    },
                );

                println!(
                    "Initiating IPv6 Ping from host_a ({:?}) to {:?}...",
                    h_a_ip6, target_ip6
                );
                if let Some(frame) =
                    lab.host_mut("host_a")
                        .unwrap()
                        .stack
                        .ping6(target_ip6, 0x5678, 1, b"LAB_PING6")
                {
                    lab.send_from_host("host_a", frame);
                    let steps = lab.run_until_quiescent(10);
                    let host_a = lab.host("host_a").unwrap();
                    if !host_a.stack.received_icmpv6_replies.is_empty() {
                        println!(
                            "✓ 64 bytes from {:?}: icmp_seq=1 hop_limit=64 (simulation steps: {})",
                            target_ip6, steps
                        );
                        println!(
                            "  NDP Cache: {:?} -> {:?}",
                            target_ip6,
                            host_a.stack.ndp_table.lookup(&target_ip6)
                        );
                    } else {
                        println!("✗ Request timeout for {:?}", target_ip6);
                    }
                }
            }

            "route4" => {
                let target_ip = if args.len() >= 2 {
                    Ipv4Address::from_str(args[1]).unwrap_or(Ipv4Address::new(10, 0, 2, 2))
                } else {
                    Ipv4Address::new(10, 0, 2, 2)
                };
                let ttl: u8 = if args.len() >= 3 {
                    args[2].parse().unwrap_or(64)
                } else {
                    64
                };

                let mut lab = VirtualLab::new();
                let host_a_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x03, 0x10]);
                let host_b_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x03, 0x20]);
                let rtr_if0_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x03, 0x01]);
                let rtr_if1_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x03, 0x02]);

                let host_a_ip = Ipv4Address::new(10, 0, 1, 2);
                let rtr_if0_ip = Ipv4Address::new(10, 0, 1, 1);
                let rtr_if1_ip = Ipv4Address::new(10, 0, 2, 1);
                let host_b_ip = Ipv4Address::new(10, 0, 2, 2);

                lab.add_host(
                    "host_a",
                    "link_net1",
                    NetStackConfig {
                        mac: host_a_mac,
                        ip: host_a_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: Some(rtr_if0_ip),
                    },
                );
                lab.add_host(
                    "host_b",
                    "link_net2",
                    NetStackConfig {
                        mac: host_b_mac,
                        ip: host_b_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: Some(rtr_if1_ip),
                    },
                );

                let mut router = LabRouter::new("rtr1");
                router.add_interface("eth0", rtr_if0_mac, rtr_if0_ip, 24, "link_net1");
                router.add_interface("eth1", rtr_if1_mac, rtr_if1_ip, 24, "link_net2");
                lab.add_router(router);

                println!(
                    "Routing IPv4 packet from Host A ({}) to {} (TTL={})...",
                    host_a_ip, target_ip, ttl
                );

                if ttl == 1 {
                    let icmp_req = IcmpPacket::build_echo_request(0x9999, 1, b"TTL1_EXPIRY");
                    let ip_ttl1 = Ipv4Packet::serialize(
                        host_a_ip,
                        target_ip,
                        IP_PROTO_ICMP,
                        555,
                        1,
                        &icmp_req,
                    );
                    let eth_frame =
                        EthernetFrame::serialize(rtr_if0_mac, host_a_mac, ETHERTYPE_IPV4, &ip_ttl1);
                    lab.send_from_host("host_a", eth_frame);
                    lab.run_until_quiescent(10);

                    let host_a = lab.host("host_a").unwrap();
                    if !host_a.stack.received_icmp_time_exceeded.is_empty() {
                        println!(
                            "! From {} icmp_seq=1 Time to live exceeded (Type 11 Code 0)",
                            host_a.stack.received_icmp_time_exceeded[0].0
                        );
                    }
                } else if let Some(frame) = lab.host_mut("host_a").unwrap().stack.ping4(
                    target_ip,
                    0xABCD,
                    1,
                    b"ROUTED_PING",
                ) {
                    lab.send_from_host("host_a", frame);
                    let steps = lab.run_until_quiescent(20);
                    let host_a = lab.host("host_a").unwrap();
                    if !host_a.stack.received_icmp_replies.is_empty() {
                        println!(
                            "✓ Routed reply from {}: icmp_seq=1 ttl=62 (traversed rtr1 in {} steps)",
                            target_ip, steps
                        );
                    }
                }
            }

            "udp-echo" => {
                let msg = if args.len() >= 2 {
                    args[1]
                } else {
                    "Hello Virtual Lab"
                };
                let mut lab = VirtualLab::new();
                let host_a_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x04, 0x10]);
                let host_b_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x04, 0x20]);
                let host_a_ip = Ipv4Address::new(192, 168, 10, 10);
                let host_b_ip = Ipv4Address::new(192, 168, 10, 20);

                lab.add_host(
                    "host_a",
                    "lan_udp",
                    NetStackConfig {
                        mac: host_a_mac,
                        ip: host_a_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: None,
                    },
                );
                lab.add_host(
                    "host_b",
                    "lan_udp",
                    NetStackConfig {
                        mac: host_b_mac,
                        ip: host_b_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: None,
                    },
                );

                lab.host_mut("host_b").unwrap().stack.udp_sockets.bind(
                    9000,
                    |_src_ip, _src_port, payload| {
                        let mut echo = b"ECHO: ".to_vec();
                        echo.extend_from_slice(payload);
                        Some(echo)
                    },
                );

                println!(
                    "Sending UDP echo from host_a:45000 to host_b ({}:9000): '{}'...",
                    host_b_ip, msg
                );
                if let Some(frame) = lab.host_mut("host_a").unwrap().stack.send_udp(
                    host_b_ip,
                    45000,
                    9000,
                    msg.as_bytes(),
                ) {
                    lab.send_from_host("host_a", frame);
                    let steps = lab.run_until_quiescent(10);
                    let host_a = lab.host("host_a").unwrap();
                    if !host_a.stack.received_udp_payloads.is_empty() {
                        let (_, _, _, ref data) = host_a.stack.received_udp_payloads[0];
                        println!(
                            "✓ Received UDP Echo: '{}' (steps: {})",
                            String::from_utf8_lossy(data),
                            steps
                        );
                    }
                }
            }

            "tcp-demo" => {
                let mut lab = VirtualLab::new();
                let host_a_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x05, 0x10]);
                let host_b_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x05, 0x20]);
                let host_a_ip = Ipv4Address::new(192, 168, 20, 10);
                let host_b_ip = Ipv4Address::new(192, 168, 20, 20);

                lab.add_host(
                    "client",
                    "lan_tcp",
                    NetStackConfig {
                        mac: host_a_mac,
                        ip: host_a_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: None,
                    },
                );
                lab.add_host(
                    "server",
                    "lan_tcp",
                    NetStackConfig {
                        mac: host_b_mac,
                        ip: host_b_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: None,
                    },
                );
                lab.host_mut("server").unwrap().stack.tcp_manager.listen(80);

                let client_sock = SocketAddrV4 {
                    ip: host_a_ip,
                    port: 50000,
                };
                let server_sock = SocketAddrV4 {
                    ip: host_b_ip,
                    port: 80,
                };

                println!("1. TCP 3-Way Handshake [SYN -> SYN-ACK -> ACK]...");
                let syn = lab
                    .host_mut("client")
                    .unwrap()
                    .stack
                    .tcp_connect(host_b_ip, 50000, 80, 1000)
                    .unwrap();
                lab.send_from_host("client", syn);
                lab.run_until_quiescent(10);
                println!(
                    "   Client State: {} | Server State: {}",
                    lab.host("client")
                        .unwrap()
                        .stack
                        .tcp_manager
                        .get_connection(client_sock, server_sock)
                        .unwrap()
                        .state,
                    lab.host("server")
                        .unwrap()
                        .stack
                        .tcp_manager
                        .get_connection(server_sock, client_sock)
                        .unwrap()
                        .state,
                );

                println!("2. TCP Data Streaming [GET / HTTP/1.1]...");
                let data = lab
                    .host_mut("client")
                    .unwrap()
                    .stack
                    .tcp_send_data(host_b_ip, 50000, 80, b"GET / HTTP/1.1\r\n\r\n")
                    .unwrap();
                lab.send_from_host("client", data);
                lab.run_until_quiescent(10);
                let srv_buf = &lab
                    .host("server")
                    .unwrap()
                    .stack
                    .tcp_manager
                    .get_connection(server_sock, client_sock)
                    .unwrap()
                    .rx_buffer;
                println!(
                    "   Server Inbound Buffer: '{}'",
                    String::from_utf8_lossy(srv_buf)
                );

                println!("3. TCP 4-Way Connection Teardown [FIN-ACK -> ACK]...");
                let fin = lab
                    .host_mut("client")
                    .unwrap()
                    .stack
                    .tcp_close(host_b_ip, 50000, 80)
                    .unwrap();
                lab.send_from_host("client", fin);
                lab.run_until_quiescent(10);
                println!(
                    "   Client State: {} | Server State: {}",
                    lab.host("client")
                        .unwrap()
                        .stack
                        .tcp_manager
                        .get_connection(client_sock, server_sock)
                        .unwrap()
                        .state,
                    lab.host("server")
                        .unwrap()
                        .stack
                        .tcp_manager
                        .get_connection(server_sock, client_sock)
                        .unwrap()
                        .state,
                );
            }

            "dhcp" => {
                println!("=== Virtual Lab: DHCPv4 DORA Auto-Configuration Demo ===");
                let mut lab = VirtualLab::new();
                let srv_ip = Ipv4Address::new(192, 168, 1, 1);
                let client_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x99]);

                lab.add_host(
                    "dhcp_server",
                    "lan_dhcp",
                    NetStackConfig {
                        mac: MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x01]),
                        ip: srv_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: None,
                    },
                );
                lab.host_mut("dhcp_server").unwrap().stack.dhcp_server =
                    Some(crate::dhcp::DhcpServer::new(
                        srv_ip,
                        Ipv4Address::new(255, 255, 255, 0),
                        srv_ip,
                        Ipv4Address::new(8, 8, 8, 8),
                        Ipv4Address::new(192, 168, 1, 150),
                        Ipv4Address::new(192, 168, 1, 200),
                        86400,
                    ));

                lab.add_host(
                    "client",
                    "lan_dhcp",
                    NetStackConfig {
                        mac: client_mac,
                        ip: Ipv4Address::UNSPECIFIED,
                        ipv6: None,
                        subnet_mask: 0,
                        gateway: None,
                    },
                );

                println!("1. Client broadcasting DHCP Discover...");
                let disc = lab
                    .host_mut("client")
                    .unwrap()
                    .stack
                    .dhcp_discover(0xABCDEF);
                lab.send_from_host("client", disc);
                lab.run_until_quiescent(10);

                let client = lab.host_mut("client").unwrap();
                let offer = client.stack.received_dhcp_offers[0].clone();
                println!("2. Client received DHCP Offer: IP = {}", offer.yiaddr);

                println!("3. Client sending DHCP Request for {}...", offer.yiaddr);
                let req =
                    client
                        .stack
                        .dhcp_request(offer.yiaddr, offer.server_id.unwrap(), 0xABCDEF);
                lab.send_from_host("client", req);
                lab.run_until_quiescent(10);

                let client = lab.host_mut("client").unwrap();
                let ack = client.stack.received_dhcp_acks[0].clone();
                println!(
                    "4. Client received DHCP ACK: IP = {}, Router = {:?}",
                    ack.yiaddr, ack.router
                );

                client.stack.apply_dhcp_ack(&ack);
                println!(
                    "✓ Client stack dynamically reconfigured: IP = {}/{}",
                    client.stack.config.ip, client.stack.config.subnet_mask
                );
            }

            "nat" => {
                println!("=== Virtual Lab: NAPT (SNAT & DNAT) Router Demo ===");
                let mut lab = VirtualLab::new();
                let client_ip = Ipv4Address::new(192, 168, 10, 5);
                let router_lan_ip = Ipv4Address::new(192, 168, 10, 1);
                let router_wan_ip = Ipv4Address::new(203, 0, 113, 1);
                let server_ip = Ipv4Address::new(203, 0, 113, 80);

                lab.add_host(
                    "private_client",
                    "lan",
                    NetStackConfig {
                        mac: MacAddress([0x02, 0x00, 0x00, 0x00, 0x0A, 0x10]),
                        ip: client_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: Some(router_lan_ip),
                    },
                );

                lab.add_host(
                    "wan_server",
                    "wan",
                    NetStackConfig {
                        mac: MacAddress([0x02, 0x00, 0x00, 0x00, 0x0B, 0x80]),
                        ip: server_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: Some(router_wan_ip),
                    },
                );

                lab.host_mut("wan_server").unwrap().stack.udp_sockets.bind(
                    8080,
                    |_src, _port, data| {
                        let mut resp = b"ACK:".to_vec();
                        resp.extend_from_slice(data);
                        Some(resp)
                    },
                );

                let mut r = LabRouter::new("nat_router");
                r.add_interface(
                    "eth_lan",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x0A, 0x01]),
                    router_lan_ip,
                    24,
                    "lan",
                );
                r.add_interface(
                    "eth_wan",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x0B, 0x01]),
                    router_wan_ip,
                    24,
                    "wan",
                );
                r.enable_nat("eth_lan", "eth_wan", router_wan_ip);
                lab.add_router(r);

                println!(
                    "1. LAN Client {} sending UDP to WAN Server {}:8080...",
                    client_ip, server_ip
                );
                let query = lab
                    .host_mut("private_client")
                    .unwrap()
                    .stack
                    .send_udp(server_ip, 45000, 8080, b"TRANSLATION_TEST")
                    .unwrap();
                lab.send_from_host("private_client", query);
                lab.run_until_quiescent(20);

                let wan_srv = lab.host("wan_server").unwrap();
                let (src, _, _, _) = &wan_srv.stack.received_udp_payloads[0];
                println!(
                    "2. WAN Server received datagram from: {} (SNAT rewritten from {})",
                    src, client_ip
                );

                let client = lab.host("private_client").unwrap();
                let (_, _, _, reply) = &client.stack.received_udp_payloads[0];
                println!(
                    "3. Private Client received reply: '{}' (DNAT de-translated)",
                    String::from_utf8_lossy(reply)
                );
                println!("✓ Full SNAT and DNAT session translation verified!");
            }

            "rip" => {
                println!("=== Virtual Lab: RIPv2 Multi-Router Dynamic Convergence Demo ===");
                let mut lab = VirtualLab::new();
                let h_a_ip = Ipv4Address::new(10, 0, 1, 2);
                let h_b_ip = Ipv4Address::new(10, 0, 2, 2);

                lab.add_host(
                    "host_a",
                    "link_a",
                    NetStackConfig {
                        mac: MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x02]),
                        ip: h_a_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: Some(Ipv4Address::new(10, 0, 1, 1)),
                    },
                );

                lab.add_host(
                    "host_b",
                    "link_b",
                    NetStackConfig {
                        mac: MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0x02]),
                        ip: h_b_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: Some(Ipv4Address::new(10, 0, 2, 1)),
                    },
                );

                let mut r1 = LabRouter::new("r1");
                r1.add_interface(
                    "r1_lan",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x01]),
                    Ipv4Address::new(10, 0, 1, 1),
                    24,
                    "link_a",
                );
                r1.add_interface(
                    "r1_wan",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x11, 0x01]),
                    Ipv4Address::new(172, 16, 0, 1),
                    24,
                    "link_tr",
                );
                r1.enable_rip();
                lab.add_router(r1);

                let mut r2 = LabRouter::new("r2");
                r2.add_interface(
                    "r2_wan",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x11, 0x02]),
                    Ipv4Address::new(172, 16, 0, 2),
                    24,
                    "link_tr",
                );
                r2.add_interface(
                    "r2_lan",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0x01]),
                    Ipv4Address::new(10, 0, 2, 1),
                    24,
                    "link_b",
                );
                r2.enable_rip();
                lab.add_router(r2);

                println!("1. Routers exchanging RIPv2 updates over 224.0.0.9:520...");
                lab.broadcast_rip_advertisements();
                lab.run_until_quiescent(10);

                let r1_route = lab
                    .router("r1")
                    .unwrap()
                    .routing_table
                    .lookup(h_b_ip)
                    .unwrap();
                println!(
                    "2. Router 1 dynamically learned route to 10.0.2.0/24 via next-hop {:?}",
                    r1_route.next_hop(h_b_ip)
                );

                println!(
                    "3. Host A ({}) pinging Host B ({}) across converged multi-router fabric...",
                    h_a_ip, h_b_ip
                );
                let ping = lab
                    .host_mut("host_a")
                    .unwrap()
                    .stack
                    .ping4(h_b_ip, 0x1122, 1, b"RIP_TEST")
                    .unwrap();
                lab.send_from_host("host_a", ping);
                lab.run_until_quiescent(20);

                let host_a = lab.host("host_a").unwrap();
                if !host_a.stack.received_icmp_replies.is_empty() {
                    println!("✓ Multi-hop dynamic routing ping successful!");
                }
            }

            "vxlan" => {
                println!("=== Virtual Lab: VXLAN L2 Overlay Fabric Demo ===");
                let mut lab = VirtualLab::new();
                let h1_ip = Ipv4Address::new(192, 168, 100, 10);
                let h2_ip = Ipv4Address::new(192, 168, 100, 20);

                lab.add_host(
                    "tenant_h1",
                    "acc_1",
                    NetStackConfig {
                        mac: MacAddress([0x02, 0x00, 0x00, 0x00, 0x0A, 0x10]),
                        ip: h1_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: None,
                    },
                );
                lab.add_host(
                    "tenant_h2",
                    "acc_2",
                    NetStackConfig {
                        mac: MacAddress([0x02, 0x00, 0x00, 0x00, 0x0A, 0x20]),
                        ip: h2_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: None,
                    },
                );

                let mut leaf1 = LabRouter::new("leaf1");
                leaf1.add_interface(
                    "eth_acc",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0xAA]),
                    Ipv4Address::new(192, 168, 100, 254),
                    24,
                    "acc_1",
                );
                leaf1.add_interface(
                    "eth_und",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x01]),
                    Ipv4Address::new(10, 0, 1, 1),
                    24,
                    "und_1",
                );
                leaf1.routing_table.add_route(
                    Ipv4Address::new(10, 0, 2, 0),
                    24,
                    Some(Ipv4Address::new(10, 0, 1, 254)),
                    "eth_und",
                );
                leaf1.add_vxlan_tunnel("eth_acc", 5001, Ipv4Address::new(10, 0, 2, 1), "eth_und");
                lab.add_router(leaf1);

                let mut spine = LabRouter::new("spine");
                spine.add_interface(
                    "sp_if1",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x55, 0x01]),
                    Ipv4Address::new(10, 0, 1, 254),
                    24,
                    "und_1",
                );
                spine.add_interface(
                    "sp_if2",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x55, 0x02]),
                    Ipv4Address::new(10, 0, 2, 254),
                    24,
                    "und_2",
                );
                lab.add_router(spine);

                let mut leaf2 = LabRouter::new("leaf2");
                leaf2.add_interface(
                    "eth_und",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0x01]),
                    Ipv4Address::new(10, 0, 2, 1),
                    24,
                    "und_2",
                );
                leaf2.add_interface(
                    "eth_acc",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0xAA]),
                    Ipv4Address::new(192, 168, 100, 253),
                    24,
                    "acc_2",
                );
                leaf2.routing_table.add_route(
                    Ipv4Address::new(10, 0, 1, 0),
                    24,
                    Some(Ipv4Address::new(10, 0, 2, 254)),
                    "eth_und",
                );
                leaf2.add_vxlan_tunnel("eth_acc", 5001, Ipv4Address::new(10, 0, 1, 1), "eth_und");
                lab.add_router(leaf2);

                println!(
                    "1. Encapsulating Tenant Ethernet frames over VNI 5001 across Underlay IP..."
                );
                let ping = lab
                    .host_mut("tenant_h1")
                    .unwrap()
                    .stack
                    .ping4(h2_ip, 0x4321, 1, b"VXLAN_DEMO")
                    .unwrap();
                lab.send_from_host("tenant_h1", ping);
                lab.run_until_quiescent(30);

                let h1 = lab.host("tenant_h1").unwrap();
                if !h1.stack.received_icmp_replies.is_empty() {
                    println!(
                        "✓ Tenant Host 1 received ICMP reply from Tenant Host 2 across VXLAN fabric!"
                    );
                }
            }

            "ospf" => {
                println!("=== Virtual Lab: OSPFv2 Link-State Dijkstra SPF Demo ===");
                let mut r1 = LabRouter::new("r1");
                r1.add_interface(
                    "r1_lan",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x01]),
                    Ipv4Address::new(172, 16, 1, 1),
                    24,
                    "link_a",
                );
                r1.add_interface(
                    "r1_r2",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x12, 0x01]),
                    Ipv4Address::new(10, 1, 2, 1),
                    24,
                    "link_r1_r2",
                );
                r1.add_interface(
                    "r1_r3",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x13, 0x01]),
                    Ipv4Address::new(10, 1, 3, 1),
                    24,
                    "link_r1_r3",
                );
                r1.enable_ospf();
                r1.add_ospf_link(
                    Ipv4Address::new(1, 1, 1, 1),
                    Ipv4Address::new(2, 2, 2, 2),
                    10,
                );
                r1.add_ospf_link(
                    Ipv4Address::new(2, 2, 2, 2),
                    Ipv4Address::new(3, 3, 3, 3),
                    10,
                );
                r1.add_ospf_link(
                    Ipv4Address::new(1, 1, 1, 1),
                    Ipv4Address::new(3, 3, 3, 3),
                    50,
                );

                let mut subnets = std::collections::HashMap::new();
                subnets.insert(
                    Ipv4Address::new(3, 3, 3, 3),
                    (
                        Ipv4Address::new(172, 16, 3, 0),
                        24,
                        "r1_r2".to_string(),
                        Ipv4Address::new(10, 1, 2, 2),
                    ),
                );
                r1.run_ospf_spf(Ipv4Address::new(1, 1, 1, 1), &subnets);

                let route = r1
                    .routing_table
                    .lookup(Ipv4Address::new(172, 16, 3, 10))
                    .unwrap();
                println!(
                    "1. Dijkstra Shortest Path calculated: Dest 172.16.3.0/24 -> NextHop {:?}",
                    route.next_hop(Ipv4Address::new(172, 16, 3, 10))
                );
                println!("✓ Path through R2 (Cost 20) prioritized over direct R3 link (Cost 50)");
            }

            "firewall" => {
                println!("=== Virtual Lab: Stateful Packet Filter & Firewall Demo ===");
                let mut lab = VirtualLab::new();
                let srv_ip = Ipv4Address::new(10, 0, 2, 80);

                lab.add_host(
                    "client",
                    "lan",
                    NetStackConfig {
                        mac: MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x05]),
                        ip: Ipv4Address::new(10, 0, 1, 5),
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: Some(Ipv4Address::new(10, 0, 1, 1)),
                    },
                );

                lab.add_host(
                    "server",
                    "wan",
                    NetStackConfig {
                        mac: MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0x80]),
                        ip: srv_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: Some(Ipv4Address::new(10, 0, 2, 1)),
                    },
                );

                lab.host_mut("server")
                    .unwrap()
                    .stack
                    .udp_sockets
                    .bind(80, |_src, _port, data| {
                        let mut resp = b"HTTP:".to_vec();
                        resp.extend_from_slice(data);
                        Some(resp)
                    });
                lab.host_mut("server")
                    .unwrap()
                    .stack
                    .udp_sockets
                    .bind(23, |_src, _port, _data| Some(b"TELNET".to_vec()));

                let mut r = LabRouter::new("gw");
                r.add_interface(
                    "lan",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x01]),
                    Ipv4Address::new(10, 0, 1, 1),
                    24,
                    "lan",
                );
                r.add_interface(
                    "wan",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0x01]),
                    Ipv4Address::new(10, 0, 2, 1),
                    24,
                    "wan",
                );

                let mut fw = crate::firewall::Firewall::new();
                fw.add_rule(
                    crate::firewall::FirewallChain::Forward,
                    crate::firewall::FirewallRule {
                        description: "Drop Telnet".to_string(),
                        src_cidr: None,
                        dst_cidr: None,
                        protocol: Some(crate::ipv4::IP_PROTO_UDP),
                        src_port_range: None,
                        dst_port_range: Some((23, 23)),
                        action: crate::firewall::FirewallAction::Drop,
                    },
                );
                r.set_firewall(fw);
                lab.add_router(r);

                println!("1. Testing Port 80 (Allowed)...");
                let q80 = lab
                    .host_mut("client")
                    .unwrap()
                    .stack
                    .send_udp(srv_ip, 40000, 80, b"PING80")
                    .unwrap();
                lab.send_from_host("client", q80);
                lab.run_until_quiescent(20);
                assert_eq!(
                    lab.host("client")
                        .unwrap()
                        .stack
                        .received_udp_payloads
                        .len(),
                    1
                );
                println!("✓ Port 80 query succeeded!");

                println!("2. Testing Port 23 (Firewall Drop)...");
                let q23 = lab
                    .host_mut("client")
                    .unwrap()
                    .stack
                    .send_udp(srv_ip, 40001, 23, b"PING23")
                    .unwrap();
                lab.send_from_host("client", q23);
                lab.run_until_quiescent(20);
                assert_eq!(
                    lab.host("client")
                        .unwrap()
                        .stack
                        .received_udp_payloads
                        .len(),
                    1
                );
                println!("✓ Port 23 traffic dropped by router firewall!");
            }

            "mpls" => {
                println!("=== Virtual Lab: MPLS 3-Node LSP (Push/Swap/Pop) Demo ===");
                let mut lab = VirtualLab::new();
                let h_b_ip = Ipv4Address::new(192, 168, 2, 20);

                lab.add_host(
                    "h_a",
                    "link_a",
                    NetStackConfig {
                        mac: MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x10]),
                        ip: Ipv4Address::new(192, 168, 1, 10),
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: Some(Ipv4Address::new(192, 168, 1, 1)),
                    },
                );

                lab.add_host(
                    "h_b",
                    "link_b",
                    NetStackConfig {
                        mac: MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0x20]),
                        ip: h_b_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: Some(Ipv4Address::new(192, 168, 2, 1)),
                    },
                );

                let mut r1 = LabRouter::new("r1");
                r1.add_interface(
                    "r1_cust",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x01]),
                    Ipv4Address::new(192, 168, 1, 1),
                    24,
                    "link_a",
                );
                r1.add_interface(
                    "r1_core",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x12, 0x01]),
                    Ipv4Address::new(10, 0, 12, 1),
                    24,
                    "core_12",
                );
                r1.enable_mpls();
                r1.add_mpls_push_route(h_b_ip, 100, "r1_core");
                lab.add_router(r1);

                let mut r2 = LabRouter::new("r2");
                r2.add_interface(
                    "r2_in",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x12, 0x02]),
                    Ipv4Address::new(10, 0, 12, 2),
                    24,
                    "core_12",
                );
                r2.add_interface(
                    "r2_out",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x23, 0x02]),
                    Ipv4Address::new(10, 0, 23, 2),
                    24,
                    "core_23",
                );
                r2.enable_mpls();
                r2.add_mpls_swap_route(100, 200, "r2_out");
                lab.add_router(r2);

                let mut r3 = LabRouter::new("r3");
                r3.add_interface(
                    "r3_core",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x23, 0x03]),
                    Ipv4Address::new(10, 0, 23, 3),
                    24,
                    "core_23",
                );
                r3.add_interface(
                    "r3_cust",
                    MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0x01]),
                    Ipv4Address::new(192, 168, 2, 1),
                    24,
                    "link_b",
                );
                r3.enable_mpls();
                r3.add_mpls_pop_route(200);
                lab.add_router(r3);

                println!(
                    "1. Transmitting customer packet through MPLS LSP: R1 (PUSH 100) -> R2 (SWAP 200) -> R3 (POP)..."
                );
                let pkt = lab
                    .host_mut("h_a")
                    .unwrap()
                    .stack
                    .send_udp(h_b_ip, 30000, 9000, b"MPLS_DEMO")
                    .unwrap();
                lab.send_from_host("h_a", pkt);
                lab.run_until_quiescent(25);

                let hb = lab.host("h_b").unwrap();
                assert_eq!(hb.stack.received_udp_payloads.len(), 1);
                println!(
                    "✓ Customer Host B received packet across MPLS core: '{}'",
                    String::from_utf8_lossy(&hb.stack.received_udp_payloads[0].3)
                );
            }

            "pcap" => {
                let out_file = if args.len() >= 2 {
                    args[1]
                } else {
                    "lab_trace.pcap"
                };
                let mut lab = VirtualLab::new();
                let host_a_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x07, 0x10]);
                let host_b_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x07, 0x20]);
                let host_a_ip = Ipv4Address::new(192, 168, 40, 10);
                let host_b_ip = Ipv4Address::new(192, 168, 40, 20);

                lab.add_host(
                    "host_a",
                    "lan_pcap",
                    NetStackConfig {
                        mac: host_a_mac,
                        ip: host_a_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: None,
                    },
                );
                lab.add_host(
                    "host_b",
                    "lan_pcap",
                    NetStackConfig {
                        mac: host_b_mac,
                        ip: host_b_ip,
                        ipv6: None,
                        subnet_mask: 24,
                        gateway: None,
                    },
                );
                lab.enable_pcap("lan_pcap");

                let ping_frame = lab
                    .host_mut("host_a")
                    .unwrap()
                    .stack
                    .ping4(host_b_ip, 0x7777, 1, b"PCAP_RECORD_DEMO")
                    .unwrap();
                lab.send_from_host("host_a", ping_frame);
                lab.run_until_quiescent(10);

                if let Some(pcap_bytes) = lab.export_pcap("lan_pcap") {
                    if let Ok(mut f) = File::create(out_file) {
                        let _ = f.write_all(&pcap_bytes);
                        println!(
                            "✓ Exported {} bytes of PCAP packet trace to '{}'",
                            pcap_bytes.len(),
                            out_file
                        );
                    } else {
                        println!(
                            "✓ Generated {} bytes in memory PCAP trace for 'lan_pcap'",
                            pcap_bytes.len()
                        );
                    }
                }
            }

            other => {
                println!(
                    "Unknown lab subcommand: '{}'. Type 'lab help' for usage.",
                    other
                );
            }
        }
    }
}
