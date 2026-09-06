//! Integration tests for O-RAN WG4 Open Fronthaul Carrier & Low-PHY Configuration Engine
//!
//! Conforms to O-RAN.WG4.MP.0 Section 4, o-ran-uplane-conf.yang, and o-ran-module-cap.yang.

use toy_tcpip::oran_carrier_mgmt::{
    CarrierDirection, CarrierState, CyclicPrefixType, EaxcBitAllocation, EaxcIdFields,
    IqCompressionFormat, LowLevelEndpoint, ModuleCapabilities, OranCarrierManager, RxCarrierConfig,
    TxCarrierConfig,
};

#[test]
fn test_module_capabilities_and_spectrum_validation() {
    let caps = ModuleCapabilities::n78_macro_ru();
    let eaxc_alloc = EaxcBitAllocation::default();
    let mut manager = OranCarrierManager::new(caps, eaxc_alloc);

    // 1. Valid Carrier: 3500 MHz, 100 MHz BW, 30 kHz SCS, FFT 2048
    let valid_tx = TxCarrierConfig {
        carrier_id: 0,
        name: "CC0-DL-3500M".to_string(),
        state: CarrierState::Disabled,
        center_frequency_hz: 3_500_000_000,
        channel_bandwidth_hz: 100_000_000,
        subcarrier_spacing_khz: 30,
        cyclic_prefix: CyclicPrefixType::Normal,
        fft_size: 2048,
        tx_power_dbm: 43.0,
        gain_correction_db: 0.0,
    };
    assert!(manager.configure_tx_carrier(valid_tx).is_ok());

    // 2. Frequency Out of Bounds (Lower Edge: 3250 MHz - 50 MHz = 3200 MHz < 3300 MHz)
    let out_of_band_low = TxCarrierConfig {
        carrier_id: 1,
        name: "CC-OutLow".to_string(),
        state: CarrierState::Disabled,
        center_frequency_hz: 3_320_000_000,
        channel_bandwidth_hz: 100_000_000, // half BW is 50MHz, lower edge = 3270MHz < 3300MHz
        subcarrier_spacing_khz: 30,
        cyclic_prefix: CyclicPrefixType::Normal,
        fft_size: 2048,
        tx_power_dbm: 40.0,
        gain_correction_db: 0.0,
    };
    assert!(manager.configure_tx_carrier(out_of_band_low).is_err());

    // 3. Bandwidth exceeds maximum allowable (150 MHz > 100 MHz limit)
    let excessive_bw = TxCarrierConfig {
        carrier_id: 1,
        name: "CC-ExcessBW".to_string(),
        state: CarrierState::Disabled,
        center_frequency_hz: 3_600_000_000,
        channel_bandwidth_hz: 150_000_000,
        subcarrier_spacing_khz: 30,
        cyclic_prefix: CyclicPrefixType::Normal,
        fft_size: 4096,
        tx_power_dbm: 40.0,
        gain_correction_db: 0.0,
    };
    assert!(manager.configure_tx_carrier(excessive_bw).is_err());

    // 4. Unsupported SCS (120 kHz is not supported in n78 macro profile)
    let unsupported_scs = TxCarrierConfig {
        carrier_id: 1,
        name: "CC-BadSCS".to_string(),
        state: CarrierState::Disabled,
        center_frequency_hz: 3_600_000_000,
        channel_bandwidth_hz: 100_000_000,
        subcarrier_spacing_khz: 120,
        cyclic_prefix: CyclicPrefixType::Normal,
        fft_size: 2048,
        tx_power_dbm: 40.0,
        gain_correction_db: 0.0,
    };
    assert!(manager.configure_tx_carrier(unsupported_scs).is_err());

    // 5. Invalid FFT Size (1000 is not a valid FFT power-of-2 size)
    let bad_fft = TxCarrierConfig {
        carrier_id: 1,
        name: "CC-BadFFT".to_string(),
        state: CarrierState::Disabled,
        center_frequency_hz: 3_600_000_000,
        channel_bandwidth_hz: 100_000_000,
        subcarrier_spacing_khz: 30,
        cyclic_prefix: CyclicPrefixType::Normal,
        fft_size: 1000,
        tx_power_dbm: 40.0,
        gain_correction_db: 0.0,
    };
    assert!(manager.configure_tx_carrier(bad_fft).is_err());
}

