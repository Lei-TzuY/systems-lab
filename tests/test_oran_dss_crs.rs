//! Integration tests for O-RAN WG4 Open Fronthaul Dynamic Spectrum Sharing (DSS) & CRS Rate Matching Engine.

use toy_tcpip::oran_dss_crs::*;

#[test]
fn test_lte_crs_re_coordinates_normal_cp_1port() {
    // Cell ID = 0, v_shift = 0, 1 antenna port
    let cfg = LteCrsConfig::new(0, 50, LteAntennaPorts::OnePort, LteCyclicPrefix::Normal, 0)
        .expect("Valid LTE CRS configuration should succeed");

    let filter = CrsPunctureFilter::new(cfg, NrSubcarrierSpacing::Scs15kHz);
    let mask = filter
        .generate_prb_mask(0, 0, 0)
        .expect("PRB 0 mask generation should succeed");

    // Port 0, v_shift 0:
    // Symbol 0: subcarriers (0 + 0) % 6 = 0 -> subcarriers 0, 6
    assert!(mask.is_punctured(0, 0));
    assert!(mask.is_punctured(0, 6));
    assert!(!mask.is_punctured(0, 1));
    assert_eq!(mask.symbol_masks[0].count_ones(), 2);

    // Symbol 4: subcarriers (3 + 0) % 6 = 3 -> subcarriers 3, 9
    assert!(mask.is_punctured(4, 3));
    assert!(mask.is_punctured(4, 9));
    assert_eq!(mask.symbol_masks[4].count_ones(), 2);

    // Symbol 7: subcarriers (0 + 0) % 6 = 0 -> subcarriers 0, 6
    assert_eq!(mask.symbol_masks[7].count_ones(), 2);

    // Symbol 11: subcarriers (3 + 0) % 6 = 3 -> subcarriers 3, 9
    assert_eq!(mask.symbol_masks[11].count_ones(), 2);

    // Symbols without CRS: 1, 2, 3, 5, 6, 8, 9, 10, 12, 13
    for &sym in &[1, 2, 3, 5, 6, 8, 9, 10, 12, 13] {
        assert_eq!(mask.symbol_masks[sym], 0);
    }

    // Total punctured: 4 symbols * 2 REs = 8 REs
    assert_eq!(mask.total_punctured_res(), 8);
    assert_eq!(mask.available_pdsch_res(), 168 - 8);
}

#[test]
fn test_lte_crs_re_coordinates_normal_cp_2ports() {
    // Cell ID = 1, v_shift = 1, 2 antenna ports
    let cfg = LteCrsConfig::new(1, 50, LteAntennaPorts::TwoPorts, LteCyclicPrefix::Normal, 0)
        .expect("Valid 2-port config");

    let filter = CrsPunctureFilter::new(cfg, NrSubcarrierSpacing::Scs15kHz);
    let mask = filter.generate_prb_mask(0, 0, 0).unwrap();

    // With 2 ports:
    // Symbols 0 and 7: Port 0 has v=0 -> (0 + 1)%6 = 1; Port 1 has v=3 -> (3 + 1)%6 = 4.
    // Subcarriers 1, 4, 7, 10 are punctured (4 REs per symbol)
    assert!(mask.is_punctured(0, 1));
    assert!(mask.is_punctured(0, 4));
    assert!(mask.is_punctured(0, 7));
    assert!(mask.is_punctured(0, 10));
    assert_eq!(mask.symbol_masks[0].count_ones(), 4);

    // Symbols 4 and 11: Port 0 has v=3 -> 4; Port 1 has v=0 -> 1.
    assert_eq!(mask.symbol_masks[4].count_ones(), 4);
    assert_eq!(mask.symbol_masks[7].count_ones(), 4);
    assert_eq!(mask.symbol_masks[11].count_ones(), 4);

    // Total punctured for 2 ports: 4 symbols * 4 REs = 16 REs
    assert_eq!(mask.total_punctured_res(), 16);
    assert_eq!(mask.available_pdsch_res(), 152);
}

#[test]
fn test_lte_crs_re_coordinates_normal_cp_4ports() {
    // Cell ID = 2, v_shift = 2, 4 antenna ports
    let cfg = LteCrsConfig::new(
        2,
        50,
        LteAntennaPorts::FourPorts,
        LteCyclicPrefix::Normal,
        0,
    )
    .expect("Valid 4-port config");

    let filter = CrsPunctureFilter::new(cfg, NrSubcarrierSpacing::Scs15kHz);
    let mask = filter.generate_prb_mask(0, 0, 0).unwrap();

    // Symbols 0, 4, 7, 11 (Ports 0 & 1): 4 * 4 = 16 REs
    assert_eq!(mask.symbol_masks[0].count_ones(), 4);
    assert_eq!(mask.symbol_masks[4].count_ones(), 4);
    assert_eq!(mask.symbol_masks[7].count_ones(), 4);
    assert_eq!(mask.symbol_masks[11].count_ones(), 4);

    // Symbols 1 and 8 (Ports 2 & 3): 2 * 4 = 8 REs
    assert_eq!(mask.symbol_masks[1].count_ones(), 4);
    assert_eq!(mask.symbol_masks[8].count_ones(), 4);

    // Total punctured for 4 ports: 16 + 8 = 24 REs
    assert_eq!(mask.total_punctured_res(), 24);
    assert_eq!(mask.available_pdsch_res(), 144);
}

