//! Integration tests for 3GPP Rel-17 5G NR Positioning and Location Services (LCS) Protocol Engine.

use std::collections::HashMap;
use toy_tcpip::nr_positioning_lcs::*;

#[test]
fn test_coordinate_transforms_and_uncertainty() {
    // 1. Setup local tangent plane origin at Taipei 101 area
    let origin_wgs84 = Wgs84Point::new(25.033964, 121.564468, 100.0);
    let transformer = CoordinateTransformer::new(origin_wgs84);

    // 2. Test origin maps to (0, 0, 0)
    let origin_enu = transformer.wgs84_to_enu(&origin_wgs84);
    assert!(origin_enu.east.abs() < 1e-4);
    assert!(origin_enu.north.abs() < 1e-4);
    assert!(origin_enu.up.abs() < 1e-4);

    // 3. Test arbitrary point: 500m East, 300m North, 20m Up
    let target_enu = EnuPoint::new(500.0, 300.0, 20.0);
    let target_wgs84 = transformer.enu_to_wgs84(&target_enu);

    // Verify distance from origin in ENU
    let dist = origin_enu.distance_to(&target_enu);
    let expected_dist = (500.0_f64.powi(2) + 300.0_f64.powi(2) + 20.0_f64.powi(2)).sqrt();
    assert!((dist - expected_dist).abs() < 1e-6);

    // 4. Test bidirectional round-trip consistency (WGS-84 <-> ENU)
    let roundtrip_enu = transformer.wgs84_to_enu(&target_wgs84);
    assert!((roundtrip_enu.east - target_enu.east).abs() < 1e-3);
    assert!((roundtrip_enu.north - target_enu.north).abs() < 1e-3);
    assert!((roundtrip_enu.up - target_enu.up).abs() < 1e-3);

    // 5. Verify uncertainty ellipse structure
    let uncertainty = UncertaintyEllipse {
        semi_major_m: 2.5,
        semi_minor_m: 1.8,
        orientation_deg: 35.0,
        vertical_uncertainty_m: 3.0,
        confidence_percent: 95.0,
    };
    assert_eq!(uncertainty.confidence_percent, 95.0);
    assert!(uncertainty.semi_major_m >= uncertainty.semi_minor_m);
}

#[test]
fn test_multi_rtt_trilateration_solver() {
    let origin = Wgs84Point::new(37.4220, -122.0841, 10.0);
    let transformer = CoordinateTransformer::new(origin);

    // Setup 4 TRPs in a macro/micro layout around the origin
    let mut trps = HashMap::new();
    let trp_configs = [
        (101, EnuPoint::new(0.0, 0.0, 30.0)),
        (102, EnuPoint::new(600.0, 50.0, 35.0)),
        (103, EnuPoint::new(50.0, 700.0, 28.0)),
        (104, EnuPoint::new(550.0, 650.0, 32.0)),
    ];

    for (trp_id, pos_enu) in trp_configs {
        trps.insert(
            trp_id,
            TrpInfo {
                trp_id,
                gnb_id: 1,
                pci: (trp_id % 500) as u16,
                position_enu: pos_enu,
                position_wgs84: transformer.enu_to_wgs84(&pos_enu),
                carrier_freq_mhz: 3500.0,
            },
        );
    }

    // True UE position
    let true_ue = EnuPoint::new(280.0, 320.0, 1.8);

    // Generate exact Multi-RTT measurements: RTT = 2 * d / c
    let mut measurements = Vec::new();
    for (trp_id, pos_enu) in trp_configs {
        let distance = true_ue.distance_to(&pos_enu);
        let total_rtt = (2.0 * distance) / SPEED_OF_LIGHT_M_S;

        // Split RTT into T_gnb and T_ue (e.g. 1us for gNodeB, remainder for UE)
        let t_gnb = 1.0e-6;
        let t_ue = total_rtt - t_gnb;

        measurements.push(MultiRttMeasurement {
            trp_id,
            t_gnb_rx_tx_s: t_gnb,
            t_ue_rx_tx_s: t_ue,
            dl_prs_rsrp_dbm: -85.0,
        });
    }

    // Solve 3D position
    let estimate = MultiRttSolver::solve(&measurements, &trps, &transformer).unwrap();

    // Verify sub-centimeter convergence to true position
    let error_m = estimate.position_enu.distance_to(&true_ue);
    assert!(
        error_m < 0.05,
        "Multi-RTT error was {}m, expected < 0.05m",
        error_m
    );
    assert_eq!(estimate.num_measurements_used, 4);
    assert!(estimate.uncertainty.semi_major_m > 0.0);
}

