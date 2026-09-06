//! Integration tests for ITU-T G.8275.2 Telecom Profile for Phase/Time Synchronization with Partial Timing Support (PTS).

use std::collections::HashMap;

use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::ptp_g8275_2::*;

// ---------------------------------------------------------------------------
// 1. Unicast Negotiation Lifecycle & Lease Expiration
// ---------------------------------------------------------------------------

#[test]
fn test_g8275_2_unicast_negotiation_and_lease_expiry() {
    let master_ip = Ipv4Address::new(192, 168, 10, 1);
    let slave_ip = Ipv4Address::new(192, 168, 10, 50);
    let mut master = G8275_2MasterEngine::new(master_ip);

    let start_time = 1000;

    // Slave requests 64 pkts/s Sync (-6) for 300 seconds
    let req_sync = UnicastRequest {
        client_ip: slave_ip,
        message_type: G8275_2MessageType::Sync,
        requested_rate_log2: -6,
        requested_duration_s: 300,
    };
    let grant_sync = master
        .handle_unicast_request(&req_sync, start_time)
        .expect("Sync grant failed");
    assert_eq!(grant_sync.granted_rate_log2, -6);
    assert_eq!(grant_sync.granted_duration_s, 300);

    // Slave requests Announce (8 pkts/s -> -3)
    let req_ann = UnicastRequest {
        client_ip: slave_ip,
        message_type: G8275_2MessageType::Announce,
        requested_rate_log2: -3,
        requested_duration_s: 300,
    };
    master.handle_unicast_request(&req_ann, start_time).unwrap();
    assert_eq!(master.active_client_count(), 1);

    // Time advances by 100s -> still active
    assert_eq!(master.expire_leases(start_time + 100), 0);
    assert_eq!(master.active_client_count(), 1);

    // Time advances past 300s expiration -> leases expired and client removed
    let expired = master.expire_leases(start_time + 301);
    assert_eq!(expired, 2);
    assert_eq!(master.active_client_count(), 0);
}

// ---------------------------------------------------------------------------
// 2. Alternate BMCA ClockClass Precedence
// ---------------------------------------------------------------------------

#[test]
fn test_g8275_2_alternate_bmca_clock_class_precedence() {
    let slave_ip = Ipv4Address::new(10, 0, 0, 100);
    let mut slave = G8275_2SlaveEngine::new(slave_ip);

    let gm1_ip = Ipv4Address::new(10, 0, 1, 1);
    let gm2_ip = Ipv4Address::new(10, 0, 2, 1);

    // GM1: Primary PRTC / GNSS Locked (ClockClass 6)
    slave.add_or_update_candidate(G8275_2MasterCandidate {
        master_ip: gm1_ip,
        clock_class: 6,
        clock_accuracy: 0x21,
        offset_scaled_log_variance: 0x4e5d,
        priority2: 128,
        local_priority: 128,
        steps_removed: 2,
        static_asymmetry_ns: 0,
        active_leases: HashMap::new(),
    });

    // GM2: Degraded Master in Holdover (ClockClass 135)
    slave.add_or_update_candidate(G8275_2MasterCandidate {
        master_ip: gm2_ip,
        clock_class: 135,
        clock_accuracy: 0x22,
        offset_scaled_log_variance: 0x5e5d,
        priority2: 128,
        local_priority: 10, // Even with higher local preference, ClockClass takes precedence!
        steps_removed: 1,
        static_asymmetry_ns: 0,
        active_leases: HashMap::new(),
    });

    let selected = slave.run_alternate_bmca();
    assert_eq!(selected, Some(gm1_ip));
}

// ---------------------------------------------------------------------------
// 3. Alternate BMCA LocalPriority Tie-Breaking
// ---------------------------------------------------------------------------

