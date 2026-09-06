//! O-RAN WG4 Open Fronthaul Carrier & Low-PHY Configuration Engine
//!
//! Conforms to:
//! - O-RAN.WG4.MP.0 Section 4: Carrier and Low-PHY Configuration
//! - `o-ran-uplane-conf.yang`: YANG Data Model for U-Plane Configuration
//! - `o-ran-module-cap.yang`: YANG Data Model for O-RU Module Capabilities
//! - 3GPP TS 38.104 (Base Station Radio Transmission and Reception)
//!
//! Pure standard Rust (`std`/`core` only), zero external dependencies.

use std::collections::HashMap;
use std::fmt;

/// Operational state of a TX or RX carrier inside the O-RU (o-ran-uplane-conf.yang).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarrierState {
    /// Configured but disabled; LO powered off, RF chain inactive.
    Disabled,
    /// Carrier is transitioning (frequency synthesizer tuning, calibration, power ramping).
    Busy,
    /// Fully active and operational; transmitting/receiving fronthaul IQ data.
    Ready,
}

impl fmt::Display for CarrierState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CarrierState::Disabled => write!(f, "DISABLED"),
            CarrierState::Busy => write!(f, "BUSY"),
            CarrierState::Ready => write!(f, "READY"),
        }
    }
}

/// Direction of carrier / endpoint transmission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarrierDirection {
    /// Downlink transmission from O-DU to O-RU towards air interface.
    DownlinkTx,
    /// Uplink reception from air interface towards O-DU.
    UplinkRx,
}

/// Cyclic Prefix Type (TS 38.211).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CyclicPrefixType {
    /// Normal Cyclic Prefix.
    Normal,
    /// Extended Cyclic Prefix (for 60 kHz SCS).
    Extended,
}

/// IQ Compression format for fronthaul user plane (o-ran-uplane-conf.yang).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IqCompressionFormat {
    /// Block Floating Point (BFP) compression (e.g. 9-bit or 12-bit + exponent).
    BlockFloatingPoint,
    /// Non-linear mu-law / A-law compression.
    MuLaw,
    /// Modulation compression for DL data channels.
    ModulationCompression,
    /// Uncompressed 16-bit linear I/Q samples.
    Uncompressed,
}

/// Structured fields for 16-bit extended Antenna-Carrier Identifier (eAxC_ID).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EaxcIdFields {
    /// O-DU Port Identifier.
    pub du_port_id: u16,
    /// Band and Sector Identifier.
    pub band_sector_id: u16,
    /// Component Carrier Identifier.
    pub cc_id: u16,
    /// O-RU Spatial Port / Antenna Stream Identifier.
    pub ru_port_id: u16,
}

/// Bit allocation definition for the 16-bit `eAxC_ID` (O-RAN.WG4.MP.0 §4.2).
///
/// Invariant: `du_port_bits + band_sector_bits + cc_bits + ru_port_bits == 16`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EaxcBitAllocation {
    pub du_port_bits: u8,
    pub band_sector_bits: u8,
    pub cc_bits: u8,
    pub ru_port_bits: u8,
}

