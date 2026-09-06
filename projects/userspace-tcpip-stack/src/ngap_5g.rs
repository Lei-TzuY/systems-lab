//! 5G NGAP Signalling Protocol (3GPP TS 38.413 / N2 Interface over SCTP 38412).
//!
//! Implements 5G RAN (gNodeB) <-> 5G Core (AMF) control plane signaling,
//! including NG-Setup procedures, Initial UE Messages, and PDU Session Resource Setups.

use crate::ipv4::Ipv4Address;

pub const NGAP_SCTP_PORT: u16 = 38412;

pub const NGAP_PROC_NG_SETUP: u8 = 21;
pub const NGAP_PROC_INITIAL_UE_MESSAGE: u8 = 15;
pub const NGAP_PROC_PDU_SESSION_RESOURCE_SETUP: u8 = 29;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlmnId {
    pub mcc: [u8; 3], // e.g. [2, 0, 8]
    pub mnc: [u8; 3], // e.g. [9, 5, 0]
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Snssai {
    pub sst: u8,             // Slice/Service Type: 1 = eMBB, 2 = URLLC, 3 = MIoT
    pub sd: Option<[u8; 3]>, // Slice Differentiator
}

/// NG-Setup Request (gNodeB -> AMF)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NgSetupRequest {
    pub global_gnb_id: u32,
    pub gnb_name: String,
    pub plmn: PlmnId,
    pub tac: u32, // Tracking Area Code
    pub supported_slices: Vec<Snssai>,
}

/// NG-Setup Response (AMF -> gNodeB)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NgSetupResponse {
    pub amf_name: String,
    pub plmn: PlmnId,
    pub served_guami_list: Vec<u32>, // Globally Unique AMF IDs
}

/// Initial UE Message (gNodeB -> AMF) carrying NAS Registration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialUeMessage {
    pub ran_ue_ngap_id: u32,
    pub tac: u32,
    pub nr_cgi: u64, // NR Cell Global Identifier
    pub nas_pdu: Vec<u8>,
}

/// PDU Session Resource Setup Request (AMF -> gNodeB)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PduSessionResourceSetupRequest {
    pub amf_ue_ngap_id: u64,
    pub ran_ue_ngap_id: u32,
    pub pdu_session_id: u8,
    pub upf_transport_ip: Ipv4Address,
    pub upf_gtpu_teid: u32,
}

/// PDU Session Resource Setup Response (gNodeB -> AMF)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PduSessionResourceSetupResponse {
    pub amf_ue_ngap_id: u64,
    pub ran_ue_ngap_id: u32,
    pub pdu_session_id: u8,
    pub gnb_transport_ip: Ipv4Address,
    pub gnb_gtpu_teid: u32,
}

/// 5G N2 / NGAP Signalling Node (AMF / gNodeB Interface Controller)
#[derive(Debug, Clone, Default)]
pub struct NgapNode {
    pub is_amf_connected: bool,
    pub active_gnb_name: Option<String>,
    pub registered_ues_count: u32,
    pub active_pdu_sessions_count: u32,
}

impl NgapNode {
    pub fn new() -> Self {
        NgapNode {
            is_amf_connected: false,
            active_gnb_name: None,
            registered_ues_count: 0,
            active_pdu_sessions_count: 0,
        }
    }

    /// Handles NG-Setup Request from gNodeB
    pub fn handle_ng_setup(&mut self, req: &NgSetupRequest) -> NgSetupResponse {
        self.is_amf_connected = true;
        self.active_gnb_name = Some(req.gnb_name.clone());
        NgSetupResponse {
            amf_name: "amf-core-east-01".to_string(),
            plmn: req.plmn.clone(),
            served_guami_list: vec![0xCAFE01],
        }
    }

    /// Handles Initial UE Message (Registration Request)
    pub fn handle_initial_ue_message(&mut self, _msg: &InitialUeMessage) -> u64 {
        self.registered_ues_count += 1;
        // Allocate AMF UE NGAP ID
        0x500000000000 + self.registered_ues_count as u64
    }

    /// Handles PDU Session Resource Setup
    pub fn handle_pdu_session_setup(
        &mut self,
        req: &PduSessionResourceSetupRequest,
        gnb_ip: Ipv4Address,
    ) -> PduSessionResourceSetupResponse {
        self.active_pdu_sessions_count += 1;
        PduSessionResourceSetupResponse {
            amf_ue_ngap_id: req.amf_ue_ngap_id,
            ran_ue_ngap_id: req.ran_ue_ngap_id,
            pdu_session_id: req.pdu_session_id,
            gnb_transport_ip: gnb_ip,
            gnb_gtpu_teid: 0x2000 + self.active_pdu_sessions_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ngap_setup_and_ue_registration() {
        let mut ngap = NgapNode::new();
        let plmn = PlmnId {
            mcc: [2, 0, 8],
            mnc: [9, 5, 0],
        };

        // 1. NG Setup
        let setup_req = NgSetupRequest {
            global_gnb_id: 101,
            gnb_name: "gNodeB-Taipei-01".to_string(),
            plmn: plmn.clone(),
            tac: 0x0001,
            supported_slices: vec![Snssai { sst: 1, sd: None }],
        };
        let setup_resp = ngap.handle_ng_setup(&setup_req);
        assert!(ngap.is_amf_connected);
        assert_eq!(setup_resp.amf_name, "amf-core-east-01");

        // 2. Initial UE Message
        let ue_msg = InitialUeMessage {
            ran_ue_ngap_id: 1,
            tac: 0x0001,
            nr_cgi: 0x10101,
            nas_pdu: vec![0x7E, 0x00, 0x41], // 5GS Registration Request
        };
        let amf_ue_id = ngap.handle_initial_ue_message(&ue_msg);
        assert_eq!(ngap.registered_ues_count, 1);

        // 3. PDU Session Resource Setup
        let pdu_req = PduSessionResourceSetupRequest {
            amf_ue_ngap_id: amf_ue_id,
            ran_ue_ngap_id: 1,
            pdu_session_id: 1,
            upf_transport_ip: Ipv4Address::new(10, 100, 1, 50),
            upf_gtpu_teid: 0x10001,
        };
        let pdu_resp = ngap.handle_pdu_session_setup(&pdu_req, Ipv4Address::new(10, 100, 2, 10));
        assert_eq!(pdu_resp.gnb_transport_ip, Ipv4Address::new(10, 100, 2, 10));
        assert_eq!(ngap.active_pdu_sessions_count, 1);
    }
}
