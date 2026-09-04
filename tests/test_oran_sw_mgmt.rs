//! Comprehensive Integration Tests for O-RAN WG4 M-Plane Software Management Engine.

use toy_tcpip::oran_sw_mgmt::*;

#[test]
fn test_oran_sw_mgmt_dual_slot_inventory() {
    let product_code = "ORU-REL17-SUB6-4T4R";
    let initial_slot = "SLOT_0";
    let initial_version = "v1.0.0-rel17";

    let mgr = OranSoftwareManager::new(product_code, initial_slot, initial_version);

    // Verify dual-slot architecture
    assert_eq!(mgr.product_code, product_code);
    assert_eq!(mgr.running_slot, "SLOT_0");
    assert_eq!(mgr.active_slot, "SLOT_0");
    assert!(!mgr.pending_commit);
    assert_eq!(mgr.rollback_timer_remaining_seconds, None);

    // Verify running slot properties
    let running = mgr.get_running_slot();
    assert_eq!(running.name, "SLOT_0");
    assert_eq!(running.status, SlotStatus::Valid);
    assert!(running.active);
    assert!(running.running);
    assert_eq!(running.access, SlotAccess::ReadWrite);
    assert_eq!(running.build_version, initial_version);
    assert_eq!(running.integrity, IntegrityStatus::Verified);
    assert_eq!(running.files.len(), 1);

    // Verify standby slot properties
    let standby = mgr.get_slot("SLOT_1").expect("SLOT_1 must exist");
    assert_eq!(standby.name, "SLOT_1");
    assert_eq!(standby.status, SlotStatus::Empty);
    assert!(!standby.active);
    assert!(!standby.running);
    assert_eq!(standby.files.len(), 0);

    // Verify RFC 7950 XML serialization
    let xml = mgr.to_rfc7950_xml();
    assert!(xml.contains("<software-inventory xmlns=\"urn:o-ran:software-management:1.0\">"));
    assert!(xml.contains("<product-code>ORU-REL17-SUB6-4T4R</product-code>"));
    assert!(xml.contains("<running-slot>SLOT_0</running-slot>"));
    assert!(xml.contains("<name>SLOT_0</name>"));
    assert!(xml.contains("<status>VALID</status>"));
    assert!(xml.contains("<name>SLOT_1</name>"));
    assert!(xml.contains("<status>EMPTY</status>"));

    // Verify RFC 7951 JSON serialization
    let json = mgr.to_rfc7951_json();
    assert!(json.contains("\"o-ran-software-management:software-inventory\""));
    assert!(json.contains("\"running-slot\":\"SLOT_0\""));
    assert!(json.contains("\"name\":\"SLOT_0\""));
    assert!(json.contains("\"status\":\"VALID\""));
    assert!(json.contains("\"name\":\"SLOT_1\""));
    assert!(json.contains("\"status\":\"EMPTY\""));
}

#[test]
fn test_oran_sw_mgmt_download_and_checksum_verification() {
    let mut mgr = OranSoftwareManager::new("ORU-TEST", "SLOT_0", "v1.0");

    let firmware_payload = b"ORAN_FRONTHAUL_FIRMWARE_BINARY_V2.0_PAYLOAD_DATA".to_vec();
    let expected_sha256 = compute_sha256(&firmware_payload);

    // Verify SHA-256 hex utilities
    let hex = sha256_to_hex(&expected_sha256);
    assert_eq!(hex.len(), 64);
    let parsed_sha256 = hex_to_sha256(&hex).expect("Hex parsing must succeed");
    assert_eq!(parsed_sha256, expected_sha256);

    // 1. Successful download via SFTP
    let sftp_uri = "sftp://mgmt-sw.local:22/oru/fw_v2.0.tar.gz";
    let status = mgr.software_download(sftp_uri, firmware_payload.clone(), expected_sha256);
    assert_eq!(status, DownloadStatus::Completed);
    assert_eq!(mgr.staging_package, Some(firmware_payload.clone()));

    // 2. Corrupted checksum rejection
    let mut bad_payload = firmware_payload.clone();
    bad_payload[0] ^= 0xFF; // Corrupt 1 byte
    let status_bad = mgr.software_download(sftp_uri, bad_payload, expected_sha256);
    assert_eq!(status_bad, DownloadStatus::CorruptedChecksum);

    // 3. Unsupported protocol rejection
    let bad_protocol_uri = "tftp://mgmt-sw.local/oru/fw.bin";
    let status_proto =
        mgr.software_download(bad_protocol_uri, firmware_payload.clone(), expected_sha256);
    assert_eq!(status_proto, DownloadStatus::ProtocolError);

    // 4. Empty payload rejection
    let status_empty = mgr.software_download(sftp_uri, Vec::new(), expected_sha256);
    assert_eq!(status_empty, DownloadStatus::FileNotFound);
}

