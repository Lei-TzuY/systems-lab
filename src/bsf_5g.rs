//! 3GPP TS 29.521 5G Binding Support Function (BSF) Engine.
//!
//! Implements 5G Core policy binding resolution and session management:
//! - Nbsf_Management Service (TS 29.521 Section 5.2):
//!   - PCF Session Binding registration (`CreateBinding`) by SMF or PCF
//!   - PCF Binding discovery (`DiscoverBinding`) by Application Functions (AF), NEF, or DRA/PCRF
//!     matching UE IPv4 address, SUPI, DNN, and S-NSSAI
//!   - PCF Binding updates (`UpdateBinding`) and graceful deregistration (`DeregisterBinding`)
//!   - Multi-PCF routing disambiguation across distributed 5G Core clusters
//!   - 4G/5G Diameter Rx interworking with Diameter Host & Realm mappings

use std::collections::HashMap;

use crate::ipv4::Ipv4Address;
use crate::ngap_5g::Snssai;

// ---------------------------------------------------------------------------
// 5G BSF Data Structures (TS 29.521 Section 6)
// ---------------------------------------------------------------------------

/// Active PCF Session Binding record (TS 29.521 Section 6.1.6.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcfBinding {
    pub binding_id: String,
    pub supi: String,
    pub gpsi: Option<String>,
    pub ue_ipv4_address: Option<Ipv4Address>,
    pub dnn: String,
    pub snssai: Snssai,
    pub pdu_session_id: Option<u8>,
    pub pcf_instance_id: String,
    pub pcf_fqdn: String,
    pub pcf_ip_endpoints: Vec<Ipv4Address>,
    pub pcf_diameter_host: Option<String>,
    pub pcf_diameter_realm: Option<String>,
}

// ---------------------------------------------------------------------------
// Nbsf_Management Service Operations (TS 29.521 Section 5.2)
// ---------------------------------------------------------------------------

/// Request to register a new PCF Binding (POST /pcfBindings).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateBindingRequest {
    pub supi: String,
    pub gpsi: Option<String>,
    pub ue_ipv4_address: Option<Ipv4Address>,
    pub dnn: String,
    pub snssai: Snssai,
    pub pdu_session_id: Option<u8>,
    pub pcf_instance_id: String,
    pub pcf_fqdn: String,
    pub pcf_ip_endpoints: Vec<Ipv4Address>,
    pub pcf_diameter_host: Option<String>,
    pub pcf_diameter_realm: Option<String>,
}

/// Query parameters for discovering a PCF Binding (GET /pcfBindings).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoverBindingQuery {
    pub ue_ipv4_address: Option<Ipv4Address>,
    pub supi: Option<String>,
    pub dnn: Option<String>,
    pub snssai: Option<Snssai>,
}

/// Request to update an existing PCF Binding (PATCH /pcfBindings/{bindingId}).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateBindingRequest {
    pub binding_id: String,
    pub new_ipv4_address: Option<Ipv4Address>,
    pub new_pcf_ip_endpoints: Option<Vec<Ipv4Address>>,
}

// ---------------------------------------------------------------------------
// Top-Level BSF Engine
// ---------------------------------------------------------------------------

/// 5G Binding Support Function (BSF) Engine.
pub struct BsfEngine {
    pub bsf_instance_id: String,
    pub next_binding_id: u32,
    /// Primary storage: binding_id -> PcfBinding
    pub bindings_by_id: HashMap<String, PcfBinding>,
    /// Index by UE IPv4 address for fast O(1) AF discovery lookup
    pub ip_index: HashMap<Ipv4Address, String>,
    /// Index by SUPI: supi -> list of binding_ids
    pub supi_index: HashMap<String, Vec<String>>,
}

impl BsfEngine {
    /// Create a new BSF engine instance.
    pub fn new(bsf_instance_id: &str) -> Self {
        BsfEngine {
            bsf_instance_id: bsf_instance_id.to_string(),
            next_binding_id: 1001,
            bindings_by_id: HashMap::new(),
            ip_index: HashMap::new(),
            supi_index: HashMap::new(),
        }
    }

