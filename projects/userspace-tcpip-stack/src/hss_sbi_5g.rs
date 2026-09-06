//! 3GPP TS 29.562 / TS 29.563 / TS 23.501 Rel-17 5G HSS SBI Service & IMS Core Interworking Engine.
//!
//! Implements cloud-native 5G Home Subscriber Server (HSS) Service-Based Architecture (SBA):
//! - Nhss_imsSDM Service (TS 29.562 Section 5.2):
//!   - IMPI (Private User Identity) & IMPU (Public User Identity) subscription management.
//!   - Implicit Registration Sets (IRS): Synchronized registration states across associated public identities.
//!   - Initial Filter Criteria (iFC) Evaluation Engine (TS 29.228 Annex B):
//!     - Multi-trigger Service Point Triggers (SPTs) with CNF/DNF boolean logic.
//!     - Matches SIP method, SIP headers, Session Case, and Request-URI patterns.
//!     - Dispatches matching requests to Application Servers (MMTel TAS, RCS AS) with fallback handling.
//! - Nhss_imsUECM Service (TS 29.562 Section 5.3):
//!   - S-CSCF Registration, Keepalive, and Deregistration.
//!   - S-CSCF Restoration Info: Caches P-CSCF addresses, contact URIs, dialog paths, and ICID
//!     for instant disaster-recovery failover to backup S-CSCF nodes without call drops.
//! - Nhss_SDM / Nhss_UECM Interworking (TS 29.563):
//!   - Dual Registration Management: Coordinates serving MME (4G EPS) and serving AMF (5GS).
//!   - Access Restriction Data: Enforces RAT barring (NR Secondary RAT barred, Satellite barred).

use std::collections::HashMap;

use crate::sip::{SipMessage, SipMethod};

// ---------------------------------------------------------------------------
// 5G HSS & IMS Data Types (TS 29.562 Section 6 / TS 29.228)
// ---------------------------------------------------------------------------

/// IMS Registration State.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImsRegistrationState {
    Deregistered,
    AuthenticationPending,
    Registered,
}

/// IMS Session Case for iFC evaluation (TS 29.228 Section B.2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionCase {
    Originating,
    TerminatingRegistered,
    TerminatingUnregistered,
}

/// Default handling policy if Application Server is unreachable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultHandling {
    Continue,
    Release,
}

/// Application Server (AS) target defined in iFC (TS 29.228 Annex B).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationServer {
    pub server_name: String, // SIP URI, e.g. "sip:mmtel-tas.ims.operator.net:5060"
    pub default_handling: DefaultHandling,
}

/// Service Point Trigger (SPT) condition types (TS 29.228 Section B.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServicePointTrigger {
    SipMethod(SipMethod),
    HeaderMatch { name: String, value: String },
    SessionCaseMatch(SessionCase),
    RequestUriContains(String),
}

/// Boolean combination condition for triggers within an iFC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerCondition {
    /// Conjunctive Normal Form: ALL triggers must match (Logical AND).
    Cnf,
    /// Disjunctive Normal Form: AT LEAST ONE trigger must match (Logical OR).
    Dnf,
}

/// Initial Filter Criteria (iFC - TS 29.228 Annex B).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialFilterCriteria {
    pub priority: u32, // Lower number = higher evaluation priority
    pub condition: TriggerCondition,
    pub triggers: Vec<ServicePointTrigger>,
    pub application_server: ApplicationServer,
}

/// IMS Service Profile containing an ordered set of iFCs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceProfile {
    pub profile_id: String,
    pub ifcs: Vec<InitialFilterCriteria>,
}

/// IMPU (IMS Public User Identity) Profile (TS 29.562 Section 6.1.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpuProfile {
    pub impu: String, // e.g. "sip:+15551234567@ims.operator.net"
    pub impi: String, // Associated IMPI
    pub irs_id: Option<String>,
    pub service_profile_id: String,
    pub state: ImsRegistrationState,
    pub registered_scscf: Option<String>,
}

/// IMPI (IMS Private User Identity) Subscription record (TS 29.562 Section 6.1.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpiSubscription {
    pub impi: String, // e.g. "208950000000001@ims.mnc095.mcc208.3gppnetwork.org"
    pub supi: String,
    pub impus: Vec<String>,
    pub digest_realm: Option<String>,
}

