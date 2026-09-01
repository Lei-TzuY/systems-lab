//! Integration tests for 3GPP TS 29.272 Diameter S13 Emergency Call / eCall IMEI Exemption Engine.

use toy_tcpip::diameter_s13_emergency_exemption::{
    DiameterS13EmergencyExemptionEngine, EmergencyCallType, EmergencyExemptionVerdict,
};
use toy_tcpip::diameter_s13_escn::S13EquipmentStatus;

#[test]
fn test_diameter_s13_emergency_exemption_integration() {
    let mut engine = DiameterS13EmergencyExemptionEngine::new(1800); // 30 min duration

    let blacklisted_ue = "353918009999999";
    let whitelisted_ue = "867123450000001";
    engine.set_equipment_status(blacklisted_ue, S13EquipmentStatus::BlackListed);
    engine.set_equipment_status(whitelisted_ue, S13EquipmentStatus::WhiteListed);

    // 1. Whitelisted standard call -> allowed
    let v1 = engine.evaluate_access(whitelisted_ue, false, None, "internet", 100);
    assert_eq!(
        v1,
        EmergencyExemptionVerdict::StandardAccessAllowed {
            imei: whitelisted_ue.to_string(),
            status: S13EquipmentStatus::WhiteListed,
        }
    );

    // 2. Blacklisted standard call -> blocked
    let v2 = engine.evaluate_access(blacklisted_ue, false, None, "ims", 100);
    match v2 {
        EmergencyExemptionVerdict::NonEmergencyBlocked { imei, status, .. } => {
            assert_eq!(imei, blacklisted_ue);
            assert_eq!(status, S13EquipmentStatus::BlackListed);
        }
        _ => panic!("Expected NonEmergencyBlocked"),
    }

    // 3. Blacklisted eCall automated crash notification -> emergency exempt
    let v3 = engine.evaluate_access(
        blacklisted_ue,
        true,
        Some(EmergencyCallType::ECallAutomaticCrash),
        "sos",
        100,
    );
    match v3 {
        EmergencyExemptionVerdict::EmergencyExemptionGranted {
            imei,
            call_type,
            session_id,
            permitted_apn,
            ..
        } => {
            assert_eq!(imei, blacklisted_ue);
            assert_eq!(call_type, EmergencyCallType::ECallAutomaticCrash);
            assert_eq!(session_id, 1);
            assert_eq!(permitted_apn, "sos");
        }
        _ => panic!("Expected EmergencyExemptionGranted"),
    }

    // 4. Terminate session explicitly
    assert!(engine.terminate_session(1));
    assert!(!engine.terminate_session(999));
}
