//! Comprehensive Integration Tests for 3GPP Rel-18 NTN Regenerative Payload & Satellite Ephemeris Routing Engine.

use toy_tcpip::nr_ntn_regenerative::*;

#[test]
fn test_keplerian_orbit_propagation() {
    // 600 km circular LEO orbit, inclination 53.0 degrees
    let altitude_km = 600.0;
    let ecc = 0.001; // nearly circular
    let inc_deg = 53.0;
    let raan_deg = 45.0;
    let arg_perigee_deg = 0.0;
    let epoch_s = 0.0;

    let kep = KeplerianElements::new_leo(
        altitude_km,
        ecc,
        inc_deg,
        raan_deg,
        arg_perigee_deg,
        epoch_s,
    )
    .expect("Failed to construct Keplerian elements");

    // Theoretical orbital period: T = 2*pi*sqrt(a^3 / mu)
    let period = kep.orbital_period_s();
    // 600 km LEO period is approximately 5800 seconds (~96.6 minutes)
    assert!(
        period > 5700.0 && period < 5900.0,
        "Period should be ~5800s, got {:.2}s",
        period
    );

    // Propagate at epoch (t = 0)
    let (pos_eci_0, vel_eci_0) = kep.propagate_eci(0.0).expect("Propagation at t=0 failed");
    let r_0 = pos_eci_0.magnitude();
    let expected_a = (altitude_km * 1000.0) + EARTH_RADIUS_METERS;
    let diff = (r_0 - expected_a).abs();
    assert!(
        diff < 15_000.0,
        "Radius {:.2}m should match semi-major axis {:.2}m within eccentricity bound",
        r_0,
        expected_a
    );

    // Velocity magnitude for 600 km LEO is ~7.56 km/s
    let v_0 = vel_eci_0.magnitude();
    assert!(
        v_0 > 7_400.0 && v_0 < 7_700.0,
        "Orbital velocity {:.2} m/s should be ~7.56 km/s",
        v_0
    );

    // After one full orbital period, satellite should return to the same ECI position
    let (pos_eci_t, _) = kep
        .propagate_eci(period)
        .expect("Propagation at t=T failed");
    let pos_error = pos_eci_t.distance_to(&pos_eci_0);
    assert!(
        pos_error < 1.0,
        "Position after 1 orbit should match initial ECI position (error: {:.4}m)",
        pos_error
    );
}

#[test]
fn test_eci_to_ecef_transformation_and_earth_rotation() {
    let kep = KeplerianElements::new_leo(550.0, 0.0, 97.5, 0.0, 0.0, 0.0)
        .expect("Valid SSO Keplerian parameters");

    // At t = 0, ECI and ECEF align on Z rotation (theta = 0)
    let (pos_eci_0, _) = kep.propagate_eci(0.0).unwrap();
    let (pos_ecef_0, _) = kep.propagate_ecef(0.0).unwrap();
    assert!(
        pos_eci_0.distance_to(&pos_ecef_0) < 1e-6,
        "At t=0, ECI and ECEF must coincide"
    );

    // At t = 1000s, Earth has rotated theta = omega_E * 1000 rad
    let t = 1000.0;
    let (pos_eci_t, _) = kep.propagate_eci(t).unwrap();
    let (pos_ecef_t, vel_ecef_t) = kep.propagate_ecef(t).unwrap();

    // Magnitude of position vector must be invariant under frame rotation
    let mag_eci = pos_eci_t.magnitude();
    let mag_ecef = pos_ecef_t.magnitude();
    assert!(
        (mag_eci - mag_ecef).abs() < 1e-6,
        "Position magnitude must be invariant between ECI and ECEF"
    );

    // ECEF velocity magnitude must be non-zero and finite
    assert!(vel_ecef_t.magnitude() > 1000.0);
}