#[test]
fn test_g8275_2_alternate_bmca_local_priority_tie_breaking() {
    let slave_ip = Ipv4Address::new(10, 0, 0, 100);
    let mut slave = G8275_2SlaveEngine::new(slave_ip);

    let gm_primary_ip = Ipv4Address::new(10, 10, 1, 1);
    let gm_secondary_ip = Ipv4Address::new(10, 10, 2, 1);

    // Both GMs have identical PRTC ClockClass 6
    slave.add_or_update_candidate(G8275_2MasterCandidate {
        master_ip: gm_primary_ip,
        clock_class: 6,
        clock_accuracy: 0x21,
        offset_scaled_log_variance: 0x4e5d,
        priority2: 128,
        local_priority: 50, // Preferred primary route
        steps_removed: 3,
        static_asymmetry_ns: 0,
        active_leases: HashMap::new(),
    });

    slave.add_or_update_candidate(G8275_2MasterCandidate {
        master_ip: gm_secondary_ip,
        clock_class: 6,
        clock_accuracy: 0x21,
        offset_scaled_log_variance: 0x4e5d,
        priority2: 128,
        local_priority: 100, // Backup route
        steps_removed: 3,
        static_asymmetry_ns: 0,
        active_leases: HashMap::new(),
    });

    let selected = slave.run_alternate_bmca();
    assert_eq!(selected, Some(gm_primary_ip));
}

// ---------------------------------------------------------------------------
// 4. Static Path Delay Asymmetry Calibration
// ---------------------------------------------------------------------------

#[test]
fn test_g8275_2_static_asymmetry_delay_compensation() {
    let slave_ip = Ipv4Address::new(172, 16, 0, 2);
    let mut slave = G8275_2SlaveEngine::new(slave_ip);

    let master_ip = Ipv4Address::new(172, 16, 0, 1);

    // Routed L3 network has known forward/reverse routing asymmetry of +400 ns
    slave.add_or_update_candidate(G8275_2MasterCandidate {
        master_ip,
        clock_class: 6,
        clock_accuracy: 0x20,
        offset_scaled_log_variance: 0x4000,
        priority2: 128,
        local_priority: 128,
        steps_removed: 1,
        static_asymmetry_ns: 400, // +400 ns forward delay bias
        active_leases: HashMap::new(),
    });

    // Raw measured offset including routing asymmetry
    let measured_offset = 250; // ns

    // Corrected Offset = 250 - (400 / 2) = 50 ns
    let calibrated_offset = slave.apply_asymmetry_correction(measured_offset, master_ip);
    assert_eq!(calibrated_offset, 50);
}

// ---------------------------------------------------------------------------
// 5. Slave Servo Lock & Holdover State Machine
// ---------------------------------------------------------------------------

#[test]
fn test_g8275_2_slave_servo_state_and_holdover_aging() {
    let slave_ip = Ipv4Address::new(192, 168, 1, 100);
    let mut slave = G8275_2SlaveEngine::new(slave_ip);
    slave.max_holdover_in_spec_s = 7200; // 2 hours

    assert_eq!(slave.state, G8275_2SlaveState::FreeRun);

    // Slave achieves lock within 1.5us limit
    let t0 = 100_000;
    slave.update_servo_lock(120, true, t0);
    assert_eq!(slave.state, G8275_2SlaveState::Locked);

    // Large transient error (>1.5us) pushes to Tracking
    slave.update_servo_lock(2500, true, t0 + 10);
    assert_eq!(slave.state, G8275_2SlaveState::Tracking);

    // Relocks
    slave.update_servo_lock(80, true, t0 + 20);
    assert_eq!(slave.state, G8275_2SlaveState::Locked);

    // Signal lost -> Enters HoldoverInSpec
    slave.update_servo_lock(0, false, t0 + 30);
    assert_eq!(slave.state, G8275_2SlaveState::HoldoverInSpec);

    // Holdover aging within 2h limit
    slave.update_servo_lock(0, false, t0 + 30 + 5000);
    assert_eq!(slave.state, G8275_2SlaveState::HoldoverInSpec);

    // Holdover aging exceeds 7200s -> Transitions to HoldoverOutOfSpec
    slave.update_servo_lock(0, false, t0 + 30 + 7201);
    assert_eq!(slave.state, G8275_2SlaveState::HoldoverOutOfSpec);
}