impl EaxcBitAllocation {
    /// Create a new bit allocation scheme for eAxC_ID.
    pub fn new(
        du_port_bits: u8,
        band_sector_bits: u8,
        cc_bits: u8,
        ru_port_bits: u8,
    ) -> Result<Self, &'static str> {
        if du_port_bits + band_sector_bits + cc_bits + ru_port_bits != 16 {
            return Err("Sum of eAxC subfield bit widths must equal exactly 16 bits");
        }
        if du_port_bits == 0 || band_sector_bits == 0 || cc_bits == 0 || ru_port_bits == 0 {
            return Err("Each eAxC subfield must have at least 1 bit allocated");
        }
        Ok(Self {
            du_port_bits,
            band_sector_bits,
            cc_bits,
            ru_port_bits,
        })
    }

    /// Encode structured eAxC fields into a 16-bit wire ID.
    pub fn encode(&self, fields: &EaxcIdFields) -> Result<u16, &'static str> {
        let max_du = (1 << self.du_port_bits) - 1;
        let max_bs = (1 << self.band_sector_bits) - 1;
        let max_cc = (1 << self.cc_bits) - 1;
        let max_ru = (1 << self.ru_port_bits) - 1;

        if fields.du_port_id > max_du {
            return Err("du_port_id exceeds allocated bit width");
        }
        if fields.band_sector_id > max_bs {
            return Err("band_sector_id exceeds allocated bit width");
        }
        if fields.cc_id > max_cc {
            return Err("cc_id exceeds allocated bit width");
        }
        if fields.ru_port_id > max_ru {
            return Err("ru_port_id exceeds allocated bit width");
        }

        let mut val: u16 = 0;
        let mut shift = 16;

        shift -= self.du_port_bits;
        val |= fields.du_port_id << shift;

        shift -= self.band_sector_bits;
        val |= fields.band_sector_id << shift;

        shift -= self.cc_bits;
        val |= fields.cc_id << shift;

        shift -= self.ru_port_bits;
        val |= fields.ru_port_id << shift;

        Ok(val)
    }

    /// Decode a 16-bit wire eAxC ID into structured fields.
    pub fn decode(&self, eaxc_id: u16) -> EaxcIdFields {
        let mut shift = 16;

        shift -= self.du_port_bits;
        let du_mask = (1 << self.du_port_bits) - 1;
        let du_port_id = (eaxc_id >> shift) & du_mask;

        shift -= self.band_sector_bits;
        let bs_mask = (1 << self.band_sector_bits) - 1;
        let band_sector_id = (eaxc_id >> shift) & bs_mask;

        shift -= self.cc_bits;
        let cc_mask = (1 << self.cc_bits) - 1;
        let cc_id = (eaxc_id >> shift) & cc_mask;

        shift -= self.ru_port_bits;
        let ru_mask = (1 << self.ru_port_bits) - 1;
        let ru_port_id = (eaxc_id >> shift) & ru_mask;

        EaxcIdFields {
            du_port_id,
            band_sector_id,
            cc_id,
            ru_port_id,
        }
    }
}

impl Default for EaxcBitAllocation {
    /// Default O-RAN bit allocation: 2b DU_Port, 6b BandSector, 4b CC, 4b RU_Port.
    fn default() -> Self {
        Self {
            du_port_bits: 2,
            band_sector_bits: 6,
            cc_bits: 4,
            ru_port_bits: 4,
        }
    }
}

/// Transmission (Downlink) Carrier Configuration (o-ran-uplane-conf.yang).
#[derive(Debug, Clone, PartialEq)]
pub struct TxCarrierConfig {
    /// Unique carrier ID (0..7).
    pub carrier_id: u8,
    /// Descriptive name.
    pub name: String,
    /// Operational state.
    pub state: CarrierState,
    /// RF Center frequency in Hz (e.g. 3,500,000,000 Hz).
    pub center_frequency_hz: u64,
    /// Channel bandwidth in Hz (e.g. 100,000,000 Hz for 100 MHz).
    pub channel_bandwidth_hz: u64,
    /// Subcarrier spacing in kHz (15, 30, 60, 120, 240).
    pub subcarrier_spacing_khz: u16,
    /// Cyclic prefix type.
    pub cyclic_prefix: CyclicPrefixType,
    /// FFT size (512..4096).
    pub fft_size: u16,
    /// Configured RF output power in dBm (e.g. 43.0 dBm = 20W).
    pub tx_power_dbm: f32,
    /// Gain correction in dB.
    pub gain_correction_db: f32,
}

/// Reception (Uplink) Carrier Configuration (o-ran-uplane-conf.yang).
#[derive(Debug, Clone, PartialEq)]
pub struct RxCarrierConfig {
    /// Unique carrier ID (0..7).
    pub carrier_id: u8,
    /// Descriptive name.
    pub name: String,
    /// Operational state.
    pub state: CarrierState,
    /// RF Center frequency in Hz.
    pub center_frequency_hz: u64,
    /// Channel bandwidth in Hz.
    pub channel_bandwidth_hz: u64,
    /// Subcarrier spacing in kHz.
    pub subcarrier_spacing_khz: u16,
    /// Cyclic prefix type.
    pub cyclic_prefix: CyclicPrefixType,
    /// FFT size.
    pub fft_size: u16,
    /// Baseband digital gain in dB.
    pub digital_gain_db: f32,
}

