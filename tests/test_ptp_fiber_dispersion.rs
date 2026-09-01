use toy_tcpip::ptp_fiber_dispersion::{
    FiberThermalDispersionModel, FiberType, OpticalFiberLink, SPEED_OF_LIGHT_VACUUM,
    WavelengthConfig,
};

#[test]
fn test_ptp_fiber_thermal_drift_and_bidi_dispersion_integration() {
    assert_eq!(SPEED_OF_LIGHT_VACUUM, 299_792_458.0);

    let link_dual = OpticalFiberLink {
        link_id: "Backhaul-Dual-G652".to_string(),
        fiber_type: FiberType::G652,
        length_km: 25.0, // 25 km
        reference_temp_c: 25.0,
        wavelength_cfg: WavelengthConfig::DualStrandSameWavelength {
            wavelength_nm: 1310.0,
        },
    };

    let model_dual = FiberThermalDispersionModel::new(link_dual);

    // Dual strand has symmetric wavelength -> zero dispersion asymmetry
    let comp_dual_0c = model_dual.calculate_compensation(0.0);
    assert_eq!(comp_dual_0c.chromatic_dispersion_asym_ps, 0);
    assert_eq!(comp_dual_0c.total_delay_asymmetry_ps, 0);

    // Temperature drop by 25°C: 25km * 37 ps/(km*°C) * (-25°C) = -23125 ps
    assert_eq!(comp_dual_0c.thermal_drift_ps, -23125);

    // Single strand BiDi 1310/1550 nm on G.655 NZDSF
    let link_bidi = OpticalFiberLink {
        link_id: "Fronthaul-BiDi-G655".to_string(),
        fiber_type: FiberType::G655,
        length_km: 5.0,
        reference_temp_c: 20.0,
        wavelength_cfg: WavelengthConfig::SingleStrandBiDi {
            forward_wavelength_nm: 1310.0,
            reverse_wavelength_nm: 1550.0,
        },
    };

    let model_bidi = FiberThermalDispersionModel::new(link_bidi);
    let comp_bidi = model_bidi.calculate_compensation(20.0);

    assert_eq!(comp_bidi.thermal_drift_ps, 0);
    assert!(comp_bidi.chromatic_dispersion_asym_ps != 0);
    assert_eq!(
        comp_bidi.total_delay_asymmetry_ps,
        comp_bidi.chromatic_dispersion_asym_ps
    );
}
