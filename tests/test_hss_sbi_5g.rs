//! Integration tests for 3GPP TS 29.562 / TS 29.563 / TS 23.501 Rel-17
//! 5G HSS (Home Subscriber Server) SBI Service & IMS Core Interworking Engine.

use std::collections::HashMap;
use toy_tcpip::hss_sbi_5g::{
    AccessRestrictionData, ApplicationServer, DefaultHandling, HssError, HssSbiEngine,
    ImsRegistrationState, InitialFilterCriteria, ScscfRestorationInfo, ServicePointTrigger,
    ServiceProfile, SessionCase, TriggerCondition,
};
use toy_tcpip::sip::{SipMessage, SipMethod};

#[test]
fn test_hss_sbi_impi_impu_and_implicit_registration_set() {
    let mut hss = HssSbiEngine::new("hss01.ims.mnc095.mcc208.3gppnetwork.org");

    let supi = "208950000000001";
    let impi = "208950000000001@ims.mnc095.mcc208.3gppnetwork.org";

    // 1. Register IMPI subscription
    hss.register_impi_subscription(impi, supi, Some("ims.mnc095.mcc208.3gppnetwork.org"));

    // 2. Register Service Profile
    let sp = ServiceProfile {
        profile_id: "sp-voice-default".to_string(),
        ifcs: Vec::new(),
    };
    hss.register_service_profile(sp);

    // 3. Register 3 IMPUs belonging to the same subscriber
    let impu1 = "sip:+15551000001@ims.operator.net";
    let impu2 = "tel:+15551000001";
    let impu3 = "sip:alice@ims.operator.net";

    hss.register_impu_profile(impu1, impi, "sp-voice-default")
        .unwrap();
    hss.register_impu_profile(impu2, impi, "sp-voice-default")
        .unwrap();
    hss.register_impu_profile(impu3, impi, "sp-voice-default")
        .unwrap();

    // Verify IMPI contains all 3 IMPUs
    let sub = hss.impis.get(impi).unwrap();
    assert_eq!(sub.impus.len(), 3);

    // 4. Group all 3 IMPUs into an Implicit Registration Set (IRS)
    hss.configure_implicit_reg_set("irs-sub-001", vec![impu1, impu2, impu3]);

    // Initial state: all are Deregistered
    assert_eq!(
        hss.impus.get(impu1).unwrap().state,
        ImsRegistrationState::Deregistered
    );
    assert_eq!(
        hss.impus.get(impu2).unwrap().state,
        ImsRegistrationState::Deregistered
    );
    assert_eq!(
        hss.impus.get(impu3).unwrap().state,
        ImsRegistrationState::Deregistered
    );

    // 5. S-CSCF registers using impu1 -> all 3 IMPUs in the IRS must become Registered
    let scscf_fqdn = "scscf01.ims.operator.net";
    let scscf_uri = "sip:scscf01.ims.operator.net:5060";
    let timestamp = 1700000000;

    let affected = hss
        .register_scscf(impu1, scscf_fqdn, scscf_uri, timestamp)
        .expect("S-CSCF registration must succeed");

    assert_eq!(affected.len(), 3);
    assert_eq!(
        hss.impus.get(impu1).unwrap().state,
        ImsRegistrationState::Registered
    );
    assert_eq!(
        hss.impus.get(impu2).unwrap().state,
        ImsRegistrationState::Registered
    );
    assert_eq!(
        hss.impus.get(impu3).unwrap().state,
        ImsRegistrationState::Registered
    );

    assert_eq!(
        hss.impus.get(impu1).unwrap().registered_scscf.as_deref(),
        Some(scscf_uri)
    );
    assert_eq!(
        hss.impus.get(impu2).unwrap().registered_scscf.as_deref(),
        Some(scscf_uri)
    );

    // 6. S-CSCF deregisters -> all 3 IMPUs in the IRS must become Deregistered
    let dereg_affected = hss
        .deregister_scscf(impu2)
        .expect("Deregistration must succeed");

    assert_eq!(dereg_affected.len(), 3);
    assert_eq!(
        hss.impus.get(impu1).unwrap().state,
        ImsRegistrationState::Deregistered
    );
    assert_eq!(
        hss.impus.get(impu2).unwrap().state,
        ImsRegistrationState::Deregistered
    );
    assert_eq!(
        hss.impus.get(impu3).unwrap().state,
        ImsRegistrationState::Deregistered
    );
    assert!(hss.scscf_registrations.is_empty());
}