#[test]
fn test_mbsfn_subframe_crs_exclusion() {
    // Subframe 1 is MBSFN (bit 1 set: 0b0000000010 = 2)
    let cfg = LteCrsConfig::new(
        0,
        50,
        LteAntennaPorts::TwoPorts,
        LteCyclicPrefix::Normal,
        0b0000000010,
    )
    .unwrap();

    let filter = CrsPunctureFilter::new(cfg, NrSubcarrierSpacing::Scs15kHz);

    // In Subframe 1 (MBSFN): CRS is only in symbols 0 and 1.
    // Symbols 2..13 have NO CRS!
    let mask_sf1 = filter.generate_prb_mask(1, 0, 0).unwrap();
    assert!(mask_sf1.symbol_masks[0].count_ones() > 0);
    for sym in 2..14 {
        assert_eq!(
            mask_sf1.symbol_masks[sym], 0,
            "MBSFN subframe symbol {} must be free of CRS",
            sym
        );
    }

    // In Subframe 0 (non-MBSFN): Symbols 4, 7, 11 have CRS
    let mask_sf0 = filter.generate_prb_mask(0, 0, 0).unwrap();
    assert!(mask_sf0.symbol_masks[4].count_ones() > 0);
    assert!(mask_sf0.symbol_masks[7].count_ones() > 0);
    assert!(mask_sf0.symbol_masks[11].count_ones() > 0);
}

#[test]
fn test_mixed_numerology_nr30_lte15_mapping() {
    let cfg =
        LteCrsConfig::new(0, 50, LteAntennaPorts::TwoPorts, LteCyclicPrefix::Normal, 0).unwrap();

    let filter = CrsPunctureFilter::new(cfg, NrSubcarrierSpacing::Scs30kHz);

    // NR Slot 0 in subframe 0 (0.5 ms)
    let mask_slot0 = filter.generate_prb_mask(0, 0, 0).unwrap();
    // NR Slot 1 in subframe 0 (0.5 ms)
    let mask_slot1 = filter.generate_prb_mask(0, 1, 0).unwrap();

    assert!(mask_slot0.total_punctured_res() > 0);
    assert!(mask_slot1.total_punctured_res() > 0);

    // Invalid slot index > 1 for 30 kHz
    let err = filter.generate_prb_mask(0, 2, 0);
    assert_eq!(err, Err(DssError::InvalidSlotIndex(2)));
}

#[test]
fn test_dss_capacity_metrics_and_code_rate_scaling() {
    let cfg =
        LteCrsConfig::new(0, 50, LteAntennaPorts::TwoPorts, LteCyclicPrefix::Normal, 0).unwrap();

    let filter = CrsPunctureFilter::new(cfg, NrSubcarrierSpacing::Scs15kHz);
    let mask = filter.generate_prb_mask(0, 0, 0).unwrap();

    // Nominal code rate 0.60
    let metrics = DssCapacityMetrics::compute(&mask, 0.60).unwrap();
    assert_eq!(metrics.raw_res_per_prb, 168);
    assert_eq!(metrics.punctured_res_per_prb, 16);
    assert_eq!(metrics.usable_pdsch_res_per_prb, 152);

    // Overhead % = 16 / 168 * 100 ≈ 9.5238%
    assert!((metrics.crs_overhead_pct - 9.5238).abs() < 0.01);
    assert!((metrics.capacity_loss_pct - 9.5238).abs() < 0.01);

    // Effective code rate: 0.60 * (168 / 152) = 0.66315
    assert!((metrics.effective_code_rate - 0.66315).abs() < 0.001);

    // Extreme code rate triggering threshold failure (> 0.93)
    let high_nominal_rate = 0.88; // 0.88 * (168 / 152) = 0.9726 > 0.93
    let err_result = DssCapacityMetrics::compute(&mask, high_nominal_rate);
    match err_result {
        Err(DssError::EffectiveCodeRateExceeded { rate, threshold }) => {
            assert_eq!(threshold, 930);
            assert!(rate > 930);
        }
        _ => panic!(
            "Expected EffectiveCodeRateExceeded error, got {:?}",
            err_result
        ),
    }
}

#[test]
fn test_oran_cplane_dss_section_codec() {
    let cfg = LteCrsConfig::new(
        42,
        100,
        LteAntennaPorts::FourPorts,
        LteCyclicPrefix::Normal,
        0,
    )
    .unwrap();

    let buf = OranDssSectionCodec::serialize_dss_extension(&cfg, 10, 40);
    assert_eq!(buf.len(), 12);
    assert_eq!(buf[0], 5); // Section Extension 5
    assert_eq!(buf[1], 3); // 3 32-bit words

    let (parsed_cfg, start_prb, num_prb) =
        OranDssSectionCodec::parse_dss_extension(&buf).expect("Parsing should succeed");

    assert_eq!(parsed_cfg.cell_id, 42);
    assert_eq!(parsed_cfg.antenna_ports, LteAntennaPorts::FourPorts);
    assert_eq!(parsed_cfg.v_shift, 42 % 6);
    assert_eq!(start_prb, 10);
    assert_eq!(num_prb, 40);

    // Truncated buffer test
    let trunc_err = OranDssSectionCodec::parse_dss_extension(&buf[0..6]);
    assert!(matches!(
        trunc_err,
        Err(DssError::BufferTooShort { need: 12, got: 6 })
    ));
}

#[test]
fn test_dss_error_display() {
    let err_id = DssError::InvalidCellId(600);
    assert!(err_id.to_string().contains("Invalid LTE Cell ID 600"));

    let err_prb = DssError::InvalidCarrierPrb(0);
    assert!(err_prb.to_string().contains("Invalid carrier PRB count 0"));

    let err_sf = DssError::InvalidSubframe(15);
    assert!(err_sf.to_string().contains("Invalid subframe number 15"));

    let err_rate = DssError::EffectiveCodeRateExceeded {
        rate: 950,
        threshold: 930,
    };
    assert!(err_rate.to_string().contains("Effective code rate"));
}
