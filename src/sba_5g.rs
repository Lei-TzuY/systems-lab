//! 5G Core Service Based Architecture (5G SBA - 3GPP TS 29.500 / TS 29.510 / TS 29.518 / TS 29.502).
//!
//! Implements 5G Control Plane Service-Based Architecture HTTP/2 REST dispatching,
//! NRF (Network Repository Function) discovery/registration, and AMF/SMF/UDM service pipelines.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NfType {
    Amf,
    Smf,
    Upf,
    Nrf,
    Udm,
    Pcf,
    Ausf,
    Nssf,
    Chf,
    Nwdaf,
    Udr,
    Nef,
    Bsf,
    Sepp,
    Scp,
    Lmf,
    N3iwf,
    Eir,
    Udsf,
    Gmlc,
    Easdf,
    Nssaaf,
    Nsacf,
    Ees,
    Ucmf,
    Mbsf,
    Ddnmf,
    Pkmf,
    Adrf,
}

impl NfType {
    pub fn as_str(&self) -> &'static str {
        match self {
            NfType::Amf => "AMF",
            NfType::Smf => "SMF",
            NfType::Upf => "UPF",
            NfType::Nrf => "NRF",
            NfType::Udm => "UDM",
            NfType::Pcf => "PCF",
            NfType::Ausf => "AUSF",
            NfType::Nssf => "NSSF",
            NfType::Chf => "CHF",
            NfType::Nwdaf => "NWDAF",
            NfType::Udr => "UDR",
            NfType::Nef => "NEF",
            NfType::Bsf => "BSF",
            NfType::Sepp => "SEPP",
            NfType::Scp => "SCP",
            NfType::Lmf => "LMF",
            NfType::N3iwf => "N3IWF",
            NfType::Eir => "EIR",
            NfType::Udsf => "UDSF",
            NfType::Gmlc => "GMLC",
            NfType::Easdf => "EASDF",
            NfType::Nssaaf => "NSSAAF",
            NfType::Nsacf => "NSACF",
            NfType::Ees => "EES",
            NfType::Ucmf => "UCMF",
            NfType::Mbsf => "MBSF",
            NfType::Ddnmf => "5G-DDNMF",
            NfType::Pkmf => "PKMF",
            NfType::Adrf => "ADRF",
        }
    }
}

/// 5G NF Profile registered in NRF
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NfProfile {
    pub nf_instance_id: String,
    pub nf_type: NfType,
    pub fqdn: String,
    pub ip_address: String,
    pub services: Vec<String>,
    pub capacity: u16,
}

/// 5G SBA REST Service Request (over HTTP/2)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbaRequest {
    pub service_name: String, // e.g. "namf-comm", "nsmf-pdusession", "nudm-sdm"
    pub method: String,       // "GET", "POST", "PUT", "DELETE"
    pub target_nf: NfType,
    pub resource_uri: String,
    pub payload_json: String,
}

/// 5G SBA REST Service Response
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbaResponse {
    pub status_code: u16, // 200 OK, 201 Created, 404 Not Found, etc.
    pub body_json: String,
}

/// 5G NRF (Network Repository Function) Registry
#[derive(Debug, Clone, Default)]
pub struct NrfRegistry {
    pub profiles: HashMap<String, NfProfile>, // instance_id -> profile
}

impl NrfRegistry {
    pub fn new() -> Self {
        NrfRegistry {
            profiles: HashMap::new(),
        }
    }

    /// Nnrf_NFManagement_NFRegister service operation
    pub fn register_nf(&mut self, profile: NfProfile) {
        self.profiles
            .insert(profile.nf_instance_id.clone(), profile);
    }

    /// Nnrf_NFManagement_NFDeregister service operation
    pub fn deregister_nf(&mut self, instance_id: &str) -> bool {
        self.profiles.remove(instance_id).is_some()
    }

