//! Integration tests for 3GPP TS 29.536 / TS 23.501 5G NSACF (Network Slice Admission Control Function).

use toy_tcpip::nsacf_5g::*;
use toy_tcpip::nssaaf_5g::Snssai;

// ---------------------------------------------------------------------------
// 1. UE Registration Quota Exhaustion & Recovery
// ---------------------------------------------------------------------------

#[test]
fn test_nsacf_ue_registration_quota_exhaustion() {
    let mut nsacf = NsacfEngine::new("nsacf-core-01");
    let mission_critical_slice = Snssai::new(2, Some([0x01, 0x02, 0x03]));

    // Max 2 UEs allowed on this private slice
    nsacf.configure_slice_quota(mission_critical_slice, Some(2), None, 80);

    // 1. Admit UE 1 -> Success
    let res1 = nsacf.update_ue_admission(
        mission_critical_slice,
        "imsi-208950000000001",
        NsacUpdateAction::Increase,
    );
    assert_eq!(res1, NsacAdmissionResult::Admitted);

    // 2. Admit UE 2 -> Success
    let res2 = nsacf.update_ue_admission(
        mission_critical_slice,
        "imsi-208950000000002",
        NsacUpdateAction::Increase,
    );
    assert_eq!(res2, NsacAdmissionResult::Admitted);

    // 3. Admit UE 3 -> Must be Refused (Capacity reached)
    let res3 = nsacf.update_ue_admission(
        mission_critical_slice,
        "imsi-208950000000003",
        NsacUpdateAction::Increase,
    );
    assert_eq!(res3, NsacAdmissionResult::RefusedExceededQuota);

    // 4. UE 1 deregisters
    nsacf.update_ue_admission(
        mission_critical_slice,
        "imsi-208950000000001",
        NsacUpdateAction::Decrease,
    );

    // 5. Admit UE 3 -> Now Admitted!
    let res3_retry = nsacf.update_ue_admission(
        mission_critical_slice,
        "imsi-208950000000003",
        NsacUpdateAction::Increase,
    );
    assert_eq!(res3_retry, NsacAdmissionResult::Admitted);
}

// ---------------------------------------------------------------------------
// 2. PDU Session Quota Exhaustion & Release
// ---------------------------------------------------------------------------

#[test]
fn test_nsacf_pdu_session_quota_exhaustion() {
    let mut nsacf = NsacfEngine::new("nsacf-core-02");
    let smart_grid_slice = Snssai::new(3, Some([0x00, 0x00, 0x01]));

    // Max 1 PDU session allowed concurrently
    nsacf.configure_slice_quota(smart_grid_slice, None, Some(1), 90);

    // UE A establishes PDU Session 1
    let res1 = nsacf.update_pdu_session_admission(
        smart_grid_slice,
        "imsi-ue-a",
        1,
        NsacUpdateAction::Increase,
    );
    assert_eq!(res1, NsacAdmissionResult::Admitted);

    // UE B establishes PDU Session 1 -> Exceeded quota
    let res2 = nsacf.update_pdu_session_admission(
        smart_grid_slice,
        "imsi-ue-b",
        1,
        NsacUpdateAction::Increase,
    );
    assert_eq!(res2, NsacAdmissionResult::RefusedExceededQuota);

    // UE A releases PDU Session 1
    nsacf.update_pdu_session_admission(
        smart_grid_slice,
        "imsi-ue-a",
        1,
        NsacUpdateAction::Decrease,
    );

    // UE B retries -> Admitted
    let res2_retry = nsacf.update_pdu_session_admission(
        smart_grid_slice,
        "imsi-ue-b",
        1,
        NsacUpdateAction::Increase,
    );
    assert_eq!(res2_retry, NsacAdmissionResult::Admitted);
}

// ---------------------------------------------------------------------------
// 3. Idempotent Admission
// ---------------------------------------------------------------------------

#[test]
fn test_nsacf_idempotent_admission() {
    let mut nsacf = NsacfEngine::new("nsacf-core-03");
    let slice = Snssai::new(1, Some([0x10, 0x20, 0x30]));
    nsacf.configure_slice_quota(slice, Some(2), None, 80);

    let supi = "imsi-111";

    // First request
    let res1 = nsacf.update_ue_admission(slice, supi, NsacUpdateAction::Increase);
    assert_eq!(res1, NsacAdmissionResult::Admitted);

    // Duplicate request (retransmission)
    let res2 = nsacf.update_ue_admission(slice, supi, NsacUpdateAction::Increase);
    assert_eq!(res2, NsacAdmissionResult::Admitted);

    let util = nsacf.get_slice_utilization(slice).unwrap();
    assert_eq!(util.current_ues, 1); // Not duplicated
}

// ---------------------------------------------------------------------------
// 4. Unconstrained Slice Bypass
// ---------------------------------------------------------------------------

#[test]
fn test_nsacf_unconstrained_slice_bypass() {
    let mut nsacf = NsacfEngine::new("nsacf-core-04");
    // Standard unconstrained public slice
    let public_slice = Snssai::new(1, None);

    let res = nsacf.update_ue_admission(public_slice, "imsi-999", NsacUpdateAction::Increase);
    assert_eq!(res, NsacAdmissionResult::SliceNotSubjectToNsac);
}

// ---------------------------------------------------------------------------
// 5. Capacity Monitoring & Threshold Alert
// ---------------------------------------------------------------------------

#[test]
fn test_nsacf_capacity_monitoring_and_threshold_alert() {
    let mut nsacf = NsacfEngine::new("nsacf-core-05");
    let slice = Snssai::new(2, Some([0xaa, 0xbb, 0xcc]));
    nsacf.configure_slice_quota(slice, Some(10), None, 80);

    // Add 7 UEs -> 70% utilization, no alert
    for i in 1..=7 {
        let supi = format!("imsi-{}", i);
        nsacf.update_ue_admission(slice, &supi, NsacUpdateAction::Increase);
    }
    let util1 = nsacf.get_slice_utilization(slice).unwrap();
    assert_eq!(util1.current_ues, 7);
    assert_eq!(util1.ue_utilization_pct, 70.0);
    assert!(!util1.alert_threshold_breached);

    // Add 8th UE -> 80% utilization, threshold alert triggered!
    nsacf.update_ue_admission(slice, "imsi-8", NsacUpdateAction::Increase);
    let util2 = nsacf.get_slice_utilization(slice).unwrap();
    assert_eq!(util2.current_ues, 8);
    assert_eq!(util2.ue_utilization_pct, 80.0);
    assert!(util2.alert_threshold_breached);
}