/// Low-level U-Plane Endpoint coupling a carrier to an eAxC_ID and antenna port.
#[derive(Debug, Clone, PartialEq)]
pub struct LowLevelEndpoint {
    /// Unique endpoint identifier.
    pub endpoint_id: u16,
    /// Descriptive name.
    pub name: String,
    /// Direction: Downlink TX or Uplink RX.
    pub direction: CarrierDirection,
    /// Bound carrier ID.
    pub carrier_id: u8,
    /// 16-bit eAxC ID.
    pub eaxc_id: u16,
    /// IQ Compression method.
    pub compression_format: IqCompressionFormat,
    /// IQ Bit width (8..16 bits).
    pub iq_bit_width: u8,
    /// Physical antenna port index (0..63).
    pub antenna_port_index: u16,
}

/// O-RU Hardware and Module Capabilities (o-ran-module-cap.yang).
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleCapabilities {
    /// Minimum operating frequency in Hz.
    pub min_frequency_hz: u64,
    /// Maximum operating frequency in Hz.
    pub max_frequency_hz: u64,
    /// Maximum channel bandwidth per carrier in Hz.
    pub max_bandwidth_hz: u64,
    /// Maximum aggregated bandwidth across all active carriers in Hz.
    pub max_aggregated_bandwidth_hz: u64,
    /// Maximum number of TX carriers.
    pub max_tx_carriers: u8,
    /// Maximum number of RX carriers.
    pub max_rx_carriers: u8,
    /// Maximum total aggregate transmission power in dBm (e.g. 46.0 dBm = 40W).
    pub max_tx_power_dbm: f32,
    /// Supported subcarrier spacings in kHz.
    pub supported_scs_khz: Vec<u16>,
}

impl ModuleCapabilities {
    /// Factory for typical 5G NR Band n78 (3300 MHz - 3800 MHz) 40W O-RU.
    pub fn n78_macro_ru() -> Self {
        Self {
            min_frequency_hz: 3_300_000_000,
            max_frequency_hz: 3_800_000_000,
            max_bandwidth_hz: 100_000_000,            // 100 MHz
            max_aggregated_bandwidth_hz: 200_000_000, // 200 MHz
            max_tx_carriers: 4,
            max_rx_carriers: 4,
            max_tx_power_dbm: 46.0, // 40 Watts
            supported_scs_khz: vec![15, 30, 60],
        }
    }
}

/// Primary O-RAN WG4 Carrier and Low-PHY Configuration Manager.
#[derive(Debug, Clone)]
pub struct OranCarrierManager {
    /// Hardware capabilities profile.
    pub capabilities: ModuleCapabilities,
    /// Active eAxC bit allocation.
    pub eaxc_allocation: EaxcBitAllocation,
    /// Configured TX carriers (carrier_id -> config).
    tx_carriers: HashMap<u8, TxCarrierConfig>,
    /// Configured RX carriers (carrier_id -> config).
    rx_carriers: HashMap<u8, RxCarrierConfig>,
    /// Configured Low-Level Endpoints (endpoint_id -> endpoint).
    endpoints: HashMap<u16, LowLevelEndpoint>,
}

impl OranCarrierManager {
    /// Create a new O-RAN Carrier Manager.
    pub fn new(capabilities: ModuleCapabilities, eaxc_allocation: EaxcBitAllocation) -> Self {
        Self {
            capabilities,
            eaxc_allocation,
            tx_carriers: HashMap::new(),
            rx_carriers: HashMap::new(),
            endpoints: HashMap::new(),
        }
    }

    /// Add or update a TX carrier configuration.
    /// Carrier starts in `CarrierState::Disabled`.
    pub fn configure_tx_carrier(
        &mut self,
        mut config: TxCarrierConfig,
    ) -> Result<(), &'static str> {
        self.validate_carrier_params(
            config.center_frequency_hz,
            config.channel_bandwidth_hz,
            config.subcarrier_spacing_khz,
            config.fft_size,
        )?;

        if self.tx_carriers.len() >= self.capabilities.max_tx_carriers as usize
            && !self.tx_carriers.contains_key(&config.carrier_id)
        {
            return Err("Maximum number of TX carriers reached");
        }

