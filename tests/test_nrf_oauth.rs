use toy_tcpip::nrf_oauth::{
    NrfAccessTokenClaims, NrfAccessTokenRequest, NrfAccessTokenResponse, NrfOAuthAuthority,
};
use toy_tcpip::sba_5g::NfType;

#[test]
fn test_nrf_oauth_unsupported_grant_type() {
    let mut authority = NrfOAuthAuthority::new("nrf-edge-01");
    let req = NrfAccessTokenRequest {
        grant_type: "authorization_code".to_string(), // Invalid
        nf_instance_id: "smf-01".to_string(),
        nf_type: NfType::Smf,
        target_nf_type: NfType::Pcf,
        scope: "npcf-smpolicycontrol".to_string(),
    };

    let result = authority.issue_access_token(req, 1700000000);
    assert!(result.is_err());
}

#[test]
fn test_nrf_oauth_claims_and_scope_validation() {
    let mut authority = NrfOAuthAuthority::new("nrf-core-01");
    let req = NrfAccessTokenRequest {
        grant_type: "client_credentials".to_string(),
        nf_instance_id: "amf-01".to_string(),
        nf_type: NfType::Amf,
        target_nf_type: NfType::Udm,
        scope: "nudm-sdm".to_string(),
    };

    let resp: NrfAccessTokenResponse = authority.issue_access_token(req, 1700000000).unwrap();
    assert!(resp.access_token.starts_with("5G-JWT-NRF-"));

    let claims: &NrfAccessTokenClaims = &authority.active_tokens[0].1;
    assert_eq!(claims.subject, "amf-01");
    assert_eq!(claims.audience, NfType::Udm);
    assert_eq!(claims.scope, "nudm-sdm");

    // Invalid scope verification
    assert!(!authority.verify_token(&resp.access_token, NfType::Udm, "nudm-uecm", 1700000100));

    // Valid scope verification
    assert!(authority.verify_token(&resp.access_token, NfType::Udm, "nudm-sdm", 1700000100));
}
