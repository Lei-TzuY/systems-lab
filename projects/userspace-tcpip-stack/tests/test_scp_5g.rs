//! Integration tests for 3GPP TS 29.500 / TS 29.510 5G Service Communication Proxy (SCP) Engine.

use std::collections::HashMap;

use toy_tcpip::sba_5g::NfType;
use toy_tcpip::scp_5g::*;

// ---------------------------------------------------------------------------
// 1. Delegated Discovery & Load Balancing Happy Path
// ---------------------------------------------------------------------------

#[test]
fn test_scp_delegated_discovery_and_round_robin() {
    let mut scp = ScpEngine::new("scp-core-001");

    // Register two SMF backend instances
    scp.register_backend(
        ScpBackendInstance {
            instance_id: "smf-inst-01".to_string(),
            nf_type: NfType::Smf,
            fqdn: "smf01.5gc.carrier.net".to_string(),
            weight: 100,
            locality: "Tokyo-East".to_string(),
        },
        3,
        30,
    );
    scp.register_backend(
        ScpBackendInstance {
            instance_id: "smf-inst-02".to_string(),
            nf_type: NfType::Smf,
            fqdn: "smf02.5gc.carrier.net".to_string(),
            weight: 100,
            locality: "Tokyo-East".to_string(),
        },
        3,
        30,
    );

    let req = ScpForwardRequest {
        consumer_nf_id: "amf-inst-01".to_string(),
        target_nf_type: Some(NfType::Smf),
        target_api_root: None,
        target_instance_id: None,
        http_method: "POST".to_string(),
        path: "/nsmf-pdusession/v1/sm-contexts".to_string(),
        headers: HashMap::new(),
        payload: b"{}".to_vec(),
        priority: 16,
    };

    let resp1 = scp.forward_message(&req, 1700000000).unwrap();
    let resp2 = scp.forward_message(&req, 1700000000).unwrap();

    // Round-robin dispatching between the two instances
    let targets = vec![resp1.routed_to_instance_id, resp2.routed_to_instance_id];
    assert!(targets.contains(&"smf-inst-01".to_string()));
    assert!(targets.contains(&"smf-inst-02".to_string()));
}

// ---------------------------------------------------------------------------
// 2. Canary / A/B Testing Traffic Splitting
// ---------------------------------------------------------------------------

#[test]
fn test_scp_canary_traffic_splitting() {
    let mut scp = ScpEngine::new("scp-core-002");

    scp.register_backend(
        ScpBackendInstance {
            instance_id: "pcf-v1-stable".to_string(),
            nf_type: NfType::Pcf,
            fqdn: "pcf-v1.5gc.local".to_string(),
            weight: 100,
            locality: "Region-1".to_string(),
        },
        3,
        30,
    );
    scp.register_backend(
        ScpBackendInstance {
            instance_id: "pcf-v2-canary".to_string(),
            nf_type: NfType::Pcf,
            fqdn: "pcf-v2.5gc.local".to_string(),
            weight: 100,
            locality: "Region-1".to_string(),
        },
        3,
        30,
    );

    // Split 20% traffic to Canary v2, 80% to Stable v1
    scp.add_canary_rule(CanaryRule {
        rule_id: "canary-pcf-rollout".to_string(),
        target_nf_type: NfType::Pcf,
        canary_percentage: 20,
        stable_instance_id: "pcf-v1-stable".to_string(),
        canary_instance_id: "pcf-v2-canary".to_string(),
    });

    let req = ScpForwardRequest {
        consumer_nf_id: "smf-inst-01".to_string(),
        target_nf_type: Some(NfType::Pcf),
        target_api_root: None,
        target_instance_id: None,
        http_method: "POST".to_string(),
        path: "/npcf-smpolicycontrol/v1/sm-policies".to_string(),
        headers: HashMap::new(),
        payload: b"{}".to_vec(),
        priority: 16,
    };

    let mut canary_hits = 0;
    let mut stable_hits = 0;

    for _ in 0..100 {
        let resp = scp.forward_message(&req, 1700000000).unwrap();
        if resp.routed_to_instance_id == "pcf-v2-canary" {
            canary_hits += 1;
        } else if resp.routed_to_instance_id == "pcf-v1-stable" {
            stable_hits += 1;
        }
    }

    assert_eq!(canary_hits, 20);
    assert_eq!(stable_hits, 80);
}

// ---------------------------------------------------------------------------
// 3. Circuit Breaker Trip & Failover to Backup Instance
// ---------------------------------------------------------------------------