#[test]
fn test_hss_sbi_ifc_evaluation_and_as_routing() {
    let mut hss = HssSbiEngine::new("hss.voice.net");

    let impi = "sub-bob@ims.net";
    let impu = "sip:bob@ims.net";
    hss.register_impi_subscription(impi, "supi-bob", None);

    // Define 2 iFCs:
    // iFC 1 (Priority 10): MMTel TAS for Originating INVITE (Voice Call)
    // CNF: Method = INVITE AND SessionCase = Originating
    let ifc1 = InitialFilterCriteria {
        priority: 10,
        condition: TriggerCondition::Cnf,
        triggers: vec![
            ServicePointTrigger::SipMethod(SipMethod::Invite),
            ServicePointTrigger::SessionCaseMatch(SessionCase::Originating),
        ],
        application_server: ApplicationServer {
            server_name: "sip:mmtel-tas.ims.operator.net:5060".to_string(),
            default_handling: DefaultHandling::Continue,
        },
    };

    // iFC 2 (Priority 20): RCS Messaging AS
    // DNF: Header "Content-Type" contains "application/im-iscom-message" OR Request-URI contains "rcs"
    let ifc2 = InitialFilterCriteria {
        priority: 20,
        condition: TriggerCondition::Dnf,
        triggers: vec![
            ServicePointTrigger::HeaderMatch {
                name: "Content-Type".to_string(),
                value: "application/im-iscom-message".to_string(),
            },
            ServicePointTrigger::RequestUriContains("rcs.operator.net".to_string()),
        ],
        application_server: ApplicationServer {
            server_name: "sip:rcs-as.ims.operator.net:5060".to_string(),
            default_handling: DefaultHandling::Release,
        },
    };

    let sp = ServiceProfile {
        profile_id: "sp-advanced-multimedia".to_string(),
        ifcs: vec![ifc2, ifc1], // Inserted out of order to verify priority sort
    };
    hss.register_service_profile(sp);
    hss.register_impu_profile(impu, impi, "sp-advanced-multimedia")
        .unwrap();

    // Test Case 1: Originating SIP INVITE request (Voice Call)
    let mut invite_headers = HashMap::new();
    invite_headers.insert("From".to_string(), "<sip:bob@ims.net>".to_string());
    invite_headers.insert("To".to_string(), "<sip:charlie@ims.net>".to_string());

    let invite_msg = SipMessage {
        is_response: false,
        status_code: 0,
        reason_phrase: String::new(),
        method: Some(SipMethod::Invite),
        request_uri: "sip:charlie@ims.net".to_string(),
        headers: invite_headers,
        body: String::new(),
    };

    let as_list = hss
        .evaluate_ifc(impu, &invite_msg, SessionCase::Originating)
        .expect("iFC evaluation should succeed");

    assert_eq!(as_list.len(), 1);
    assert_eq!(
        as_list[0].server_name,
        "sip:mmtel-tas.ims.operator.net:5060"
    );
    assert_eq!(as_list[0].default_handling, DefaultHandling::Continue);

    // Test Case 2: Terminating Registered INVITE request -> Does not match iFC 1 because SessionCase is Terminating
    let as_list_term = hss
        .evaluate_ifc(impu, &invite_msg, SessionCase::TerminatingRegistered)
        .unwrap();
    assert!(as_list_term.is_empty());

    // Test Case 3: Instant Message with RCS Content-Type header
    let mut msg_headers = HashMap::new();
    msg_headers.insert(
        "Content-Type".to_string(),
        "application/im-iscom-message+xml".to_string(),
    );

    let chat_msg = SipMessage {
        is_response: false,
        status_code: 0,
        reason_phrase: String::new(),
        method: Some(SipMethod::Invite),
        request_uri: "sip:charlie@ims.net".to_string(),
        headers: msg_headers,
        body: "Hello via RCS".to_string(),
    };

    // DNF matches on Header "Content-Type"
    let as_list_rcs = hss
        .evaluate_ifc(impu, &chat_msg, SessionCase::TerminatingRegistered)
        .unwrap();

    assert_eq!(as_list_rcs.len(), 1);
    assert_eq!(
        as_list_rcs[0].server_name,
        "sip:rcs-as.ims.operator.net:5060"
    );
}