    /// Nnrf_NFDiscovery_Request service operation
    pub fn discover_nf(&self, target_type: NfType) -> Vec<&NfProfile> {
        self.profiles
            .values()
            .filter(|p| p.nf_type == target_type)
            .collect()
    }
}

/// 5G SBA Inter-NF Message Dispatcher
#[derive(Debug, Clone, Default)]
pub struct SbaMessageBus {
    pub nrf: NrfRegistry,
    pub total_dispatched_messages: u64,
}

impl SbaMessageBus {
    pub fn new() -> Self {
        SbaMessageBus {
            nrf: NrfRegistry::new(),
            total_dispatched_messages: 0,
        }
    }

    /// Dispatches an SBA Service Request to the target Network Function
    pub fn dispatch(&mut self, req: &SbaRequest) -> SbaResponse {
        self.total_dispatched_messages += 1;

        // Query NRF to ensure target NF type exists
        let candidates = self.nrf.discover_nf(req.target_nf);
        if candidates.is_empty() {
            return SbaResponse {
                status_code: 404,
                body_json: format!(
                    "{{\"error\":\"No registered NF instances found for {}\"}}",
                    req.target_nf.as_str()
                ),
            };
        }

        // Mock NF service execution
        match req.target_nf {
            NfType::Amf => {
                // Namf_Communication_CreateUEContext
                SbaResponse {
                    status_code: 201,
                    body_json:
                        "{\"amfStatus\":\"UE_CONTEXT_CREATED\",\"supi\":\"imsi-208950000000001\"}"
                            .to_string(),
                }
            }
            NfType::Smf => {
                // Nsmf_PDUSession_CreateSMContext
                SbaResponse {
                    status_code: 201,
                    body_json: "{\"pduSessionId\":1,\"upfTunnelTeid\":0x1002,\"sessionState\":\"ESTABLISHED\"}".to_string(),
                }
            }
            NfType::Udm => {
                // Nudm_SDM_Get (Subscription Data Management)
                SbaResponse {
                    status_code: 200,
                    body_json: "{\"subscriberData\":{\"imsi\":\"208950000000001\",\"sliceDnn\":\"internet\"}}".to_string(),
                }
            }
            _ => SbaResponse {
                status_code: 200,
                body_json: "{\"result\":\"GENERIC_SBA_SUCCESS\"}".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nrf_registration_and_discovery() {
        let mut nrf = NrfRegistry::new();
        let amf_profile = NfProfile {
            nf_instance_id: "amf-inst-001".to_string(),
            nf_type: NfType::Amf,
            fqdn: "amf.5gcore.local".to_string(),
            ip_address: "10.100.1.1".to_string(),
            services: vec!["namf-comm".to_string(), "namf-evts".to_string()],
            capacity: 100,
        };

        nrf.register_nf(amf_profile);

        let discovered = nrf.discover_nf(NfType::Amf);
        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].fqdn, "amf.5gcore.local");
        assert_eq!(nrf.discover_nf(NfType::Smf).len(), 0);
    }

    #[test]
    fn test_sba_message_bus_dispatch() {
        let mut bus = SbaMessageBus::new();
        bus.nrf.register_nf(NfProfile {
            nf_instance_id: "smf-inst-001".to_string(),
            nf_type: NfType::Smf,
            fqdn: "smf.5gcore.local".to_string(),
            ip_address: "10.100.2.1".to_string(),
            services: vec!["nsmf-pdusession".to_string()],
            capacity: 100,
        });

        let req = SbaRequest {
            service_name: "nsmf-pdusession".to_string(),
            method: "POST".to_string(),
            target_nf: NfType::Smf,
            resource_uri: "/nsmf-pdusession/v1/sm-contexts".to_string(),
            payload_json: "{\"pduSessionId\":1,\"dnn\":\"internet\"}".to_string(),
        };

        let resp = bus.dispatch(&req);
        assert_eq!(resp.status_code, 201);
        assert!(resp.body_json.contains("ESTABLISHED"));
        assert_eq!(bus.total_dispatched_messages, 1);
    }
}