#[test]
fn test_scp_circuit_breaker_trip_and_failover() {
    let mut scp = ScpEngine::new("scp-core-003");

    scp.register_backend(
        ScpBackendInstance {
            instance_id: "udm-primary".to_string(),
            nf_type: NfType::Udm,
            fqdn: "udm01.5gc.local".to_string(),
            weight: 100,
            locality: "Site-A".to_string(),
        },
        3,  // 3 consecutive failures trips circuit
        30, // 30s recovery timeout
    );
    scp.register_backend(
        ScpBackendInstance {
            instance_id: "udm-backup".to_string(),
            nf_type: NfType::Udm,
            fqdn: "udm02.5gc.local".to_string(),
            weight: 100,
            locality: "Site-B".to_string(),
        },
        3,
        30,
    );

    let t0 = 1700000000;

    // Simulate 3 failures on udm-primary
    scp.report_instance_result("udm-primary", false, t0);
    scp.report_instance_result("udm-primary", false, t0 + 1);
    scp.report_instance_result("udm-primary", false, t0 + 2);

    assert_eq!(
        scp.circuit_breakers.get("udm-primary").unwrap().state,
        CircuitState::Open
    );

    // Request explicitly targeting udm-primary automatically fails over to udm-backup
    let req = ScpForwardRequest {
        consumer_nf_id: "ausf-01".to_string(),
        target_nf_type: Some(NfType::Udm),
        target_api_root: None,
        target_instance_id: Some("udm-primary".to_string()),
        http_method: "GET".to_string(),
        path: "/nudm-ueau/v1/suci-1".to_string(),
        headers: HashMap::new(),
        payload: vec![],
        priority: 16,
    };

    let resp = scp.forward_message(&req, t0 + 5).unwrap();
    assert_eq!(resp.routed_to_instance_id, "udm-backup");
    assert_eq!(resp.routed_to_fqdn, "udm02.5gc.local");
}

// ---------------------------------------------------------------------------
// 4. Circuit Breaker Half-Open State & Recovery
// ---------------------------------------------------------------------------

#[test]
fn test_scp_circuit_breaker_half_open_recovery() {
    let mut scp = ScpEngine::new("scp-core-004");
    scp.register_backend(
        ScpBackendInstance {
            instance_id: "ausf-node".to_string(),
            nf_type: NfType::Ausf,
            fqdn: "ausf.5gc.local".to_string(),
            weight: 100,
            locality: "Site-A".to_string(),
        },
        2,  // 2 failures to trip
        20, // 20s recovery timeout
    );

    let t0 = 1700000000;

    // Trip circuit
    scp.report_instance_result("ausf-node", false, t0);
    scp.report_instance_result("ausf-node", false, t0 + 1);
    assert_eq!(
        scp.circuit_breakers.get("ausf-node").unwrap().state,
        CircuitState::Open
    );

    // During recovery timeout (t0 + 10s), requests blocked
    let cb = scp.circuit_breakers.get_mut("ausf-node").unwrap();
    assert!(!cb.allow_request(t0 + 10));

    // After recovery timeout (t0 + 25s), probes with HalfOpen
    assert!(cb.allow_request(t0 + 25));
    assert_eq!(cb.state, CircuitState::HalfOpen);

    // Probing request succeeds -> Circuit resets to Closed
    scp.report_instance_result("ausf-node", true, t0 + 26);
    assert_eq!(
        scp.circuit_breakers.get("ausf-node").unwrap().state,
        CircuitState::Closed
    );
}

// ---------------------------------------------------------------------------
// 5. Message Prioritization & Overload Throttling
// ---------------------------------------------------------------------------

#[test]
fn test_scp_message_prioritization_and_overload_throttling() {
    let mut scp = ScpEngine::new("scp-core-005");
    scp.register_backend(
        ScpBackendInstance {
            instance_id: "amf-node".to_string(),
            nf_type: NfType::Amf,
            fqdn: "amf.5gc.local".to_string(),
            weight: 100,
            locality: "DC-1".to_string(),
        },
        3,
        30,
    );

    // Enable overload mode (drop priority > 10)
    scp.overload_mode = true;
    scp.overload_threshold_priority = 10;

    // Emergency Session request (priority 0) -> Allowed through
    let emerg_req = ScpForwardRequest {
        consumer_nf_id: "ran-gnb-01".to_string(),
        target_nf_type: Some(NfType::Amf),
        target_api_root: None,
        target_instance_id: None,
        http_method: "POST".to_string(),
        path: "/namf-comm/v1/ue-contexts".to_string(),
        headers: HashMap::new(),
        payload: vec![],
        priority: 0, // Emergency / Highest priority
    };
    assert!(scp.forward_message(&emerg_req, 1700000000).is_ok());

    // Background Analytics request (priority 25) -> Throttled!
    let bg_req = ScpForwardRequest {
        consumer_nf_id: "nwdaf-01".to_string(),
        target_nf_type: Some(NfType::Amf),
        target_api_root: None,
        target_instance_id: None,
        http_method: "GET".to_string(),
        path: "/namf-evts/v1/subscriptions".to_string(),
        headers: HashMap::new(),
        payload: vec![],
        priority: 25, // Low priority
    };
    let res = scp.forward_message(&bg_req, 1700000000);
    assert_eq!(res, Err(ScpError::OverloadThrottled(25)));
}