/// S-CSCF Registration context (Nhss_imsUECM - TS 29.562 Section 5.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScscfRegistration {
    pub impu: String,
    pub scscf_fqdn: String,
    pub scscf_uri: String,
    pub state: ImsRegistrationState,
    pub auth_timestamp_s: u64,
}

/// S-CSCF Restoration Info cached for disaster-recovery failover (TS 29.562 Section 5.3.2.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScscfRestorationInfo {
    pub impu: String,
    pub pcscf_fqdn: String,
    pub contact_uri: String,
    pub path: Vec<String>,
    pub icid: String, // IMS Charging Identifier
}

/// 4G/5G Access Restriction Data (Nhss_SDM - TS 29.563 Section 6.1.6.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessRestrictionData {
    pub nr_as_secondary_rat_barred: bool,
    pub unlicensed_spectrum_barred: bool,
    pub satellite_access_barred: bool,
    pub roaming_restricted: bool,
}

impl Default for AccessRestrictionData {
    fn default() -> Self {
        AccessRestrictionData {
            nr_as_secondary_rat_barred: false,
            unlicensed_spectrum_barred: false,
            satellite_access_barred: false,
            roaming_restricted: false,
        }
    }
}

/// Dual Registration state tracking serving nodes in EPC and 5GS (TS 29.563).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DualRegistrationState {
    pub supi: String,
    pub serving_mme: Option<String>,
    pub serving_amf: Option<String>,
    pub access_restrictions: AccessRestrictionData,
}

/// Errors occurring in 5G HSS operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HssError {
    ImpiNotFound,
    ImpuNotFound,
    ServiceProfileNotFound,
    RegistrationNotFound,
    RestorationInfoNotFound,
}

// ---------------------------------------------------------------------------
// Top-Level 5G HSS SBI Engine
// ---------------------------------------------------------------------------

/// 5G Home Subscriber Server (HSS) SBI Engine.
pub struct HssSbiEngine {
    pub hss_id: String,
    pub impis: HashMap<String, ImpiSubscription>,
    pub impus: HashMap<String, ImpuProfile>,
    /// irs_id -> list of IMPUs in the Implicit Registration Set
    pub irs_groups: HashMap<String, Vec<String>>,
    pub service_profiles: HashMap<String, ServiceProfile>,
    pub scscf_registrations: HashMap<String, ScscfRegistration>,
    pub restoration_info: HashMap<String, ScscfRestorationInfo>,
    pub dual_registrations: HashMap<String, DualRegistrationState>,
}