#[test]
fn test_carrier_lifecycle_state_transitions() {
    let caps = ModuleCapabilities::n78_macro_ru();
    let eaxc_alloc = EaxcBitAllocation::default();
    let mut manager = OranCarrierManager::new(caps, eaxc_alloc);

    let tx_cfg = TxCarrierConfig {
        carrier_id: 0,
        name: "DL-Carrier-0".to_string(),
        state: CarrierState::Disabled,
        center_frequency_hz: 3_500_000_000,
        channel_bandwidth_hz: 100_000_000,
        subcarrier_spacing_khz: 30,
        cyclic_prefix: CyclicPrefixType::Normal,
        fft_size: 2048,
        tx_power_dbm: 43.0,
        gain_correction_db: 0.0,
    };

    let rx_cfg = RxCarrierConfig {
        carrier_id: 0,
        name: "UL-Carrier-0".to_string(),
        state: CarrierState::Disabled,
        center_frequency_hz: 3_500_000_000,
        channel_bandwidth_hz: 100_000_000,
        subcarrier_spacing_khz: 30,
        cyclic_prefix: CyclicPrefixType::Normal,
        fft_size: 2048,
        digital_gain_db: 0.0,
    };

    manager.configure_tx_carrier(tx_cfg).unwrap();
    manager.configure_rx_carrier(rx_cfg).unwrap();

    // Verify initial states are Disabled
    assert_eq!(
        manager.get_tx_carrier(0).unwrap().state,
        CarrierState::Disabled
    );
    assert_eq!(
        manager.get_rx_carrier(0).unwrap().state,
        CarrierState::Disabled
    );

    // Activate TX carrier
    manager.activate_tx_carrier(0).unwrap();
    assert_eq!(
        manager.get_tx_carrier(0).unwrap().state,
        CarrierState::Ready
    );
    assert_eq!(manager.compute_total_tx_bandwidth(), 100_000_000);

    // Activate RX carrier
    manager.activate_rx_carrier(0).unwrap();
    assert_eq!(
        manager.get_rx_carrier(0).unwrap().state,
        CarrierState::Ready
    );
    assert_eq!(manager.compute_total_rx_bandwidth(), 100_000_000);

    // Deactivate TX carrier
    manager.deactivate_tx_carrier(0).unwrap();
    assert_eq!(
        manager.get_tx_carrier(0).unwrap().state,
        CarrierState::Disabled
    );
    assert_eq!(manager.compute_total_tx_bandwidth(), 0);

    // Reactivate and test emergency shutdown
    manager.activate_tx_carrier(0).unwrap();
    assert_eq!(
        manager.get_tx_carrier(0).unwrap().state,
        CarrierState::Ready
    );
    assert_eq!(
        manager.get_rx_carrier(0).unwrap().state,
        CarrierState::Ready
    );

    manager.emergency_stop_all();
    assert_eq!(
        manager.get_tx_carrier(0).unwrap().state,
        CarrierState::Disabled
    );
    assert_eq!(
        manager.get_rx_carrier(0).unwrap().state,
        CarrierState::Disabled
    );
}

#[test]
fn test_eaxc_bit_allocation_codec() {
    // 1. Valid 16-bit scheme: 2 bits DU Port, 6 bits BandSector, 4 bits CC, 4 bits RU Port
    let alloc = EaxcBitAllocation::new(2, 6, 4, 4).expect("Valid 16-bit scheme");

    let fields = EaxcIdFields {
        du_port_id: 2,      // 2 bits: 0..3 (2 is valid)
        band_sector_id: 45, // 6 bits: 0..63 (45 is valid)
        cc_id: 3,           // 4 bits: 0..15 (3 is valid)
        ru_port_id: 11,     // 4 bits: 0..15 (11 is valid)
    };

    let encoded = alloc.encode(&fields).expect("Valid encoding");
    let decoded = alloc.decode(encoded);

    assert_eq!(decoded.du_port_id, 2);
    assert_eq!(decoded.band_sector_id, 45);
    assert_eq!(decoded.cc_id, 3);
    assert_eq!(decoded.ru_port_id, 11);

    // 2. Value overflow check: du_port_id = 4 requires 3 bits, should error on 2-bit field
    let overflow_fields = EaxcIdFields {
        du_port_id: 4,
        band_sector_id: 1,
        cc_id: 1,
        ru_port_id: 1,
    };
    assert!(alloc.encode(&overflow_fields).is_err());

    // 3. Scheme must sum to exactly 16 bits
    assert!(EaxcBitAllocation::new(3, 6, 4, 4).is_err()); // 17 bits
    assert!(EaxcBitAllocation::new(1, 6, 4, 4).is_err()); // 15 bits
    assert!(EaxcBitAllocation::new(0, 8, 4, 4).is_err()); // Zero-width field
}

