//! 5G Core NRF OAuth 2.0 Access Token Service (3GPP TS 29.510 Nnrf_AccessToken & TS 33.501 Security Architecture).
//!
//! Implements service-to-service authorization token minting, scope validation,
//! and Bearer token verification across 5G Service Based Architecture (SBA) interfaces.

use crate::sba_5g::NfType;

/// OAuth 2.0 Access Token Request (TS 29.510 Section 5.3.2)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NrfAccessTokenRequest {
    pub grant_type: String, // "client_credentials"
    pub nf_instance_id: String,
    pub nf_type: NfType,
    pub target_nf_type: NfType,
    pub scope: String, // e.g. "nudm-sdm", "npcf-smpolicycontrol"
}

/// Minted 5G Access Token Claims
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NrfAccessTokenClaims {
    pub issuer: String,   // NRF instance ID
    pub subject: String,  // Consumer NF instance ID
    pub audience: NfType, // Target Producer NF Type
    pub scope: String,
    pub issued_at_sec: u64,
    pub expires_at_sec: u64,
}

/// OAuth 2.0 Access Token Response
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NrfAccessTokenResponse {
    pub access_token: String,
    pub token_type: String, // "Bearer"
    pub expires_in_sec: u32,
    pub scope: String,
}

/// 5G NRF OAuth 2.0 Token Authority & Verification Server
#[derive(Debug, Clone)]
pub struct NrfOAuthAuthority {
    pub nrf_instance_id: String,
    pub default_token_lifetime_sec: u32,
    pub active_tokens: Vec<(String, NrfAccessTokenClaims)>,
    pub token_counter: u64,
}

impl NrfOAuthAuthority {
    pub fn new(nrf_instance_id: &str) -> Self {
        NrfOAuthAuthority {
            nrf_instance_id: nrf_instance_id.to_string(),
            default_token_lifetime_sec: 3600,
            active_tokens: Vec::new(),
            token_counter: 1,
        }
    }

    /// Issues an OAuth 2.0 Access Token for an authorized consumer NF
    pub fn issue_access_token(
        &mut self,
        req: NrfAccessTokenRequest,
        now_sec: u64,
    ) -> Result<NrfAccessTokenResponse, &'static str> {
        if req.grant_type != "client_credentials" {
            return Err("Unsupported grant_type (must be client_credentials)");
        }

        let token_str = format!(
            "5G-JWT-NRF-{}-{:08X}",
            self.nrf_instance_id, self.token_counter
        );
        self.token_counter += 1;

        let claims = NrfAccessTokenClaims {
            issuer: self.nrf_instance_id.clone(),
            subject: req.nf_instance_id.clone(),
            audience: req.target_nf_type,
            scope: req.scope.clone(),
            issued_at_sec: now_sec,
            expires_at_sec: now_sec + (self.default_token_lifetime_sec as u64),
        };

        self.active_tokens.push((token_str.clone(), claims));

        Ok(NrfAccessTokenResponse {
            access_token: token_str,
            token_type: "Bearer".to_string(),
            expires_in_sec: self.default_token_lifetime_sec,
            scope: req.scope,
        })
    }

    /// Producer NF validates incoming Bearer Token before serving SBA request
    pub fn verify_token(
        &self,
        token: &str,
        producer_nf_type: NfType,
        required_scope: &str,
        now_sec: u64,
    ) -> bool {
        if let Some((_, claims)) = self.active_tokens.iter().find(|(t, _)| t == token) {
            if claims.audience != producer_nf_type {
                return false;
            }
            if claims.scope != required_scope {
                return false;
            }
            if now_sec > claims.expires_at_sec {
                return false; // Token expired
            }
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nrf_oauth_issue_and_verify_token() {
        let mut authority = NrfOAuthAuthority::new("nrf-core-01");

        let req = NrfAccessTokenRequest {
            grant_type: "client_credentials".to_string(),
            nf_instance_id: "amf-taiwan-01".to_string(),
            nf_type: NfType::Amf,
            target_nf_type: NfType::Udm,
            scope: "nudm-sdm".to_string(),
        };

        let resp = authority.issue_access_token(req, 1700000000).unwrap();
        assert_eq!(resp.token_type, "Bearer");
        assert_eq!(resp.expires_in_sec, 3600);

        // Verification at UDM: Valid
        let is_valid =
            authority.verify_token(&resp.access_token, NfType::Udm, "nudm-sdm", 1700000500);
        assert!(is_valid);

        // Verification at PCF: Rejected (wrong audience)
        let is_valid_pcf =
            authority.verify_token(&resp.access_token, NfType::Pcf, "nudm-sdm", 1700000500);
        assert!(!is_valid_pcf);

        // Verification after expiration: Rejected
        let is_expired =
            authority.verify_token(&resp.access_token, NfType::Udm, "nudm-sdm", 1700005000);
        assert!(!is_expired);
    }
}
