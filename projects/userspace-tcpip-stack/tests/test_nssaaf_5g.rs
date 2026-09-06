//! Integration tests for 3GPP TS 29.531 / TS 23.501 5G NSSAAF (Network Slice-Specific Authentication and Authorization Function).

use toy_tcpip::nssaaf_5g::*;

// ---------------------------------------------------------------------------
// 1. Enterprise Slice Authentication Happy Path
// ---------------------------------------------------------------------------

#[test]
fn test_nssaaf_enterprise_slice_auth_happy_path() {
    let mut nssaaf = NssaafEngine::new("nssaaf-core-01");
    let industrial_slice = Snssai::new(2, Some([0x01, 0x02, 0x03]));
    let aaa_s_fqdn = "aaa-s.factory.enterprise.com";
    let supi = "imsi-208950000000001";
    let secret = b"enterprise-device-cert-token-xyz".to_vec();

    nssaaf.register_enterprise_slice(industrial_slice, aaa_s_fqdn);
    nssaaf.add_enterprise_credential(aaa_s_fqdn, supi, secret.clone());

    // 1. AMF initiates slice auth
    let (ctx_id, eap_req) = nssaaf
        .initiate_slice_auth(supi, industrial_slice, "amf-factory-01")
        .expect("Initiation failed");

    assert_eq!(eap_req.code, EapCode::Request);
    assert_eq!(eap_req.identifier, 1);

    // 2. UE responds with valid enterprise credentials
    let ue_resp = EapPacket {
        code: EapCode::Response,
        identifier: 1,
        payload: secret,
    };

    let (status, eap_res) = nssaaf
        .progress_slice_auth(&ctx_id, &ue_resp, 1700000000)
        .expect("Auth progress failed");

    assert_eq!(status, SliceAuthStatus::Success);
    assert_eq!(eap_res.code, EapCode::Success);
    assert!(nssaaf.is_slice_authorized(supi, industrial_slice, 1700000000));
}

// ---------------------------------------------------------------------------
// 2. Invalid Enterprise Credential Rejection
// ---------------------------------------------------------------------------

#[test]
fn test_nssaaf_invalid_credential_failure() {
    let mut nssaaf = NssaafEngine::new("nssaaf-core-02");
    let enterprise_slice = Snssai::new(4, Some([0xaa, 0xbb, 0xcc]));
    let aaa_s_fqdn = "aaa-s.automotive.fleet.com";
    let supi = "imsi-208950000000002";

    nssaaf.register_enterprise_slice(enterprise_slice, aaa_s_fqdn);
    nssaaf.add_enterprise_credential(aaa_s_fqdn, supi, b"correct-secret".to_vec());

    let (ctx_id, _) = nssaaf
        .initiate_slice_auth(supi, enterprise_slice, "amf-01")
        .unwrap();

    // UE sends wrong password
    let bad_resp = EapPacket {
        code: EapCode::Response,
        identifier: 1,
        payload: b"wrong-tampered-password".to_vec(),
    };

    let (status, eap_res) = nssaaf
        .progress_slice_auth(&ctx_id, &bad_resp, 1700000000)
        .unwrap();

    assert_eq!(status, SliceAuthStatus::Failed);
    assert_eq!(eap_res.code, EapCode::Failure);
    assert!(!nssaaf.is_slice_authorized(supi, enterprise_slice, 1700000000));
}

// ---------------------------------------------------------------------------
// 3. Slice Revocation Lifecycle
// ---------------------------------------------------------------------------

#[test]
fn test_nssaaf_slice_revocation_lifecycle() {
    let mut nssaaf = NssaafEngine::new("nssaaf-core-03");
    let enterprise_slice = Snssai::new(2, Some([0x00, 0x00, 0x01]));
    let aaa_s_fqdn = "aaa-s.smartgrid.utility.com";
    let supi = "imsi-208950000000003";
    let secret = b"smartgrid-token".to_vec();

    nssaaf.register_enterprise_slice(enterprise_slice, aaa_s_fqdn);
    nssaaf.add_enterprise_credential(aaa_s_fqdn, supi, secret.clone());

    // Successfully authenticate
    let (ctx_id, _) = nssaaf
        .initiate_slice_auth(supi, enterprise_slice, "amf-grid-01")
        .unwrap();
    let resp = EapPacket {
        code: EapCode::Response,
        identifier: 1,
        payload: secret,
    };
    nssaaf.progress_slice_auth(&ctx_id, &resp, 1000).unwrap();
    assert!(nssaaf.is_slice_authorized(supi, enterprise_slice, 1000));

    // Enterprise AAA-S revokes slice access
    nssaaf
        .revoke_slice_auth(supi, enterprise_slice, "Security Policy Violation", 2000)
        .expect("Revocation failed");

    // Slice is no longer authorized
    assert!(!nssaaf.is_slice_authorized(supi, enterprise_slice, 2000));

    // Revocation notification queued for AMF
    assert_eq!(nssaaf.amf_revocation_queue.len(), 1);
    let notif = &nssaaf.amf_revocation_queue[0];
    assert_eq!(notif.supi, supi);
    assert_eq!(notif.amf_id, "amf-grid-01");
    assert_eq!(notif.reason, "Security Policy Violation");
}

// ---------------------------------------------------------------------------
// 4. Standard Slice (No NSSAA Required)
// ---------------------------------------------------------------------------

#[test]
fn test_nssaaf_unconfigured_slice_bypass() {
    let mut nssaaf = NssaafEngine::new("nssaaf-core-04");
    // Standard public eMBB slice
    let public_embb = Snssai::new(1, None);

    let err = nssaaf.initiate_slice_auth("imsi-12345", public_embb, "amf-01");
    assert_eq!(err, Err(NssaafError::SliceNotRequiringNssaa));
}

// ---------------------------------------------------------------------------
// 5. Slice Token Expiration
// ---------------------------------------------------------------------------

#[test]
fn test_nssaaf_slice_token_expiration() {
    let mut nssaaf = NssaafEngine::new("nssaaf-core-05");
    let slice = Snssai::new(2, Some([0x11, 0x22, 0x33]));
    let aaa_s = "aaa-s.test.com";
    let supi = "imsi-208950000000005";
    let secret = b"secret".to_vec();

    nssaaf.register_enterprise_slice(slice, aaa_s);
    nssaaf.add_enterprise_credential(aaa_s, supi, secret.clone());

    let (ctx_id, _) = nssaaf.initiate_slice_auth(supi, slice, "amf-01").unwrap();
    let resp = EapPacket {
        code: EapCode::Response,
        identifier: 1,
        payload: secret,
    };
    nssaaf.progress_slice_auth(&ctx_id, &resp, 1000).unwrap();

    // Within lifetime (default 86400s): t = 1000 + 3600 -> authorized
    assert!(nssaaf.is_slice_authorized(supi, slice, 4600));

    // Beyond lifetime: t = 1000 + 86401 -> expired
    assert!(!nssaaf.is_slice_authorized(supi, slice, 87401));
}