#[test]
fn test_low_level_endpoint_configuration_and_compression() {
    let caps = ModuleCapabilities::n78_macro_ru();
    let eaxc_alloc = EaxcBitAllocation::default();
    let mut manager = OranCarrierManager::new(caps, eaxc_alloc);

    // Configure TX Carrier 0 and RX Carrier 1
    let tx_cfg = TxCarrierConfig {
        carrier_id: 0,
        name: "DL0".to_string(),
        state: CarrierState::Disabled,
        center_frequency_hz: 3_500_000_000,
        channel_bandwidth_hz: 100_000_000,
        subcarrier_spacing_khz: 30,
        cyclic_prefix: CyclicPrefixType::Normal,
        fft_size: 2048,
        tx_power_dbm: 43.0,
        gain_correction_db: 0.0,
    };
    let rx_cfg = RxCarrierConfig {
        carrier_id: 1,
        name: "UL1".to_string(),
        state: CarrierState::Disabled,
        center_frequency_hz: 3_600_000_000,
        channel_bandwidth_hz: 100_000_000,
        subcarrier_spacing_khz: 30,
        cyclic_prefix: CyclicPrefixType::Normal,
        fft_size: 2048,
        digital_gain_db: 0.0,
    };
    manager.configure_tx_carrier(tx_cfg).unwrap();
    manager.configure_rx_carrier(rx_cfg).unwrap();

    // 1. Configure valid DL Endpoint with Block Floating Point (9-bit)
    let dl_ep = LowLevelEndpoint {
        endpoint_id: 101,
        name: "DL-Endpoint-BFP9".to_string(),
        direction: CarrierDirection::DownlinkTx,
        carrier_id: 0,
        eaxc_id: 0x1234,
        compression_format: IqCompressionFormat::BlockFloatingPoint,
        iq_bit_width: 9,
        antenna_port_index: 0,
    };
    assert!(manager.configure_endpoint(dl_ep).is_ok());

    // 2. Configure valid UL Endpoint with MuLaw (8-bit)
    let ul_ep = LowLevelEndpoint {
        endpoint_id: 201,
        name: "UL-Endpoint-MuLaw8".to_string(),
        direction: CarrierDirection::UplinkRx,
        carrier_id: 1,
        eaxc_id: 0x5678,
        compression_format: IqCompressionFormat::MuLaw,
        iq_bit_width: 8,
        antenna_port_index: 1,
    };
    assert!(manager.configure_endpoint(ul_ep).is_ok());

    // 3. Endpoint binding to non-existent carrier fails
    let bad_carrier_ep = LowLevelEndpoint {
        endpoint_id: 301,
        name: "Bad-Carrier-EP".to_string(),
        direction: CarrierDirection::DownlinkTx,
        carrier_id: 5, // Non-existent
        eaxc_id: 0x9999,
        compression_format: IqCompressionFormat::Uncompressed,
        iq_bit_width: 16,
        antenna_port_index: 2,
    };
    assert!(manager.configure_endpoint(bad_carrier_ep).is_err());

    // 4. Invalid bit width (< 8 or > 16) fails
    let bad_bitwidth_ep = LowLevelEndpoint {
        endpoint_id: 401,
        name: "Bad-BitWidth-EP".to_string(),
        direction: CarrierDirection::DownlinkTx,
        carrier_id: 0,
        eaxc_id: 0xAAAA,
        compression_format: IqCompressionFormat::BlockFloatingPoint,
        iq_bit_width: 6, // Too small
        antenna_port_index: 3,
    };
    assert!(manager.configure_endpoint(bad_bitwidth_ep).is_err());
}

