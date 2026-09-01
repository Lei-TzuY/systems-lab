// src/gtpu_bearer_qos_flow_map.rs
//
// 3GPP TS 29.281 / TS 23.501 5G-to-4G Bearer ID (EBI) to QoS Flow Identifier (QFI) Translation Engine
// References:
// - 3GPP TS 23.501 Section 5.7: QoS Architecture and 5G-4G Interworking
// - 3GPP TS 23.502 Section 4.11: 5GS to EPS Handover and Interworking
// - 3GPP TS 29.281 Section 5.1: GTP-U Header & PDU Session Container QFI Field

pub const MIN_VALID_EBI: u8 = 5;
pub const MAX_VALID_EBI: u8 = 15;
pub const MIN_VALID_QFI: u8 = 1;
pub const MAX_VALID_QFI: u8 = 64;

/// Result of 4G Bearer to 5G QoS Flow translation or vice versa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BearerFlowTranslationVerdict {
    QfiToEbiMapped {
        qfi: u8,
        ebi: u8,
        qci: u8,
        tunnel_teid: u32,
    },
    EbiToQfiResolved {
        ebi: u8,
        default_qfi: u8,
        all_mapped_qfis: Vec<u8>,
        tunnel_teid: u32,
    },
    UnmappedQfiFallback {
        qfi: u8,
        fallback_ebi: u8,
        fallback_teid: u32,
    },
    BearerNotFound {
        ebi: u8,
    },
}

/// EPS Bearer to QoS Flow binding configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BearerFlowBinding {
    pub ebi: u8,
    pub qci: u8,
    pub mapped_qfis: Vec<u8>,
    pub is_default_bearer: bool,
    pub tunnel_teid: u32,
}

/// 3GPP 5G-to-4G Bearer ID (EBI) to QoS Flow (QFI) Translation Engine.
#[derive(Debug, Clone)]
pub struct GtpuBearerQosFlowMapEngine {
    pub default_ebi: u8,
    pub default_tunnel_teid: u32,
    pub bindings: Vec<BearerFlowBinding>,
    pub total_translations: u64,
    pub total_qfi_to_ebi: u64,
    pub total_ebi_to_qfi: u64,
    pub total_fallbacks: u64,
}

impl Default for GtpuBearerQosFlowMapEngine {
    fn default() -> Self {
        Self::new(5, 0x10001)
    }
}

impl GtpuBearerQosFlowMapEngine {
    pub fn new(default_ebi: u8, default_tunnel_teid: u32) -> Self {
        let mut engine = Self {
            default_ebi: if (MIN_VALID_EBI..=MAX_VALID_EBI).contains(&default_ebi) {
                default_ebi
            } else {
                5
            },
            default_tunnel_teid,
            bindings: Vec::new(),
            total_translations: 0,
            total_qfi_to_ebi: 0,
            total_ebi_to_qfi: 0,
            total_fallbacks: 0,
        };

        // Seed default 4G/5G standard bearer mappings
        // EBI 5: Default Internet / Web (QCI 9, QFI 9, QFI 8)
        engine.register_bearer_binding(5, 9, &[9, 8], true, default_tunnel_teid);
        // EBI 6: IMS Signaling (QCI 5, QFI 5)
        engine.register_bearer_binding(6, 5, &[5], false, default_tunnel_teid + 1);
        // EBI 7: Conversational Voice VoLTE/VoNR (QCI 1, QFI 1)
        engine.register_bearer_binding(7, 1, &[1], false, default_tunnel_teid + 2);
        // EBI 8: Low-Latency URLLC (QCI 82, QFI 82 -> QFI 2)
        engine.register_bearer_binding(8, 2, &[2], false, default_tunnel_teid + 3);

        engine
    }

    /// Register or update an EPS Bearer to 5G QoS Flow binding.
    pub fn register_bearer_binding(
        &mut self,
        ebi: u8,
        qci: u8,
        mapped_qfis: &[u8],
        is_default_bearer: bool,
        tunnel_teid: u32,
    ) {
        if let Some(b) = self.bindings.iter_mut().find(|b| b.ebi == ebi) {
            b.qci = qci;
            b.mapped_qfis = mapped_qfis.to_vec();
            b.is_default_bearer = is_default_bearer;
            b.tunnel_teid = tunnel_teid;
        } else {
            self.bindings.push(BearerFlowBinding {
                ebi,
                qci,
                mapped_qfis: mapped_qfis.to_vec(),
                is_default_bearer,
                tunnel_teid,
            });
        }
    }

    /// Translate a 5G QFI to its corresponding 4G EPS Bearer (EBI) and GTP-U Tunnel TEID.
    pub fn translate_qfi_to_ebi(&mut self, qfi: u8) -> BearerFlowTranslationVerdict {
        self.total_translations += 1;
        self.total_qfi_to_ebi += 1;

        for b in &self.bindings {
            if b.mapped_qfis.contains(&qfi) {
                return BearerFlowTranslationVerdict::QfiToEbiMapped {
                    qfi,
                    ebi: b.ebi,
                    qci: b.qci,
                    tunnel_teid: b.tunnel_teid,
                };
            }
        }

        // Fallback to default bearer
        self.total_fallbacks += 1;
        BearerFlowTranslationVerdict::UnmappedQfiFallback {
            qfi,
            fallback_ebi: self.default_ebi,
            fallback_teid: self.default_tunnel_teid,
        }
    }

    /// Resolve a 4G EPS Bearer (EBI) to its primary and multiplexed 5G QFIs.
    pub fn resolve_ebi_to_qfi(&mut self, ebi: u8) -> BearerFlowTranslationVerdict {
        self.total_translations += 1;
        self.total_ebi_to_qfi += 1;

        if let Some(b) = self.bindings.iter().find(|b| b.ebi == ebi) {
            let default_qfi = b.mapped_qfis.first().copied().unwrap_or(9);
            BearerFlowTranslationVerdict::EbiToQfiResolved {
                ebi,
                default_qfi,
                all_mapped_qfis: b.mapped_qfis.clone(),
                tunnel_teid: b.tunnel_teid,
            }
        } else {
            BearerFlowTranslationVerdict::BearerNotFound { ebi }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qfi_to_ebi_mappings() {
        let mut engine = GtpuBearerQosFlowMapEngine::new(5, 0x10001);

        // VoNR / VoLTE Voice (QFI 1) -> EBI 7 (QCI 1)
        let v1 = engine.translate_qfi_to_ebi(1);
        assert_eq!(
            v1,
            BearerFlowTranslationVerdict::QfiToEbiMapped {
                qfi: 1,
                ebi: 7,
                qci: 1,
                tunnel_teid: 0x10003,
            }
        );

        // Unregistered QFI 33 -> Fallback to Default Bearer EBI 5
        let v_fall = engine.translate_qfi_to_ebi(33);
        assert_eq!(
            v_fall,
            BearerFlowTranslationVerdict::UnmappedQfiFallback {
                qfi: 33,
                fallback_ebi: 5,
                fallback_teid: 0x10001,
            }
        );

        // Resolve EBI 5 -> QFIs [9, 8]
        let v_ebi = engine.resolve_ebi_to_qfi(5);
        assert_eq!(
            v_ebi,
            BearerFlowTranslationVerdict::EbiToQfiResolved {
                ebi: 5,
                default_qfi: 9,
                all_mapped_qfis: vec![9, 8],
                tunnel_teid: 0x10001,
            }
        );
    }
}
