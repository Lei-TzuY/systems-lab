use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::ngap_5g::{
    InitialUeMessage, NgSetupRequest, NgapNode, PduSessionResourceSetupRequest, PlmnId, Snssai,
    NGAP_PROC_INITIAL_UE_MESSAGE, NGAP_PROC_NG_SETUP, NGAP_PROC_PDU_SESSION_RESOURCE_SETUP,
    NGAP_SCTP_PORT,
};

#[test]
fn test_ngap_constants_and_elementary_procedures() {
    assert_eq!(NGAP_SCTP_PORT, 38412);
    assert_eq!(NGAP_PROC_NG_SETUP, 21);
    assert_eq!(NGAP_PROC_INITIAL_UE_MESSAGE, 15);
    assert_eq!(NGAP_PROC_PDU_SESSION_RESOURCE_SETUP, 29);
}

#[test]
fn test_ngap_end_to_end_signalling_flow() {
    let mut ngap = NgapNode::new();

    // 1. NG Setup
    let req = NgSetupRequest {
        global_gnb_id: 5001,
        gnb_name: "gNB-Hsinchu-01".to_string(),
        plmn: PlmnId { mcc: [4, 6, 6], mnc: [0, 1, 0] },
        tac: 0x1234,
        supported_slices: vec![
            Snssai { sst: 1, sd: None }, // eMBB
            Snssai { sst: 2, sd: Some([0x00, 0x01, 0x02]) }, // URLLC
        ],
    };
    let resp = ngap.handle_ng_setup(&req);
    assert!(ngap.is_amf_connected);
    assert_eq!(resp.amf_name, "amf-core-east-01");
    assert_eq!(resp.served_guami_list.len(), 1);

    // 2. Initial UE Message
    let ue_req = InitialUeMessage {
        ran_ue_ngap_id: 10,
        tac: 0x1234,
        nr_cgi: 0x500101,
        nas_pdu: vec![0x7E, 0x00, 0x41, 0x01],
    };
    let amf_ue_id = ngap.handle_initial_ue_message(&ue_req);
    assert_eq!(ngap.registered_ues_count, 1);

    // 3. PDU Session Resource Setup
    let pdu_req = PduSessionResourceSetupRequest {
        amf_ue_ngap_id: amf_ue_id,
        ran_ue_ngap_id: 10,
        pdu_session_id: 5,
        upf_transport_ip: Ipv4Address::new(192, 168, 50, 1),
        upf_gtpu_teid: 0xAABBCC,
    };
    let pdu_resp = ngap.handle_pdu_session_setup(&pdu_req, Ipv4Address::new(192, 168, 50, 200));
    assert_eq!(pdu_resp.pdu_session_id, 5);
    assert_eq!(pdu_resp.gnb_transport_ip, Ipv4Address::new(192, 168, 50, 200));
    assert_eq!(ngap.active_pdu_sessions_count, 1);
}