#[test]
fn test_hss_sbi_scscf_registration_and_restoration_info() {
    let mut hss = HssSbiEngine::new("hss.failover.net");

    let impi = "sub-charlie@ims.net";
    let impu = "sip:charlie@ims.net";
    hss.register_impi_subscription(impi, "supi-charlie", None);

    let sp = ServiceProfile {
        profile_id: "sp-basic".to_string(),
        ifcs: Vec::new(),
    };
    hss.register_service_profile(sp);
    hss.register_impu_profile(impu, impi, "sp-basic").unwrap();

    hss.register_scscf(
        impu,
        "scscf-primary.ims.net",
        "sip:scscf-primary.ims.net",
        1700000000,
    )
    .unwrap();

    // Store restoration info
    let restoration = ScscfRestorationInfo {
        impu: impu.to_string(),
        pcscf_fqdn: "pcscf-tokyo.ims.net".to_string(),
        contact_uri: "<sip:charlie@10.20.30.40:5060;transport=udp>".to_string(),
        path: vec!["<sip:term@pcscf-tokyo.ims.net;lr>".to_string()],
        icid: "ims-charging-id-777888".to_string(),
    };

    hss.store_restoration_info(restoration.clone())
        .expect("Store must succeed");

    let retrieved = hss
        .get_restoration_info(impu)
        .expect("Restoration info must exist");
    assert_eq!(retrieved.pcscf_fqdn, "pcscf-tokyo.ims.net");
    assert_eq!(
        retrieved.contact_uri,
        "<sip:charlie@10.20.30.40:5060;transport=udp>"
    );
    assert_eq!(retrieved.icid, "ims-charging-id-777888");
}

#[test]
fn test_hss_sbi_dual_registration_and_access_restrictions() {
    let mut hss = HssSbiEngine::new("hss.dualreg.net");

    let supi = "208950000000099";

    let restrictions = AccessRestrictionData {
        nr_as_secondary_rat_barred: false,
        unlicensed_spectrum_barred: true,
        satellite_access_barred: true,
        roaming_restricted: false,
    };

    hss.update_dual_registration(
        supi,
        Some("mme01.epc.mnc095.mcc208.3gppnetwork.org".to_string()),
        Some("amf01.5gc.mnc095.mcc208.3gppnetwork.org".to_string()),
        Some(restrictions.clone()),
    );

    let state = hss.dual_registrations.get(supi).expect("State must exist");
    assert_eq!(
        state.serving_mme.as_deref(),
        Some("mme01.epc.mnc095.mcc208.3gppnetwork.org")
    );
    assert_eq!(
        state.serving_amf.as_deref(),
        Some("amf01.5gc.mnc095.mcc208.3gppnetwork.org")
    );
    assert!(state.access_restrictions.satellite_access_barred);
    assert!(state.access_restrictions.unlicensed_spectrum_barred);
    assert!(!state.access_restrictions.nr_as_secondary_rat_barred);

    let retrieved_restrictions = hss.get_access_restrictions(supi).unwrap();
    assert_eq!(retrieved_restrictions, &restrictions);
}

#[test]
fn test_hss_sbi_error_handling_unknown_identities() {
    let mut hss = HssSbiEngine::new("hss.err.net");

    // 1. Registering IMPU with non-existent Service Profile
    let err_sp = hss.register_impu_profile("sip:ghost@ims.net", "impi-ghost", "non-existent-sp");
    assert_eq!(err_sp, Err(HssError::ServiceProfileNotFound));

    // 2. Evaluating iFC for unknown IMPU
    let dummy_msg = SipMessage {
        is_response: false,
        status_code: 0,
        reason_phrase: String::new(),
        method: Some(SipMethod::Invite),
        request_uri: "sip:target@ims.net".to_string(),
        headers: HashMap::new(),
        body: String::new(),
    };
    let err_eval = hss.evaluate_ifc("sip:unknown@ims.net", &dummy_msg, SessionCase::Originating);
    assert_eq!(err_eval, Err(HssError::ImpuNotFound));

    // 3. Registering S-CSCF for unknown IMPU
    let err_reg = hss.register_scscf("sip:unknown@ims.net", "scscf.net", "sip:scscf.net", 0);
    assert_eq!(err_reg, Err(HssError::ImpuNotFound));

    // 4. Storing restoration info for unknown IMPU
    let bad_restoration = ScscfRestorationInfo {
        impu: "sip:unknown@ims.net".to_string(),
        pcscf_fqdn: "pcscf.net".to_string(),
        contact_uri: "sip:contact.net".to_string(),
        path: Vec::new(),
        icid: "icid-1".to_string(),
    };
    let err_store = hss.store_restoration_info(bad_restoration);
    assert_eq!(err_store, Err(HssError::ImpuNotFound));
}