#[test]
fn test_oran_sw_mgmt_install_and_validation() {
    let product_code = "ORU-REL17-SUB6-4T4R";
    let mut mgr = OranSoftwareManager::new(product_code, "SLOT_0", "v1.0.0");

    let manifest = vec![
        SoftwareFile {
            name: "fpga_rru_dpd.bin".to_string(),
            version: "v2.0.0".to_string(),
            size_bytes: 8 * 1024 * 1024,
            checksum_sha256: [0x11u8; 32],
        },
        SoftwareFile {
            name: "oru_linux_kernel.elf".to_string(),
            version: "v2.0.0".to_string(),
            size_bytes: 12 * 1024 * 1024,
            checksum_sha256: [0x22u8; 32],
        },
    ];

    // 1. Safety check: Cannot install into currently RUNNING slot
    let err_running = mgr.software_install(
        "SLOT_0",
        "Upgrade Build",
        "v2.0.0",
        "BUILD-200",
        product_code,
        manifest.clone(),
    );
    assert_eq!(err_running, InstallStatus::SlotIsRunning);

    // 2. Safety check: Incompatible Product Code
    let err_prod = mgr.software_install(
        "SLOT_1",
        "Upgrade Build",
        "v2.0.0",
        "BUILD-200",
        "WRONG-HARDWARE-SKU",
        manifest.clone(),
    );
    assert_eq!(err_prod, InstallStatus::ProductCodeMismatch);
    assert_eq!(mgr.get_slot("SLOT_1").unwrap().status, SlotStatus::Invalid);

    // 3. Safety check: Empty manifest
    let err_manifest = mgr.software_install(
        "SLOT_1",
        "Upgrade Build",
        "v2.0.0",
        "BUILD-200",
        product_code,
        Vec::new(),
    );
    assert_eq!(err_manifest, InstallStatus::InvalidManifest);

    // 4. Successful installation into standby SLOT_1
    let ok_install = mgr.software_install(
        "SLOT_1",
        "Release 17 Production",
        "v2.0.0",
        "BUILD-2026-09",
        product_code,
        manifest.clone(),
    );
    assert_eq!(ok_install, InstallStatus::Completed);

    let slot1 = mgr.get_slot("SLOT_1").unwrap();
    assert_eq!(slot1.status, SlotStatus::Valid);
    assert_eq!(slot1.build_version, "v2.0.0");
    assert_eq!(slot1.build_id, "BUILD-2026-09");
    assert_eq!(slot1.integrity, IntegrityStatus::Verified);
    assert_eq!(slot1.files.len(), 2);
    assert!(!slot1.running);
    assert!(!slot1.active);
}