#[test]
fn test_ground_station_visibility_look_angles() {
    // Equator ground station at (0 lat, 0 lon)
    let gs = GroundStation::new("GS_Equator", 0.0, 0.0, 10.0);

    // Satellite directly overhead at 600 km altitude on equator
    let sat_pos_overhead = Vector3D::new(EARTH_RADIUS_METERS + 600_000.0, 0.0, 0.0);
    let (slant_m, el_deg, az_deg) = gs.compute_look_angles(&sat_pos_overhead);

    // Slant range should be exactly 600 km - 10m altitude
    assert!(
        (slant_m - 599_990.0).abs() < 1.0,
        "Slant range {:.2}m should be ~599,990m",
        slant_m
    );
    // Elevation should be ~90 degrees (zenith)
    assert!(
        (el_deg - 90.0).abs() < 0.1,
        "Zenith elevation should be 90°, got {:.2}°",
        el_deg
    );
    let _ = az_deg; // Azimuth at zenith is singular/indeterminate

    // Satellite on opposite side of Earth
    let sat_pos_antipodal = Vector3D::new(-(EARTH_RADIUS_METERS + 600_000.0), 0.0, 0.0);
    let (_, el_antipodal, _) = gs.compute_look_angles(&sat_pos_antipodal);
    assert!(
        el_antipodal < 0.0,
        "Antipodal satellite must have negative elevation ({:.2}°)",
        el_antipodal
    );
}

#[test]
fn test_payload_architecture_splits_and_local_breakout() {
    let mut engine = NtnRegenerativeEngine::new("NTN_LEO_Mesh");

    let kep = KeplerianElements::new_leo(600.0, 0.0, 53.0, 0.0, 0.0, 0.0).unwrap();
    let mut sat1 = SatelliteNode::new("SAT_1", PayloadArchitecture::FullGnbOnboard, kep.clone());

    // Register two UEs under SAT_1 in different beams
    let beam1 = SatelliteBeam::new_earth_moving(101, 30.0, 7.0);
    let beam2 = SatelliteBeam::new_earth_moving(102, 30.0, 7.0);
    sat1.add_beam(beam1);
    sat1.add_beam(beam2);
    sat1.attach_ue("UE_Alice", 101);
    sat1.attach_ue("UE_Bob", 102);

    engine.register_satellite(sat1);

    // Packet from Alice to Bob (both on SAT_1)
    let mut packet = SpacePacket::new(
        1001,
        "UE_Alice",
        "UE_Bob",
        "SAT_1",
        "SAT_1",
        SpaceQosPriority::MissionCritical,
        vec![0xAA, 0xBB, 0xCC],
    );

    let decision = engine.process_packet("SAT_1", &mut packet, None, 0.0);
    match decision {
        ForwardingDecision::LocalBreakout {
            egress_beam_id,
            processing_delay_ms,
        } => {
            assert_eq!(egress_beam_id, 102);
            assert_eq!(processing_delay_ms, 2.5);
            assert_eq!(packet.hop_count, 1);
        }
        other => panic!("Expected LocalBreakout, got {:?}", other),
    }

    // Now test with Transparent Transponder (cannot do local breakout)
    let mut sat2 = SatelliteNode::new("SAT_2", PayloadArchitecture::TransparentTransponder, kep);
    sat2.attach_ue("UE_Charlie", 201);
    sat2.attach_ue("UE_Dave", 202);
    engine.register_satellite(sat2);

    // Add ground gateway visible from SAT_2
    let gw = GroundStation::new("GW_Tokyo", 0.0, 0.0, 0.0);
    engine.register_ground_station(gw);

    let mut packet2 = SpacePacket::new(
        1002,
        "UE_Charlie",
        "UE_Dave",
        "SAT_2",
        "SAT_2",
        SpaceQosPriority::InteractiveData,
        vec![1, 2, 3],
    );

    let decision2 = engine.process_packet("SAT_2", &mut packet2, Some("GW_Tokyo"), 0.0);
    match decision2 {
        ForwardingDecision::FeederDownlink {
            gateway_id,
            feeder_rtt_ms,
        } => {
            assert_eq!(gateway_id, "GW_Tokyo");
            assert!(
                feeder_rtt_ms > 3.0 && feeder_rtt_ms < 15.0,
                "Feeder RTT should be ~4 ms for 600 km LEO, got {:.2} ms",
                feeder_rtt_ms
            );
        }
        other => panic!("Expected FeederDownlink, got {:?}", other),
    }
}