#[test]
fn test_dl_tdoa_hyperbolic_multilateration() {
    let origin = Wgs84Point::new(48.8584, 2.2945, 35.0);
    let transformer = CoordinateTransformer::new(origin);

    // Setup reference TRP (0) and 3 neighbor TRPs
    let mut trps = HashMap::new();
    let ref_trp_id = 1;
    let trp_coords = [
        (1, EnuPoint::new(0.0, 0.0, 80.0)), // Reference TRP (macro tower)
        (2, EnuPoint::new(500.0, 0.0, 15.0)), // Neighbor TRP 1 (street micro)
        (3, EnuPoint::new(0.0, 600.0, 45.0)), // Neighbor TRP 2 (mid-rise)
        (4, EnuPoint::new(450.0, 550.0, 110.0)), // Neighbor TRP 3 (tall mast)
    ];

    for (trp_id, pos_enu) in trp_coords {
        trps.insert(
            trp_id,
            TrpInfo {
                trp_id,
                gnb_id: 2,
                pci: trp_id as u16,
                position_enu: pos_enu,
                position_wgs84: transformer.enu_to_wgs84(&pos_enu),
                carrier_freq_mhz: 2600.0,
            },
        );
    }

    // True UE position
    let true_ue = EnuPoint::new(180.0, 220.0, 2.0);
    let d_ref = true_ue.distance_to(&trp_coords[0].1);

    // Generate DL RSTD measurements: RSTD_i = (d_i - d_ref) / c
    let mut rstd_measurements = Vec::new();
    for &(trp_id, pos_enu) in &trp_coords[1..] {
        let d_i = true_ue.distance_to(&pos_enu);
        let delta_d = d_i - d_ref;
        let rstd_s = delta_d / SPEED_OF_LIGHT_M_S;

        rstd_measurements.push(DlRstdMeasurement {
            neighbor_trp_id: trp_id,
            reference_trp_id: ref_trp_id,
            rstd_seconds: rstd_s,
            search_window_s: 1.0e-6,
            rsrp_dbm: -90.0,
        });
    }

    // Solve DL-TDOA
    let estimate =
        DlTdoaSolver::solve(ref_trp_id, &rstd_measurements, &trps, &transformer).unwrap();

    // Verify sub-meter convergence
    let error_m = estimate.position_enu.distance_to(&true_ue);
    assert!(
        error_m < 0.50,
        "DL-TDOA error was {}m, expected < 0.50m",
        error_m
    );
    assert_eq!(estimate.num_measurements_used, 3);
}

#[test]
fn test_aoa_aod_triangulation_solver() {
    let origin = Wgs84Point::new(51.5074, -0.1278, 15.0);
    let transformer = CoordinateTransformer::new(origin);

    // Setup 3 TRPs with angular coverage
    let mut trps = HashMap::new();
    let trp_data = [
        (1, EnuPoint::new(0.0, 0.0, 20.0)),
        (2, EnuPoint::new(400.0, 0.0, 25.0)),
        (3, EnuPoint::new(200.0, 400.0, 22.0)),
    ];

    for (trp_id, pos) in trp_data {
        trps.insert(
            trp_id,
            TrpInfo {
                trp_id,
                gnb_id: 3,
                pci: trp_id as u16,
                position_enu: pos,
                position_wgs84: transformer.enu_to_wgs84(&pos),
                carrier_freq_mhz: 3700.0,
            },
        );
    }

    // Target UE position
    let target_ue = EnuPoint::new(180.0, 150.0, 5.0);

    // Compute exact bearing angles (Azimuth clockwise from North, Elevation up from horizon)
    let mut angles = Vec::new();
    for (trp_id, pos) in trp_data {
        let de = target_ue.east - pos.east;
        let dn = target_ue.north - pos.north;
        let du = target_ue.up - pos.up;

        // Azimuth from North clockwise: atan2(East, North)
        let az_rad = de.atan2(dn);
        let az_deg = (az_rad.to_degrees() + 360.0) % 360.0;

        // Elevation: atan2(Up, horizontal_dist)
        let horiz_dist = (de * de + dn * dn).sqrt();
        let el_deg = du.atan2(horiz_dist).to_degrees();

        angles.push(AngleMeasurement {
            trp_id,
            azimuth_deg: az_deg,
            elevation_deg: el_deg,
            rsrp_dbm: -80.0,
        });
    }

    let estimate = AoATriangulationSolver::solve(&angles, &trps, &transformer).unwrap();

    let error_m = estimate.position_enu.distance_to(&target_ue);
    assert!(
        error_m < 0.10,
        "Triangulation error was {}m, expected < 0.10m",
        error_m
    );
}