        // Newly configured carrier starts in Disabled state
        config.state = CarrierState::Disabled;
        self.tx_carriers.insert(config.carrier_id, config);
        Ok(())
    }

    /// Add or update an RX carrier configuration.
    /// Carrier starts in `CarrierState::Disabled`.
    pub fn configure_rx_carrier(
        &mut self,
        mut config: RxCarrierConfig,
    ) -> Result<(), &'static str> {
        self.validate_carrier_params(
            config.center_frequency_hz,
            config.channel_bandwidth_hz,
            config.subcarrier_spacing_khz,
            config.fft_size,
        )?;

        if self.rx_carriers.len() >= self.capabilities.max_rx_carriers as usize
            && !self.rx_carriers.contains_key(&config.carrier_id)
        {
            return Err("Maximum number of RX carriers reached");
        }

        config.state = CarrierState::Disabled;
        self.rx_carriers.insert(config.carrier_id, config);
        Ok(())
    }

    /// Activate a configured TX carrier (transitions Disabled -> Busy -> Ready).
    /// Performs aggregate power and bandwidth validation before activation.
    pub fn activate_tx_carrier(&mut self, carrier_id: u8) -> Result<(), &'static str> {
        let (state, channel_bw, tx_power_dbm) = {
            let carrier = self
                .tx_carriers
                .get(&carrier_id)
                .ok_or("TX carrier not found")?;
            (
                carrier.state,
                carrier.channel_bandwidth_hz,
                carrier.tx_power_dbm,
            )
        };

        if state == CarrierState::Ready {
            return Ok(()); // Already active
        }

        // Check aggregate bandwidth if activated
        let current_bw = self.compute_total_tx_bandwidth();
        let target_bw = current_bw + channel_bw;
        if target_bw > self.capabilities.max_aggregated_bandwidth_hz {
            return Err("Activation would exceed maximum aggregated TX bandwidth");
        }

        // Check aggregate RF power if activated
        let current_p_watts = self.compute_aggregate_tx_power().0;
        let additional_watts = 10.0f64.powf((tx_power_dbm as f64 - 30.0) / 10.0);
        let target_p_watts = current_p_watts + additional_watts;
        let max_p_watts = 10.0f64.powf((self.capabilities.max_tx_power_dbm as f64 - 30.0) / 10.0);

        if target_p_watts > max_p_watts + 1e-5 {
            return Err("Activation would exceed maximum allowable O-RU transmission power");
        }

        let carrier = self.tx_carriers.get_mut(&carrier_id).unwrap();
        // Transition through Busy to Ready
        carrier.state = CarrierState::Busy;
        // In real hardware, synthesizer locks here; simulated to Ready
        carrier.state = CarrierState::Ready;

        Ok(())
    }

    /// Deactivate an active TX carrier (transitions Ready -> Busy -> Disabled).
    pub fn deactivate_tx_carrier(&mut self, carrier_id: u8) -> Result<(), &'static str> {
        let carrier = self
            .tx_carriers
            .get_mut(&carrier_id)
            .ok_or("TX carrier not found")?;

        carrier.state = CarrierState::Busy;
        // Ramp down power, flush buffers
        carrier.state = CarrierState::Disabled;
        Ok(())
    }

    /// Activate a configured RX carrier.
    pub fn activate_rx_carrier(&mut self, carrier_id: u8) -> Result<(), &'static str> {
        let (state, channel_bw) = {
            let carrier = self
                .rx_carriers
                .get(&carrier_id)
                .ok_or("RX carrier not found")?;
            (carrier.state, carrier.channel_bandwidth_hz)
        };

        if state == CarrierState::Ready {
            return Ok(());
        }

        let current_bw = self.compute_total_rx_bandwidth();
        let target_bw = current_bw + channel_bw;
        if target_bw > self.capabilities.max_aggregated_bandwidth_hz {
            return Err("Activation would exceed maximum aggregated RX bandwidth");
        }

        let carrier = self.rx_carriers.get_mut(&carrier_id).unwrap();
        carrier.state = CarrierState::Busy;
        carrier.state = CarrierState::Ready;
        Ok(())
    }

    /// Deactivate an active RX carrier.
    pub fn deactivate_rx_carrier(&mut self, carrier_id: u8) -> Result<(), &'static str> {
        let carrier = self
            .rx_carriers
            .get_mut(&carrier_id)
            .ok_or("RX carrier not found")?;

        carrier.state = CarrierState::Busy;
        carrier.state = CarrierState::Disabled;
        Ok(())
    }

    /// Add a Low-Level Endpoint binding.
    pub fn configure_endpoint(&mut self, endpoint: LowLevelEndpoint) -> Result<(), &'static str> {
        // Validate bound carrier exists
        match endpoint.direction {
            CarrierDirection::DownlinkTx => {
                if !self.tx_carriers.contains_key(&endpoint.carrier_id) {
                    return Err("Endpoint binds to non-existent TX carrier");
                }
            }
            CarrierDirection::UplinkRx => {
                if !self.rx_carriers.contains_key(&endpoint.carrier_id) {
                    return Err("Endpoint binds to non-existent RX carrier");
                }
            }
        }

        // Validate bit width
        if endpoint.iq_bit_width < 8 || endpoint.iq_bit_width > 16 {
            return Err("IQ bit width must be between 8 and 16");
        }

        self.endpoints.insert(endpoint.endpoint_id, endpoint);
        Ok(())
    }

    /// Get a configured TX carrier.
    pub fn get_tx_carrier(&self, carrier_id: u8) -> Option<&TxCarrierConfig> {
        self.tx_carriers.get(&carrier_id)
    }

    /// Get a configured RX carrier.
    pub fn get_rx_carrier(&self, carrier_id: u8) -> Option<&RxCarrierConfig> {
        self.rx_carriers.get(&carrier_id)
    }

    /// Get a configured endpoint.
    pub fn get_endpoint(&self, endpoint_id: u16) -> Option<&LowLevelEndpoint> {
        self.endpoints.get(&endpoint_id)
    }

    /// Calculate aggregate RF transmission power across all READY TX carriers.
    /// Returns `(power_watts, power_dbm)`.
    pub fn compute_aggregate_tx_power(&self) -> (f64, f32) {
        let mut total_watts = 0.0f64;
        for c in self.tx_carriers.values() {
            if c.state == CarrierState::Ready {
                let watts = 10.0f64.powf((c.tx_power_dbm as f64 - 30.0) / 10.0);
                total_watts += watts;
            }
        }

        let total_dbm = if total_watts > 1e-9 {
            (10.0 * total_watts.log10() + 30.0) as f32
        } else {
            -100.0 // Noise floor / disabled
        };

        (total_watts, total_dbm)
    }

    /// Calculate total aggregated bandwidth of all active (Ready) TX carriers.
    pub fn compute_total_tx_bandwidth(&self) -> u64 {
        self.tx_carriers
            .values()
            .filter(|c| c.state == CarrierState::Ready)
            .map(|c| c.channel_bandwidth_hz)
            .sum()
    }

    /// Calculate total aggregated bandwidth of all active (Ready) RX carriers.
    pub fn compute_total_rx_bandwidth(&self) -> u64 {
        self.rx_carriers
            .values()
            .filter(|c| c.state == CarrierState::Ready)
            .map(|c| c.channel_bandwidth_hz)
            .sum()
    }

    /// Emergency shutdown: Immediately disables all active TX and RX carriers.
    pub fn emergency_stop_all(&mut self) {
        for c in self.tx_carriers.values_mut() {
            c.state = CarrierState::Disabled;
        }
        for c in self.rx_carriers.values_mut() {
            c.state = CarrierState::Disabled;
        }
    }

    /// Validate carrier parameters against module capabilities and physics constraints.
    fn validate_carrier_params(
        &self,
        center_freq: u64,
        bandwidth: u64,
        scs_khz: u16,
        fft_size: u16,
    ) -> Result<(), &'static str> {
        if bandwidth == 0 || bandwidth > self.capabilities.max_bandwidth_hz {
            return Err("Carrier channel bandwidth exceeds maximum allowed bandwidth");
        }

        let half_bw = bandwidth / 2;
        if center_freq < self.capabilities.min_frequency_hz + half_bw {
            return Err("Lower carrier edge is below O-RU minimum operating frequency");
        }
        if center_freq + half_bw > self.capabilities.max_frequency_hz {
            return Err("Upper carrier edge is above O-RU maximum operating frequency");
        }

        if !self.capabilities.supported_scs_khz.contains(&scs_khz) {
            return Err("Subcarrier spacing is not supported by O-RU module capabilities");
        }

        // Validate FFT size is power of 2 between 512 and 4096
        if !matches!(fft_size, 512 | 1024 | 1536 | 2048 | 4096) {
            return Err("FFT size must be 512, 1024, 1536, 2048, or 4096");
        }

        Ok(())
    }
}