#[test]
fn test_oran_sw_mgmt_happy_path_activate_and_commit() {
    let product_code = "ORU-REL17-SUB6-4T4R";
    let mut mgr = OranSoftwareManager::new(product_code, "SLOT_0", "v1.0.0");

    let manifest = vec![SoftwareFile {
        name: "oru_fw.bin".to_string(),
        version: "v2.0.0".to_string(),
        size_bytes: 16 * 1024 * 1024,
        checksum_sha256: [0x88u8; 32],
    }];

    // Step 1: Install v2.0.0 into SLOT_1
    assert_eq!(
        mgr.software_install(
            "SLOT_1",
            "Release 2.0",
            "v2.0.0",
            "BUILD-V2",
            product_code,
            manifest
        ),
        InstallStatus::Completed
    );

    // Step 2: Activate SLOT_1 with 180s watchdog timer
    let act_status = mgr.software_activate("SLOT_1", Some(180));
    assert_eq!(act_status, ActivationStatus::Completed);

    // Verify new slot is running and active
    assert_eq!(mgr.running_slot, "SLOT_1");
    assert_eq!(mgr.active_slot, "SLOT_1");
    assert!(mgr.get_slot("SLOT_1").unwrap().running);
    assert!(mgr.get_slot("SLOT_1").unwrap().active);
    assert!(!mgr.get_slot("SLOT_0").unwrap().running);
    assert!(!mgr.get_slot("SLOT_0").unwrap().active);

    // Verify commit pending state and watchdog
    assert!(mgr.pending_commit);
    assert_eq!(mgr.rollback_slot, Some("SLOT_0".to_string()));
    assert_eq!(mgr.rollback_timer_remaining_seconds, Some(180));

    // Step 3: Advance clock by 60 seconds (120s remaining)
    let ev = mgr.tick_seconds(60);
    assert_eq!(ev, None);
    assert_eq!(mgr.rollback_timer_remaining_seconds, Some(120));
    assert!(mgr.pending_commit);

    // Step 4: Software commit confirmation from Fronthaul OAM controller
    let commit_status = mgr.software_commit();
    assert_eq!(commit_status, CommitStatus::Completed);

    // Watchdog disarmed and state permanently locked
    assert!(!mgr.pending_commit);
    assert_eq!(mgr.rollback_slot, None);
    assert_eq!(mgr.rollback_timer_remaining_seconds, None);

    // Step 5: Clock advances past original timeout -> no rollback occurs!
    let ev_after = mgr.tick_seconds(200);
    assert_eq!(ev_after, None);
    assert_eq!(mgr.running_slot, "SLOT_1");
    assert_eq!(mgr.active_slot, "SLOT_1");
}

#[test]
fn test_oran_sw_mgmt_watchdog_auto_rollback() {
    let product_code = "ORU-REL17-SUB6-4T4R";
    let mut mgr = OranSoftwareManager::new(product_code, "SLOT_0", "v1.0.0");

    let manifest = vec![SoftwareFile {
        name: "buggy_oru_fw.bin".to_string(),
        version: "v2.1.0-broken".to_string(),
        size_bytes: 16 * 1024 * 1024,
        checksum_sha256: [0xAAu8; 32],
    }];

    // Install into SLOT_1
    assert_eq!(
        mgr.software_install(
            "SLOT_1",
            "Broken Candidate",
            "v2.1.0-broken",
            "BUILD-ERR",
            product_code,
            manifest
        ),
        InstallStatus::Completed
    );

    // Activate with 60 second watchdog timeout
    assert_eq!(
        mgr.software_activate("SLOT_1", Some(60)),
        ActivationStatus::Completed
    );
    assert_eq!(mgr.running_slot, "SLOT_1");

    // Advance 30 seconds -> still running SLOT_1
    assert_eq!(mgr.tick_seconds(30), None);
    assert_eq!(mgr.rollback_timer_remaining_seconds, Some(30));
    assert_eq!(mgr.running_slot, "SLOT_1");

    // Advance 35 seconds -> watchdog expires!
    let rollback_event = mgr
        .tick_seconds(35)
        .expect("Rollback event must be emitted");
    match rollback_event {
        SoftwareEvent::AutoRollbackTriggered {
            failed_slot,
            restored_slot,
            reason,
        } => {
            assert_eq!(failed_slot, "SLOT_1");
            assert_eq!(restored_slot, "SLOT_0");
            assert!(reason.contains("Watchdog timer expired"));
        }
        _ => panic!("Expected AutoRollbackTriggered event"),
    }

    // Verify failed slot is now INVALID and inactive
    let slot1 = mgr.get_slot("SLOT_1").unwrap();
    assert_eq!(slot1.status, SlotStatus::Invalid);
    assert!(!slot1.running);
    assert!(!slot1.active);

    // Verify restored slot is active and running again
    let slot0 = mgr.get_slot("SLOT_0").unwrap();
    assert_eq!(slot0.status, SlotStatus::Valid);
    assert!(slot0.running);
    assert!(slot0.active);

    assert_eq!(mgr.running_slot, "SLOT_0");
    assert_eq!(mgr.active_slot, "SLOT_0");
    assert!(!mgr.pending_commit);
    assert_eq!(mgr.rollback_timer_remaining_seconds, None);
}
