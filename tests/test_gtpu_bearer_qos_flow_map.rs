// tests/test_gtpu_bearer_qos_flow_map.rs

use toy_tcpip::gtpu_bearer_qos_flow_map::{
    BearerFlowTranslationVerdict, GtpuBearerQosFlowMapEngine, MAX_VALID_EBI, MAX_VALID_QFI,
    MIN_VALID_EBI, MIN_VALID_QFI,
};

#[test]
fn test_gtpu_bearer_qos_flow_map_lifecycle() {
    let mut engine = GtpuBearerQosFlowMapEngine::new(5, 0x20001);

    // 1. QFI 1 (Voice) -> EBI 7 (QCI 1)
    let v1 = engine.translate_qfi_to_ebi(1);
    assert_eq!(
        v1,
        BearerFlowTranslationVerdict::QfiToEbiMapped {
            qfi: 1,
            ebi: 7,
            qci: 1,
            tunnel_teid: 0x20003,
        }
    );

    // 2. QFI 5 (IMS Signaling) -> EBI 6 (QCI 5)
    let v2 = engine.translate_qfi_to_ebi(5);
    assert_eq!(
        v2,
        BearerFlowTranslationVerdict::QfiToEbiMapped {
            qfi: 5,
            ebi: 6,
            qci: 5,
            tunnel_teid: 0x20002,
        }
    );

    // 3. Register custom URLLC gaming flow QFI 80 -> Dedicated EBI 10
    engine.register_bearer_binding(10, 82, &[80], false, 0x20005);
    let v3 = engine.translate_qfi_to_ebi(80);
    assert_eq!(
        v3,
        BearerFlowTranslationVerdict::QfiToEbiMapped {
            qfi: 80,
            ebi: 10,
            qci: 82,
            tunnel_teid: 0x20005,
        }
    );

    // 4. Unmapped QFI 50 -> Fallback to Default Bearer EBI 5
    let v_fallback = engine.translate_qfi_to_ebi(50);
    assert_eq!(
        v_fallback,
        BearerFlowTranslationVerdict::UnmappedQfiFallback {
            qfi: 50,
            fallback_ebi: 5,
            fallback_teid: 0x20001,
        }
    );

    // 5. Resolve EBI 5 -> multiple QFIs [9, 8]
    let v_resolve = engine.resolve_ebi_to_qfi(5);
    assert_eq!(
        v_resolve,
        BearerFlowTranslationVerdict::EbiToQfiResolved {
            ebi: 5,
            default_qfi: 9,
            all_mapped_qfis: vec![9, 8],
            tunnel_teid: 0x20001,
        }
    );

    // 6. Resolve non-existent EBI 14
    let v_not_found = engine.resolve_ebi_to_qfi(14);
    assert_eq!(
        v_not_found,
        BearerFlowTranslationVerdict::BearerNotFound { ebi: 14 }
    );

    assert_eq!(engine.total_translations, 6);
    assert_eq!(engine.total_qfi_to_ebi, 4);
    assert_eq!(engine.total_ebi_to_qfi, 2);
    assert_eq!(engine.total_fallbacks, 1);
}

#[test]
fn test_bearer_flow_constants() {
    assert_eq!(MIN_VALID_EBI, 5);
    assert_eq!(MAX_VALID_EBI, 15);
    assert_eq!(MIN_VALID_QFI, 1);
    assert_eq!(MAX_VALID_QFI, 64);
}