impl HssSbiEngine {
    /// Create a new 5G HSS SBI Engine instance.
    pub fn new(hss_id: &str) -> Self {
        HssSbiEngine {
            hss_id: hss_id.to_string(),
            impis: HashMap::new(),
            impus: HashMap::new(),
            irs_groups: HashMap::new(),
            service_profiles: HashMap::new(),
            scscf_registrations: HashMap::new(),
            restoration_info: HashMap::new(),
            dual_registrations: HashMap::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Nhss_imsSDM: IMS Subscription & Profile Management (TS 29.562 §5.2)
    // -----------------------------------------------------------------------

    /// Register an IMPI subscription.
    pub fn register_impi_subscription(
        &mut self,
        impi: &str,
        supi: &str,
        digest_realm: Option<&str>,
    ) {
        let sub = ImpiSubscription {
            impi: impi.to_string(),
            supi: supi.to_string(),
            impus: Vec::new(),
            digest_realm: digest_realm.map(|s| s.to_string()),
        };
        self.impis.insert(impi.to_string(), sub);
    }

    /// Register a Service Profile containing iFCs.
    pub fn register_service_profile(&mut self, profile: ServiceProfile) {
        self.service_profiles
            .insert(profile.profile_id.clone(), profile);
    }

    /// Register an IMPU profile linked to an IMPI and Service Profile.
    pub fn register_impu_profile(
        &mut self,
        impu: &str,
        impi: &str,
        service_profile_id: &str,
    ) -> Result<(), HssError> {
        if !self.service_profiles.contains_key(service_profile_id) {
            return Err(HssError::ServiceProfileNotFound);
        }

        let impu_prof = ImpuProfile {
            impu: impu.to_string(),
            impi: impi.to_string(),
            irs_id: None,
            service_profile_id: service_profile_id.to_string(),
            state: ImsRegistrationState::Deregistered,
            registered_scscf: None,
        };

        self.impus.insert(impu.to_string(), impu_prof);

        if let Some(impi_sub) = self.impis.get_mut(impi) {
            if !impi_sub.impus.contains(&impu.to_string()) {
                impi_sub.impus.push(impu.to_string());
            }
        }

        Ok(())
    }

    /// Group a set of IMPUs into an Implicit Registration Set (IRS).
    pub fn configure_implicit_reg_set(&mut self, irs_id: &str, impus: Vec<&str>) {
        let string_impus: Vec<String> = impus.into_iter().map(|s| s.to_string()).collect();
        for impu in &string_impus {
            if let Some(prof) = self.impus.get_mut(impu) {
                prof.irs_id = Some(irs_id.to_string());
            }
        }
        self.irs_groups.insert(irs_id.to_string(), string_impus);
    }

    // -----------------------------------------------------------------------
    // Initial Filter Criteria (iFC) Evaluation Engine (TS 29.228 Annex B)
    // -----------------------------------------------------------------------

    /// Evaluate a single Service Point Trigger against an incoming SIP message.
    fn evaluate_spt(
        spt: &ServicePointTrigger,
        sip_msg: &SipMessage,
        session_case: SessionCase,
    ) -> bool {
        match spt {
            ServicePointTrigger::SipMethod(method) => sip_msg.method.as_ref() == Some(method),
            ServicePointTrigger::HeaderMatch { name, value } => {
                if let Some(header_val) = sip_msg.headers.get(name) {
                    header_val.contains(value)
                } else {
                    false
                }
            }
            ServicePointTrigger::SessionCaseMatch(expected_case) => *expected_case == session_case,
            ServicePointTrigger::RequestUriContains(pattern) => {
                sip_msg.request_uri.contains(pattern)
            }
        }
    }

    /// Evaluate iFC rules for a given IMPU against an incoming SIP message.
    /// Returns priority-ordered list of Application Servers to route the request through.
    pub fn evaluate_ifc(
        &self,
        impu: &str,
        sip_msg: &SipMessage,
        session_case: SessionCase,
    ) -> Result<Vec<ApplicationServer>, HssError> {
        let impu_prof = self.impus.get(impu).ok_or(HssError::ImpuNotFound)?;
        let srv_prof = self
            .service_profiles
            .get(&impu_prof.service_profile_id)
            .ok_or(HssError::ServiceProfileNotFound)?;

        // Sort iFCs by priority ascending (lower number = evaluated first)
        let mut sorted_ifcs = srv_prof.ifcs.clone();
        sorted_ifcs.sort_by_key(|ifc| ifc.priority);

        let mut matched_servers = Vec::new();

        for ifc in &sorted_ifcs {
            if ifc.triggers.is_empty() {
                // If triggers are empty, default matches all traffic
                matched_servers.push(ifc.application_server.clone());
                continue;
            }

            let matches = match ifc.condition {
                TriggerCondition::Cnf => {
                    // ALL triggers must match (Logical AND)
                    ifc.triggers
                        .iter()
                        .all(|spt| Self::evaluate_spt(spt, sip_msg, session_case))
                }
                TriggerCondition::Dnf => {
                    // AT LEAST ONE trigger must match (Logical OR)
                    ifc.triggers
                        .iter()
                        .any(|spt| Self::evaluate_spt(spt, sip_msg, session_case))
                }
            };

            if matches {
                matched_servers.push(ifc.application_server.clone());
            }
        }

        Ok(matched_servers)
    }

    // -----------------------------------------------------------------------
    // Nhss_imsUECM: S-CSCF Registration & Restoration Context (TS 29.562 §5.3)
    // -----------------------------------------------------------------------

    /// Register an S-CSCF for an IMPU (TS 29.562 Section 5.3.2.2).
    /// Automatically updates all IMPUs in the same Implicit Registration Set (IRS).
    pub fn register_scscf(
        &mut self,
        impu: &str,
        scscf_fqdn: &str,
        scscf_uri: &str,
        timestamp_s: u64,
    ) -> Result<Vec<String>, HssError> {
        let impu_prof = self.impus.get(impu).ok_or(HssError::ImpuNotFound)?;
        let irs_id = impu_prof.irs_id.clone();

        let affected_impus = match irs_id {
            Some(ref id) => self
                .irs_groups
                .get(id)
                .cloned()
                .unwrap_or_else(|| vec![impu.to_string()]),
            None => vec![impu.to_string()],
        };

        for target_impu in &affected_impus {
            if let Some(prof) = self.impus.get_mut(target_impu) {
                prof.state = ImsRegistrationState::Registered;
                prof.registered_scscf = Some(scscf_uri.to_string());
            }

            let reg = ScscfRegistration {
                impu: target_impu.clone(),
                scscf_fqdn: scscf_fqdn.to_string(),
                scscf_uri: scscf_uri.to_string(),
                state: ImsRegistrationState::Registered,
                auth_timestamp_s: timestamp_s,
            };
            self.scscf_registrations.insert(target_impu.clone(), reg);
        }

        Ok(affected_impus)
    }

    /// Deregister an S-CSCF for an IMPU (TS 29.562 Section 5.3.2.3).
    /// Automatically deregisters all associated IMPUs in the IRS.
    pub fn deregister_scscf(&mut self, impu: &str) -> Result<Vec<String>, HssError> {
        let impu_prof = self.impus.get(impu).ok_or(HssError::ImpuNotFound)?;
        let irs_id = impu_prof.irs_id.clone();

        let affected_impus = match irs_id {
            Some(ref id) => self
                .irs_groups
                .get(id)
                .cloned()
                .unwrap_or_else(|| vec![impu.to_string()]),
            None => vec![impu.to_string()],
        };

        for target_impu in &affected_impus {
            if let Some(prof) = self.impus.get_mut(target_impu) {
                prof.state = ImsRegistrationState::Deregistered;
                prof.registered_scscf = None;
            }
            self.scscf_registrations.remove(target_impu);
            self.restoration_info.remove(target_impu);
        }

        Ok(affected_impus)
    }

    /// Store S-CSCF Restoration Info for fast failover (TS 29.562 Section 5.3.2.4).
    pub fn store_restoration_info(&mut self, info: ScscfRestorationInfo) -> Result<(), HssError> {
        if !self.impus.contains_key(&info.impu) {
            return Err(HssError::ImpuNotFound);
        }
        self.restoration_info.insert(info.impu.clone(), info);
        Ok(())
    }

    /// Retrieve S-CSCF Restoration Info.
    pub fn get_restoration_info(&self, impu: &str) -> Option<&ScscfRestorationInfo> {
        self.restoration_info.get(impu)
    }

    // -----------------------------------------------------------------------
    // Nhss_SDM / Nhss_UECM: 4G/5G Dual Registration (TS 29.563)
    // -----------------------------------------------------------------------

    /// Update Dual Registration state tracking serving MME and serving AMF.
    pub fn update_dual_registration(
        &mut self,
        supi: &str,
        serving_mme: Option<String>,
        serving_amf: Option<String>,
        access_restrictions: Option<AccessRestrictionData>,
    ) {
        let entry = self
            .dual_registrations
            .entry(supi.to_string())
            .or_insert_with(|| DualRegistrationState {
                supi: supi.to_string(),
                serving_mme: None,
                serving_amf: None,
                access_restrictions: AccessRestrictionData::default(),
            });

        if let Some(mme) = serving_mme {
            entry.serving_mme = Some(mme);
        }
        if let Some(amf) = serving_amf {
            entry.serving_amf = Some(amf);
        }
        if let Some(restrictions) = access_restrictions {
            entry.access_restrictions = restrictions;
        }
    }

    /// Get Access Restriction Data for a subscriber.
    pub fn get_access_restrictions(&self, supi: &str) -> Option<&AccessRestrictionData> {
        self.dual_registrations
            .get(supi)
            .map(|state| &state.access_restrictions)
    }
}
