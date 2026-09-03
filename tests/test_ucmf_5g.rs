//! Integration tests for 3GPP TS 29.525 / TS 23.501 5G UCMF (UE Radio Capability Management Function).

use toy_tcpip::ucmf_5g::*;

// ---------------------------------------------------------------------------
// 1. Assign and Resolve RAC ID Happy Path
// ---------------------------------------------------------------------------

#[test]
fn test_ucmf_assign_and_resolve_rac_id_happy_path() {
    let mut ucmf = UcmfEngine::new("ucmf-core-01");
    let plmn_id = "208-95";

    // Simulated 5G NR ASN.1 capability payload (e.g. n78, n28, 4x4 MIMO, 256QAM)
    let mock_nr_capability = vec![0x30, 0x82, 0x01, 0x55, 0x02, 0x01, 0x00, 0xAA, 0xBB, 0xCC];

    let rac_id = ucmf
        .assign_rac_id(
            plmn_id,
            RadioCapFormat::Nr,
            mock_nr_capability.clone(),
            Some("Pixel-8-Pro"),
            1700000000,
        )
        .expect("Assignment failed");

    assert_eq!(rac_id.rac_type, RacIdType::PlmnAssigned);
    assert_eq!(rac_id.plmn_id, "208-95");
    assert!(rac_id.id_string.contains("PLMN-20895-RAC-"));

    // Resolve RAC ID
    let entry = ucmf.resolve_rac_id(&rac_id).expect("Resolution failed");
    assert_eq!(entry.cap_format, RadioCapFormat::Nr);
    assert_eq!(entry.capability_bytes, mock_nr_capability);
    assert_eq!(entry.associated_models, vec!["Pixel-8-Pro".to_string()]);
}

// ---------------------------------------------------------------------------
// 2. Capability Deduplication Across Device Models
// ---------------------------------------------------------------------------

#[test]
fn test_ucmf_capability_deduplication() {
    let mut ucmf = UcmfEngine::new("ucmf-core-02");
    let plmn = "466-92";

    let common_firmware_bytes = vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];

    // Model A registers capability
    let rac_a = ucmf
        .assign_rac_id(
            plmn,
            RadioCapFormat::Nr,
            common_firmware_bytes.clone(),
            Some("Model-Alpha"),
            1000,
        )
        .unwrap();

    // Model B has identical capability bytes
    let rac_b = ucmf
        .assign_rac_id(
            plmn,
            RadioCapFormat::Nr,
            common_firmware_bytes,
            Some("Model-Beta"),
            1050,
        )
        .unwrap();

    // Deduplication must return identical RAC ID
    assert_eq!(rac_a, rac_b);

    let entry = ucmf.resolve_rac_id(&rac_a).unwrap();
    assert_eq!(
        entry.associated_models,
        vec!["Model-Alpha".to_string(), "Model-Beta".to_string()]
    );
}

// ---------------------------------------------------------------------------
// 3. Manufacturer-Assigned RAC ID Ingestion
// ---------------------------------------------------------------------------

#[test]
fn test_ucmf_manufacturer_assigned_rac_id() {
    let mut ucmf = UcmfEngine::new("ucmf-core-03");
    let mfr_rac = RacId::new_manufacturer_assigned("208-95", "MFR-CHIPSET-X75-CAP1");
    let cap_bytes = vec![0xAA, 0xBB, 0xCC, 0xDD];

    ucmf.register_manufacturer_rac_id(
        mfr_rac.clone(),
        RadioCapFormat::MrDc,
        cap_bytes.clone(),
        1000,
    )
    .expect("Ingestion failed");

    let entry = ucmf.resolve_rac_id(&mfr_rac).unwrap();
    assert_eq!(entry.rac_id.rac_type, RacIdType::ManufacturerAssigned);
    assert_eq!(entry.cap_format, RadioCapFormat::MrDc);
    assert_eq!(entry.capability_bytes, cap_bytes);

    // Duplicate registration should fail
    let err = ucmf.register_manufacturer_rac_id(mfr_rac, RadioCapFormat::MrDc, cap_bytes, 1010);
    assert_eq!(err, Err(UcmfError::DuplicateManufacturerRacId));
}

// ---------------------------------------------------------------------------
// 4. Dictionary Deletion & Not Found
// ---------------------------------------------------------------------------

#[test]
fn test_ucmf_dictionary_deletion_and_not_found() {
    let mut ucmf = UcmfEngine::new("ucmf-core-04");
    let rac = ucmf
        .assign_rac_id(
            "208-95",
            RadioCapFormat::Eutra,
            vec![0x01, 0x02],
            None,
            1000,
        )
        .unwrap();

    // Delete
    ucmf.delete_dictionary_entry(&rac).unwrap();

    // Now resolve must fail
    assert_eq!(ucmf.resolve_rac_id(&rac), Err(UcmfError::RacIdNotFound));
}

// ---------------------------------------------------------------------------
// 5. Empty Capability Payload Rejection
// ---------------------------------------------------------------------------

#[test]
fn test_ucmf_empty_payload_rejection() {
    let mut ucmf = UcmfEngine::new("ucmf-core-05");
    let err = ucmf.assign_rac_id("208-95", RadioCapFormat::Nr, Vec::new(), None, 1000);
    assert_eq!(
        err,
        Err(UcmfError::InvalidCapabilityPayload(
            "Capability bytes cannot be empty"
        ))
    );
}