#[test]
fn test_aggregate_tx_power_monitoring_and_limit_enforcement() {
    // n78 macro RU has max_tx_power_dbm = 46.0 dBm (approx 39.81 Watts)
    let caps = ModuleCapabilities::n78_macro_ru();
    let eaxc_alloc = EaxcBitAllocation::default();
    let mut manager = OranCarrierManager::new(caps, eaxc_alloc);

    // Carrier 0: 42.0 dBm = 15.85 Watts
    let c0 = TxCarrierConfig {
        carrier_id: 0,
        name: "CC0".to_string(),
        state: CarrierState::Disabled,
        center_frequency_hz: 3_450_000_000,
        channel_bandwidth_hz: 50_000_000,
        subcarrier_spacing_khz: 30,
        cyclic_prefix: CyclicPrefixType::Normal,
        fft_size: 1024,
        tx_power_dbm: 42.0,
        gain_correction_db: 0.0,
    };

    // Carrier 1: 42.0 dBm = 15.85 Watts
    let c1 = TxCarrierConfig {
        carrier_id: 1,
        name: "CC1".to_string(),
        state: CarrierState::Disabled,
        center_frequency_hz: 3_550_000_000,
        channel_bandwidth_hz: 50_000_000,
        subcarrier_spacing_khz: 30,
        cyclic_prefix: CyclicPrefixType::Normal,
        fft_size: 1024,
        tx_power_dbm: 42.0,
        gain_correction_db: 0.0,
    };

    // Carrier 2: 42.0 dBm = 15.85 Watts
    let c2 = TxCarrierConfig {
        carrier_id: 2,
        name: "CC2".to_string(),
        state: CarrierState::Disabled,
        center_frequency_hz: 3_650_000_000,
        channel_bandwidth_hz: 50_000_000,
        subcarrier_spacing_khz: 30,
        cyclic_prefix: CyclicPrefixType::Normal,
        fft_size: 1024,
        tx_power_dbm: 42.0,
        gain_correction_db: 0.0,
    };

    manager.configure_tx_carrier(c0).unwrap();
    manager.configure_tx_carrier(c1).unwrap();
    manager.configure_tx_carrier(c2).unwrap();

    // 1. Initial power: 0W
    let (watts_0, _) = manager.compute_aggregate_tx_power();
    assert_eq!(watts_0, 0.0);

    // 2. Activate C0: 15.85W (42.0 dBm)
    manager.activate_tx_carrier(0).unwrap();
    let (watts_1, dbm_1) = manager.compute_aggregate_tx_power();
    assert!((watts_1 - 15.85).abs() < 0.1);
    assert!((dbm_1 - 42.0).abs() < 0.1);

    // 3. Activate C1: 15.85 + 15.85 = 31.7W (approx 45.01 dBm <= 46.0 dBm limit)
    manager.activate_tx_carrier(1).unwrap();
    let (watts_2, dbm_2) = manager.compute_aggregate_tx_power();
    assert!((watts_2 - 31.7).abs() < 0.2);
    assert!((dbm_2 - 45.01).abs() < 0.1);

    // 4. Activating C2 would push total to 31.7 + 15.85 = 47.55W > 39.81W limit! Must be rejected!
    let act_c2_err = manager.activate_tx_carrier(2);
    assert!(
        act_c2_err.is_err(),
        "Must reject carrier activation exceeding total RU power rating"
    );
    assert_eq!(
        manager.get_tx_carrier(2).unwrap().state,
        CarrierState::Disabled
    );

    // 5. Deactivate C0 -> allows C2 to be activated (15.85 + 15.85 = 31.7W <= 39.81W)
    manager.deactivate_tx_carrier(0).unwrap();
    assert!(manager.activate_tx_carrier(2).is_ok());
    let (watts_final, _) = manager.compute_aggregate_tx_power();
    assert!((watts_final - 31.7).abs() < 0.2);
}