#[test]
fn test_isl_mesh_routing_dijkstra_shortest_path() {
    let mut engine = NtnRegenerativeEngine::new("Constellation_Mesh");

    // Create 4 satellites in the same orbital plane spaced 10 degrees apart
    for i in 1..=4 {
        let kep = KeplerianElements::new_leo(550.0, 0.0, 53.0, 0.0, 0.0, 0.0).unwrap();
        // Modify mean anomaly to space them around the orbit
        let mut kep_shifted = kep;
        kep_shifted.mean_anomaly_epoch_rad = ((i - 1) as f64) * 10.0f64.to_radians();

        let sat = SatelliteNode::new(
            &format!("SAT_{}", i),
            PayloadArchitecture::FullGnbOnboard,
            kep_shifted,
        );
        engine.register_satellite(sat);
    }

    // Connect ISLs: SAT_1 <-> SAT_2 <-> SAT_3 <-> SAT_4
    engine.add_isl_link(IslLink::new(
        "L12",
        "SAT_1",
        "SAT_2",
        IslType::OpticalLaser,
        100.0,
    ));
    engine.add_isl_link(IslLink::new(
        "L23",
        "SAT_2",
        "SAT_3",
        IslType::OpticalLaser,
        100.0,
    ));
    engine.add_isl_link(IslLink::new(
        "L34",
        "SAT_3",
        "SAT_4",
        IslType::OpticalLaser,
        100.0,
    ));

    // Also add a direct cross-link SAT_1 <-> SAT_3
    engine.add_isl_link(IslLink::new(
        "L13",
        "SAT_1",
        "SAT_3",
        IslType::OpticalLaser,
        50.0,
    ));

    // Compute route from SAT_1 to SAT_3
    let (path, delay_ms) = engine
        .compute_isl_route("SAT_1", "SAT_3", 0.0)
        .expect("Route search failed");

    // Direct link L13 should be selected since Euclidean triangle inequality guarantees
    // direct distance SAT_1 <-> SAT_3 < distance(SAT_1, SAT_2) + distance(SAT_2, SAT_3)
    assert_eq!(path, vec!["SAT_1", "SAT_3"]);
    assert!(
        delay_ms > 0.0 && delay_ms < 20.0,
        "Direct ISL delay should be ~4-15 ms, got {:.2} ms",
        delay_ms
    );

    // Compute route from SAT_1 to SAT_4: should be SAT_1 -> SAT_3 -> SAT_4
    let (path_14, _) = engine
        .compute_isl_route("SAT_1", "SAT_4", 0.0)
        .expect("Route to SAT_4 failed");
    assert_eq!(path_14, vec!["SAT_1", "SAT_3", "SAT_4"]);
}

#[test]
fn test_isl_dynamic_link_failure_and_reroute() {
    let mut engine = NtnRegenerativeEngine::new("Fault_Tolerant_ISL");

    for i in 1..=3 {
        let mut kep = KeplerianElements::new_leo(600.0, 0.0, 0.0, 0.0, 0.0, 0.0).unwrap();
        kep.mean_anomaly_epoch_rad = ((i - 1) as f64) * 5.0f64.to_radians();
        let sat = SatelliteNode::new(
            &format!("SAT_{}", i),
            PayloadArchitecture::FullGnbOnboard,
            kep,
        );
        engine.register_satellite(sat);
    }

    // Links: SAT_1 - SAT_2, SAT_2 - SAT_3, SAT_1 - SAT_3 (direct)
    engine.add_isl_link(IslLink::new(
        "L12",
        "SAT_1",
        "SAT_2",
        IslType::OpticalLaser,
        100.0,
    ));
    engine.add_isl_link(IslLink::new(
        "L23",
        "SAT_2",
        "SAT_3",
        IslType::OpticalLaser,
        100.0,
    ));
    engine.add_isl_link(IslLink::new(
        "L13_direct",
        "SAT_1",
        "SAT_3",
        IslType::OpticalLaser,
        100.0,
    ));

    // Initially direct
    let (init_path, _) = engine.compute_isl_route("SAT_1", "SAT_3", 0.0).unwrap();
    assert_eq!(init_path, vec!["SAT_1", "SAT_3"]);

    // Simulate optical link disruption (solar conjunction or gimbal limit)
    engine
        .set_isl_status("L13_direct", IslStatus::Down)
        .expect("Failed to set ISL status");

    // Now path must detour through SAT_2
    let (rerouted_path, _) = engine
        .compute_isl_route("SAT_1", "SAT_3", 0.0)
        .expect("Reroute failed");
    assert_eq!(rerouted_path, vec!["SAT_1", "SAT_2", "SAT_3"]);

    // Test packet TTL handling
    let mut packet = SpacePacket::new(
        999,
        "UE_1",
        "UE_2",
        "SAT_1",
        "SAT_3",
        SpaceQosPriority::BestEffort,
        vec![0],
    );
    packet.hop_count = 16; // At max hops

    let decision = engine.process_packet("SAT_1", &mut packet, None, 0.0);
    match decision {
        ForwardingDecision::Drop { reason } => {
            assert!(reason.contains("TTL exceeded"));
        }
        _ => panic!("Expected Drop due to TTL"),
    }
}