    /// Nbsf_Management_Register: Register a PCF binding for a PDU session.
    pub fn register_binding(
        &mut self,
        req: &CreateBindingRequest,
    ) -> Result<PcfBinding, &'static str> {
        let binding_id = format!("urn:bsf:binding:{}", self.next_binding_id);
        self.next_binding_id += 1;

        let binding = PcfBinding {
            binding_id: binding_id.clone(),
            supi: req.supi.clone(),
            gpsi: req.gpsi.clone(),
            ue_ipv4_address: req.ue_ipv4_address,
            dnn: req.dnn.clone(),
            snssai: req.snssai.clone(),
            pdu_session_id: req.pdu_session_id,
            pcf_instance_id: req.pcf_instance_id.clone(),
            pcf_fqdn: req.pcf_fqdn.clone(),
            pcf_ip_endpoints: req.pcf_ip_endpoints.clone(),
            pcf_diameter_host: req.pcf_diameter_host.clone(),
            pcf_diameter_realm: req.pcf_diameter_realm.clone(),
        };

        // Index by IPv4
        if let Some(ip) = req.ue_ipv4_address {
            self.ip_index.insert(ip, binding_id.clone());
        }

        // Index by SUPI
        self.supi_index
            .entry(req.supi.clone())
            .or_insert_with(Vec::new)
            .push(binding_id.clone());

        self.bindings_by_id.insert(binding_id, binding.clone());

        Ok(binding)
    }

    /// Nbsf_Management_Discover: Discover active PCF bindings matching query filters.
    pub fn discover_bindings(&self, query: &DiscoverBindingQuery) -> Vec<PcfBinding> {
        // Fast path: if IPv4 is provided, check direct index
        if let Some(ip) = query.ue_ipv4_address {
            if let Some(bid) = self.ip_index.get(&ip) {
                if let Some(binding) = self.bindings_by_id.get(bid) {
                    if query.dnn.as_ref().map_or(true, |d| d == &binding.dnn)
                        && query.snssai.as_ref().map_or(true, |s| s == &binding.snssai)
                    {
                        return vec![binding.clone()];
                    }
                }
            }
            return Vec::new();
        }

        // Fast path: if SUPI is provided, check SUPI index
        if let Some(supi) = &query.supi {
            if let Some(bids) = self.supi_index.get(supi) {
                let mut results = Vec::new();
                for bid in bids {
                    if let Some(binding) = self.bindings_by_id.get(bid) {
                        if query.dnn.as_ref().map_or(true, |d| d == &binding.dnn)
                            && query.snssai.as_ref().map_or(true, |s| s == &binding.snssai)
                        {
                            results.push(binding.clone());
                        }
                    }
                }
                return results;
            }
            return Vec::new();
        }

        // Fallback scan
        self.bindings_by_id
            .values()
            .filter(|b| {
                if let Some(dnn) = &query.dnn {
                    if &b.dnn != dnn {
                        return false;
                    }
                }
                if let Some(snssai) = &query.snssai {
                    if &b.snssai != snssai {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect()
    }

    /// Nbsf_Management_Update: Update binding IP or endpoints.
    pub fn update_binding(
        &mut self,
        req: &UpdateBindingRequest,
    ) -> Result<PcfBinding, &'static str> {
        let binding = self
            .bindings_by_id
            .get_mut(&req.binding_id)
            .ok_or("Binding not found")?;

        if let Some(new_ip) = req.new_ipv4_address {
            if let Some(old_ip) = binding.ue_ipv4_address {
                self.ip_index.remove(&old_ip);
            }
            binding.ue_ipv4_address = Some(new_ip);
            self.ip_index.insert(new_ip, req.binding_id.clone());
        }

        if let Some(endpoints) = &req.new_pcf_ip_endpoints {
            binding.pcf_ip_endpoints = endpoints.clone();
        }

        Ok(binding.clone())
    }

    /// Nbsf_Management_Deregister: Delete PCF binding upon session termination.
    pub fn deregister_binding(&mut self, binding_id: &str) -> bool {
        if let Some(binding) = self.bindings_by_id.remove(binding_id) {
            if let Some(ip) = binding.ue_ipv4_address {
                self.ip_index.remove(&ip);
            }
            if let Some(bids) = self.supi_index.get_mut(&binding.supi) {
                bids.retain(|id| id != binding_id);
            }
            true
        } else {
            false
        }
    }
}
