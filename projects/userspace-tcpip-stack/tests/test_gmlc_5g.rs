//! Integration tests for 3GPP TS 29.515 / TS 23.273 5G Gateway Mobile Location Centre (GMLC) Engine.

use toy_tcpip::gmlc_5g::*;
use toy_tcpip::lmf_5g::GeographicCoordinates;

// ---------------------------------------------------------------------------
// 1. Commercial ProvideLocation Happy Path with Notification
// ---------------------------------------------------------------------------

#[test]
fn test_gmlc_commercial_provide_location_happy_path() {
    let mut gmlc = GmlcEngine::new("gmlc-core-01");
    let client_id = "ride-share-app-01";
    let gpsi = "msisdn-886912345678";

    gmlc.register_client(client_id, LcsClientClass::CommercialWithNotification);
    gmlc.set_privacy_policy(gpsi, PrivacyConsent::AllowedWithNotification);
    gmlc.update_serving_amf(gpsi, "amf-region-01-instance-02");

    let req = ProvideLocationRequest {
        client_id: client_id.to_string(),
        client_class: LcsClientClass::CommercialWithNotification,
        target_gpsi: gpsi.to_string(),
        target_supi: None,
        requested_qos: None,
        timestamp_epoch_s: 1700000000,
    };

    let mock_coords = GeographicCoordinates {
        latitude: 25.0339,
        longitude: 121.5644,
        altitude_m: Some(20.0),
        uncertainty_horizontal_m: 2.5,
        uncertainty_vertical_m: Some(5.0),
        confidence_percent: 95,
    };

    let resp = gmlc
        .provide_location(&req, mock_coords.clone())
        .expect("Location inquiry failed");

    assert_eq!(resp.target_gpsi, gpsi);
    assert_eq!(resp.serving_amf_id, "amf-region-01-instance-02");
    assert_eq!(resp.coordinates, mock_coords);
    assert!(resp.privacy_notified);
}

// ---------------------------------------------------------------------------
// 2. Subscriber Privacy Opt-Out Rejection
// ---------------------------------------------------------------------------

#[test]
fn test_gmlc_subscriber_opt_out_privacy_rejection() {
    let mut gmlc = GmlcEngine::new("gmlc-core-02");
    let client_id = "ad-tech-client";
    let gpsi = "msisdn-886987654321";

    gmlc.register_client(client_id, LcsClientClass::CommercialWithNotification);
    // Subscriber explicitly disallows location tracking
    gmlc.set_privacy_policy(gpsi, PrivacyConsent::Disallowed);
    gmlc.update_serving_amf(gpsi, "amf-01");

    let req = ProvideLocationRequest {
        client_id: client_id.to_string(),
        client_class: LcsClientClass::CommercialWithNotification,
        target_gpsi: gpsi.to_string(),
        target_supi: None,
        requested_qos: None,
        timestamp_epoch_s: 1700000000,
    };

    let mock_coords = GeographicCoordinates {
        latitude: 25.0339,
        longitude: 121.5644,
        altitude_m: None,
        uncertainty_horizontal_m: 10.0,
        uncertainty_vertical_m: None,
        confidence_percent: 90,
    };

    let res = gmlc.provide_location(&req, mock_coords);
    assert_eq!(
        res,
        Err(GmlcError::PrivacyCheckFailed("Subscriber opted out of LCS"))
    );
}

// ---------------------------------------------------------------------------
// 3. Emergency Client Bypasses Privacy Opt-Out (E911 Priority)
// ---------------------------------------------------------------------------

#[test]
fn test_gmlc_emergency_client_bypasses_privacy_disallow() {
    let mut gmlc = GmlcEngine::new("gmlc-core-03");
    let emergency_psap = "psap-e911-dispatch";
    let gpsi = "msisdn-886987654321";

    gmlc.register_client(emergency_psap, LcsClientClass::EmergencyServices);
    // Subscriber disallowed commercial tracking
    gmlc.set_privacy_policy(gpsi, PrivacyConsent::Disallowed);
    gmlc.update_serving_amf(gpsi, "amf-01");

    let req = ProvideLocationRequest {
        client_id: emergency_psap.to_string(),
        client_class: LcsClientClass::EmergencyServices,
        target_gpsi: gpsi.to_string(),
        target_supi: None,
        requested_qos: None,
        timestamp_epoch_s: 1700000000,
    };

    let mock_coords = GeographicCoordinates {
        latitude: 25.0339,
        longitude: 121.5644,
        altitude_m: Some(10.0),
        uncertainty_horizontal_m: 1.0,
        uncertainty_vertical_m: Some(2.0),
        confidence_percent: 99,
    };

    // Emergency client must succeed regardless of privacy policy
    let resp = gmlc
        .provide_location(&req, mock_coords)
        .expect("Emergency location must bypass privacy checks");

    assert_eq!(resp.target_gpsi, gpsi);
    assert!(!resp.privacy_notified); // Emergency calls do not alert subscriber
}

// ---------------------------------------------------------------------------
// 4. Deferred Geo-Fencing Event Triggers
// ---------------------------------------------------------------------------

#[test]
fn test_gmlc_deferred_geo_fencing_event_triggers() {
    let mut gmlc = GmlcEngine::new("gmlc-core-04");
    let client_id = "fleet-logistics-01";
    let gpsi = "gpsi-fleet-vehicle-99";

    gmlc.register_client(client_id, LcsClientClass::ValueAddedWhitelisted);

    // Fence: Taipei 101 center, 500m radius
    let fence = CircularGeoFence {
        center_lat: 25.0339,
        center_lon: 121.5644,
        radius_m: 500.0,
    };

    let sub_id = gmlc
        .create_deferred_subscription(client_id, gpsi, fence, GeoFenceEvent::EnteringArea)
        .expect("Subscription failed");

    // Position 1: Outside fence (~2000m away)
    let triggered1 = gmlc.evaluate_geo_fence_events(gpsi, 25.0500, 121.5644);
    assert!(triggered1.is_empty());

    // Position 2: Enters fence (~100m from center)
    let triggered2 = gmlc.evaluate_geo_fence_events(gpsi, 25.0340, 121.5645);
    assert_eq!(triggered2, vec![sub_id.clone()]); // Fired EnteringArea!

    // Position 3: Still inside fence
    let triggered3 = gmlc.evaluate_geo_fence_events(gpsi, 25.0341, 121.5646);
    assert!(triggered3.is_empty()); // Already inside, not entering again
}

// ---------------------------------------------------------------------------
// 5. Unauthorized Client Rejection
// ---------------------------------------------------------------------------

#[test]
fn test_gmlc_unauthorized_client_rejection() {
    let gmlc = GmlcEngine::new("gmlc-core-05");

    let req = ProvideLocationRequest {
        client_id: "unknown-rogue-client".to_string(),
        client_class: LcsClientClass::CommercialWithNotification,
        target_gpsi: "msisdn-12345".to_string(),
        target_supi: None,
        requested_qos: None,
        timestamp_epoch_s: 1700000000,
    };

    let mock_coords = GeographicCoordinates {
        latitude: 0.0,
        longitude: 0.0,
        altitude_m: None,
        uncertainty_horizontal_m: 100.0,
        uncertainty_vertical_m: None,
        confidence_percent: 50,
    };

    let res = gmlc.provide_location(&req, mock_coords);
    assert_eq!(res, Err(GmlcError::UnauthorizedClient));
}