#[test]
fn test_earth_moving_vs_earth_fixed_beams() {
    // 1. Earth-moving beam: 30 km radius, ground track speed 7.0 km/s
    let moving_beam = SatelliteBeam::new_earth_moving(1, 30.0, 7.0);
    // At center: dwell time = 30 / 7.0 = 4.28 seconds
    let dwell_center = moving_beam.calculate_dwell_time_s(0.0);
    assert!(
        (dwell_center - (30.0 / 7.0)).abs() < 1e-3,
        "Dwell time at center should be ~4.29s, got {:.2}s",
        dwell_center
    );

    // At edge (30 km): dwell time = 0.0s
    let dwell_edge = moving_beam.calculate_dwell_time_s(30.0);
    assert_eq!(dwell_edge, 0.0);

    // Beyond edge (35 km): dwell time = 0.0s
    let dwell_beyond = moving_beam.calculate_dwell_time_s(35.0);
    assert_eq!(dwell_beyond, 0.0);

    // 2. Earth-fixed steerable beam
    let target_center_ecef = Vector3D::new(EARTH_RADIUS_METERS, 0.0, 0.0);
    let fixed_beam = SatelliteBeam::new_earth_fixed(2, target_center_ecef, 45.0);

    // Satellite at zenith above target: steering angle should be 0.0 deg
    let sat_zenith = Vector3D::new(EARTH_RADIUS_METERS + 500_000.0, 0.0, 0.0);
    let steer_zenith = fixed_beam
        .evaluate_steering(&sat_zenith)
        .expect("Zenith steering evaluation failed");
    assert!(
        steer_zenith < 1e-4,
        "Steering angle at zenith should be 0°, got {:.2}°",
        steer_zenith
    );

    // Satellite displaced along Y axis by 300 km: steering angle should increase
    let sat_displaced = Vector3D::new(EARTH_RADIUS_METERS + 500_000.0, 300_000.0, 0.0);
    let steer_displaced = fixed_beam
        .evaluate_steering(&sat_displaced)
        .expect("Displaced steering evaluation failed");
    assert!(
        steer_displaced > 15.0 && steer_displaced < 40.0,
        "Steering angle should be ~25-35°, got {:.2}°",
        steer_displaced
    );

    // Satellite displaced too far (1500 km): steering angle exceeds 45 deg limit
    let sat_far = Vector3D::new(EARTH_RADIUS_METERS + 500_000.0, 1_500_000.0, 0.0);
    let steer_err = fixed_beam.evaluate_steering(&sat_far);
    match steer_err {
        Err(NtnRegenerativeError::SteeringLimitExceeded { angle_deg, max_deg }) => {
            assert_eq!(max_deg, 45.0);
            assert!(angle_deg > 45.0);
        }
        other => panic!("Expected SteeringLimitExceeded, got {:?}", other),
    }
}
