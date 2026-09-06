use toy_tcpip::sba_5g::{NfProfile, NfType, NrfRegistry, SbaMessageBus, SbaRequest};

#[test]
fn test_5g_sba_nrf_discovery_and_types() {
    let mut nrf = NrfRegistry::new();
    let amf = NfProfile {
        nf_instance_id: "amf-01".to_string(),
        nf_type: NfType::Amf,
        fqdn: "amf.core.local".to_string(),
        ip_address: "10.0.0.1".to_string(),
        services: vec!["namf-comm".to_string()],
        capacity: 100,
    };
    let smf = NfProfile {
        nf_instance_id: "smf-01".to_string(),
        nf_type: NfType::Smf,
        fqdn: "smf.core.local".to_string(),
        ip_address: "10.0.0.2".to_string(),
        services: vec!["nsmf-pdusession".to_string()],
        capacity: 80,
    };

    nrf.register_nf(amf);
    nrf.register_nf(smf);

    assert_eq!(nrf.discover_nf(NfType::Amf).len(), 1);
    assert_eq!(nrf.discover_nf(NfType::Smf).len(), 1);
    assert_eq!(nrf.discover_nf(NfType::Udm).len(), 0);

    assert!(nrf.deregister_nf("amf-01"));
    assert_eq!(nrf.discover_nf(NfType::Amf).len(), 0);
}

#[test]
fn test_5g_sba_message_bus_dispatch_and_errors() {
    let mut bus = SbaMessageBus::new();
    bus.nrf.register_nf(NfProfile {
        nf_instance_id: "udm-01".to_string(),
        nf_type: NfType::Udm,
        fqdn: "udm.core.local".to_string(),
        ip_address: "10.0.0.3".to_string(),
        services: vec!["nudm-sdm".to_string()],
        capacity: 100,
    });

    let valid_req = SbaRequest {
        service_name: "nudm-sdm".to_string(),
        method: "GET".to_string(),
        target_nf: NfType::Udm,
        resource_uri: "/nudm-sdm/v1/imsi-001".to_string(),
        payload_json: "{}".to_string(),
    };
    let resp = bus.dispatch(&valid_req);
    assert_eq!(resp.status_code, 200);
    assert!(resp.body_json.contains("subscriberData"));

    let missing_req = SbaRequest {
        service_name: "npcf-policy".to_string(),
        method: "GET".to_string(),
        target_nf: NfType::Pcf,
        resource_uri: "/npcf-smpolicycontrol/v1".to_string(),
        payload_json: "{}".to_string(),
    };
    let resp404 = bus.dispatch(&missing_req);
    assert_eq!(resp404.status_code, 404);
}