#[test]
fn test_lpp_and_nrppa_signaling_engine() {
    // -----------------------------------------------------------------------
    // 1. LPP (TS 37.355) Handshake
    // -----------------------------------------------------------------------
    let mut lpp_mgr = LppTransactionManager::new(vec![
        LppPositioningMethod::MultiRtt,
        LppPositioningMethod::DlTdoa,
        LppPositioningMethod::UlAoa,
    ]);

    // Add TRPs to assistance data catalog
    let origin = Wgs84Point::new(35.6762, 139.6503, 10.0);
    let _transformer = CoordinateTransformer::new(origin);

    lpp_mgr.known_trps.push(TrpInfo {
        trp_id: 1,
        gnb_id: 10,
        pci: 100,
        position_enu: EnuPoint::new(0.0, 0.0, 30.0),
        position_wgs84: origin,
        carrier_freq_mhz: 3500.0,
    });

    // Request Capabilities
    let req_cap = lpp_mgr.create_request_capabilities();
    let tid = match req_cap {
        LppMessageType::RequestCapabilities { transaction_id } => transaction_id,
        _ => panic!("Expected RequestCapabilities"),
    };

    let prov_cap = lpp_mgr.handle_request_capabilities(tid);
    match prov_cap {
        LppMessageType::ProvideCapabilities {
            transaction_id,
            supported_methods,
        } => {
            assert_eq!(transaction_id, tid);
            assert_eq!(supported_methods.len(), 3);
            assert!(supported_methods.contains(&LppPositioningMethod::MultiRtt));
            assert!(supported_methods.contains(&LppPositioningMethod::DlTdoa));
        }
        _ => panic!("Expected ProvideCapabilities"),
    }

    // Request Assistance Data
    let prov_assist = lpp_mgr.handle_request_assistance_data(tid);
    match prov_assist {
        LppMessageType::ProvideAssistanceData {
            transaction_id,
            trp_catalog,
        } => {
            assert_eq!(transaction_id, tid);
            assert_eq!(trp_catalog.len(), 1);
            assert_eq!(trp_catalog[0].trp_id, 1);
        }
        _ => panic!("Expected ProvideAssistanceData"),
    }

    // -----------------------------------------------------------------------
    // 2. NRPPa (TS 38.455) Signaling
    // -----------------------------------------------------------------------
    let mut nrppa = NrppaEngine::new(10);
    nrppa.managed_trps = lpp_mgr.known_trps.clone();

    // TRP Information Request
    let trp_req = NrppaMessage::TrpInformationRequest {
        transaction_id: 1001,
        gnb_id: 10,
    };
    let trp_resp = nrppa.handle_message(trp_req).unwrap();
    match trp_resp {
        NrppaMessage::TrpInformationResponse {
            transaction_id,
            trps,
        } => {
            assert_eq!(transaction_id, 1001);
            assert_eq!(trps.len(), 1);
            assert_eq!(trps[0].trp_id, 1);
        }
        _ => panic!("Expected TrpInformationResponse"),
    }

    // UL SRS Measurement Request
    let meas_req = NrppaMessage::UlMeasurementRequest {
        transaction_id: 1002,
        ue_rnti: 0x4001,
        measurement_type: "UL-SRS-AoA".to_string(),
    };
    let meas_resp = nrppa.handle_message(meas_req).unwrap();
    match meas_resp {
        NrppaMessage::UlMeasurementResponse {
            transaction_id,
            ue_rnti,
            t_gnb_rx_tx_s,
            aoa_azimuth_deg,
            aoa_elevation_deg,
        } => {
            assert_eq!(transaction_id, 1002);
            assert_eq!(ue_rnti, 0x4001);
            assert!(t_gnb_rx_tx_s > 0.0);
            assert_eq!(aoa_azimuth_deg, 135.0);
            assert_eq!(aoa_elevation_deg, 15.0);
        }
        _ => panic!("Expected UlMeasurementResponse"),
    }
}
