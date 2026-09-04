//! O-RAN WG4 Open Fronthaul Control Plane Section Type 0 Engine.
//!
//! Implements O-RAN.WG4.CUS-Plane Section Type 0 ("Unused Resource Blocks or Symbols")
//! used by the O-DU to command the O-RU to blank transmissions, reserve idle guardbands,
//! protect against radar/DFS interference, puncture REs for LTE/NR Dynamic Spectrum Sharing (DSS),
//! establish quiet calibration intervals, and activate RF Power Amplifier (PA) micro-sleep.
//!
//! Section Type 0 carries no User Plane (U-Plane) IQ data.

use std::fmt;

use crate::oran_fh_cus::{OranError, OranRadioHeader};

/// O-RAN Section Type 0 identifier.
pub const ORAN_SECTION_TYPE_0: u8 = 0;

/// Length of the C-Plane Section Type 0 Common Header (radio header 4 bytes + type 0 common 8 bytes = 12 bytes).
pub const ORAN_SECTION_TYPE_0_COMMON_HEADER_LEN: usize = 12;

/// Length of a single Section Type 0 section body (8 bytes).
pub const ORAN_SECTION_TYPE_0_SECTION_LEN: usize = 8;

/// Standard number of subcarriers per Physical Resource Block in 3GPP 5G NR.
pub const NR_SUBCARRIERS_PER_PRB: usize = 12;

/// Standard number of OFDM symbols per slot with normal cyclic prefix.
pub const NR_SYMBOLS_PER_SLOT: usize = 14;

/// Errors raised during Section Type 0 processing, validation, serialization, or parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OranSectionType0Error {
    /// Payload buffer is truncated.
    Truncated { need: usize, got: usize },
    /// Unsupported payload version (expected 1).
    UnsupportedPayloadVersion(u8),
    /// Unsupported section type (expected 0 for Section Type 0).
    UnsupportedSectionType(u8),
    /// Number of sections declared in header does not match parsed sections.
    SectionCountMismatch { declared: u8, parsed: usize },
    /// Section extensions indicated (`ef = true`), but extension parsing failed or not expected.
    SectionExtensionUnsupported(u16),
    /// Field value is out of its specification-allowed bit width or range.
    FieldOutOfRange {
        field: &'static str,
        value: u32,
        max: u32,
    },
    /// Invalid FFT size index in frame structure.
    InvalidFftSize(u8),
    /// Invalid subcarrier spacing (SCS) index in frame structure.
    InvalidScs(u8),
    /// Invalid resource element mask (must fit in 12 bits).
    InvalidReMask(u16),
    /// Underlying CUS common error.
    CusError(String),
}

impl fmt::Display for OranSectionType0Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OranSectionType0Error::Truncated { need, got } => {
                write!(
                    f,
                    "O-RAN Section Type 0 payload truncated: need {} bytes, got {}",
                    need, got
                )
            }
            OranSectionType0Error::UnsupportedPayloadVersion(v) => {
                write!(f, "Unsupported payload version {}, expected 1", v)
            }
            OranSectionType0Error::UnsupportedSectionType(t) => {
                write!(f, "Unsupported section type {}, expected Type 0", t)
            }
            OranSectionType0Error::SectionCountMismatch { declared, parsed } => {
                write!(
                    f,
                    "Section count mismatch: declared {}, parsed {}",
                    declared, parsed
                )
            }
            OranSectionType0Error::SectionExtensionUnsupported(id) => {
                write!(f, "Section ID {} has unsupported section extensions", id)
            }
            OranSectionType0Error::FieldOutOfRange { field, value, max } => {
                write!(
                    f,
                    "Field '{}' value {} exceeds maximum allowed {}",
                    field, value, max
                )
            }
            OranSectionType0Error::InvalidFftSize(val) => {
                write!(f, "Invalid FFT size index {}", val)
            }
            OranSectionType0Error::InvalidScs(val) => {
                write!(f, "Invalid Subcarrier Spacing index {}", val)
            }
            OranSectionType0Error::InvalidReMask(mask) => {
                write!(f, "Invalid 12-bit RE mask 0x{:04X} (exceeds 0x0FFF)", mask)
            }
            OranSectionType0Error::CusError(msg) => {
                write!(f, "O-RAN CUS error: {}", msg)
            }
        }
    }
}

impl std::error::Error for OranSectionType0Error {}

impl From<OranError> for OranSectionType0Error {
    fn from(err: OranError) -> Self {
        match err {
            OranError::Truncated { need, got } => OranSectionType0Error::Truncated { need, got },
            OranError::UnsupportedPayloadVersion(v) => {
                OranSectionType0Error::UnsupportedPayloadVersion(v)
            }
            OranError::UnsupportedSectionType(t) => {
                OranSectionType0Error::UnsupportedSectionType(t)
            }
            OranError::SectionCountMismatch { declared, parsed } => {
                OranSectionType0Error::SectionCountMismatch { declared, parsed }
            }
            OranError::SectionExtensionUnsupported(id) => {
                OranSectionType0Error::SectionExtensionUnsupported(id)
            }
            OranError::FieldOutOfRange { field, value } => OranSectionType0Error::FieldOutOfRange {
                field,
                value,
                max: u32::MAX,
            },
            other => OranSectionType0Error::CusError(format!("{}", other)),
        }
    }
}

/// FFT Size configuration defined in O-RAN.WG4.CUS Table 7-18.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OranFftSize {
    Fft128 = 7,
    Fft256 = 8,
    Fft512 = 9,
    Fft1024 = 10,
    Fft2048 = 11,
    Fft4096 = 12,
    Fft1536 = 13,
}

impl OranFftSize {
    pub fn from_u4(val: u8) -> Result<Self, OranSectionType0Error> {
        match val & 0x0F {
            7 => Ok(OranFftSize::Fft128),
            8 => Ok(OranFftSize::Fft256),
            9 => Ok(OranFftSize::Fft512),
            10 => Ok(OranFftSize::Fft1024),
            11 => Ok(OranFftSize::Fft2048),
            12 => Ok(OranFftSize::Fft4096),
            13 => Ok(OranFftSize::Fft1536),
            other => Err(OranSectionType0Error::InvalidFftSize(other)),
        }
    }

    pub fn to_u4(self) -> u8 {
        self as u8
    }

    pub fn points(self) -> usize {
        match self {
            OranFftSize::Fft128 => 128,
            OranFftSize::Fft256 => 256,
            OranFftSize::Fft512 => 512,
            OranFftSize::Fft1024 => 1024,
            OranFftSize::Fft1536 => 1536,
            OranFftSize::Fft2048 => 2048,
            OranFftSize::Fft4096 => 4096,
        }
    }
}

/// Subcarrier Spacing (SCS) configuration defined in O-RAN.WG4.CUS Table 7-19.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OranScs {
    Scs15kHz = 0,
    Scs30kHz = 1,
    Scs60kHz = 2,
    Scs120kHz = 3,
    Scs240kHz = 4,
    Scs1_25kHz = 12,
    Scs3_75kHz = 13,
    Scs5kHz = 14,
    Scs7_5kHz = 15,
}

impl OranScs {
    pub fn from_u4(val: u8) -> Result<Self, OranSectionType0Error> {
        match val & 0x0F {
            0 => Ok(OranScs::Scs15kHz),
            1 => Ok(OranScs::Scs30kHz),
            2 => Ok(OranScs::Scs60kHz),
            3 => Ok(OranScs::Scs120kHz),
            4 => Ok(OranScs::Scs240kHz),
            12 => Ok(OranScs::Scs1_25kHz),
            13 => Ok(OranScs::Scs3_75kHz),
            14 => Ok(OranScs::Scs5kHz),
            15 => Ok(OranScs::Scs7_5kHz),
            other => Err(OranSectionType0Error::InvalidScs(other)),
        }
    }

    pub fn to_u4(self) -> u8 {
        self as u8
    }

    pub fn khz(self) -> f64 {
        match self {
            OranScs::Scs15kHz => 15.0,
            OranScs::Scs30kHz => 30.0,
            OranScs::Scs60kHz => 60.0,
            OranScs::Scs120kHz => 120.0,
            OranScs::Scs240kHz => 240.0,
            OranScs::Scs1_25kHz => 1.25,
            OranScs::Scs3_75kHz => 3.75,
            OranScs::Scs5kHz => 5.0,
            OranScs::Scs7_5kHz => 7.5,
        }
    }

    /// Nominal symbol duration in microseconds (without cyclic prefix).
    pub fn nominal_symbol_duration_us(self) -> f64 {
        1000.0 / self.khz()
    }

    /// Slot duration in microseconds for standard NR numerologies (mu = 0..4).
    pub fn slot_duration_us(self) -> Option<f64> {
        match self {
            OranScs::Scs15kHz => Some(1000.0),
            OranScs::Scs30kHz => Some(500.0),
            OranScs::Scs60kHz => Some(250.0),
            OranScs::Scs120kHz => Some(125.0),
            OranScs::Scs240kHz => Some(62.5),
            _ => None,
        }
    }
}

/// 8-bit Frame Structure field: FFT size (bits 7-4) and Subcarrier Spacing (bits 3-0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameStructure {
    pub fft_size: OranFftSize,
    pub scs: OranScs,
}

impl FrameStructure {
    pub fn new(fft_size: OranFftSize, scs: OranScs) -> Self {
        Self { fft_size, scs }
    }

    pub fn to_u8(&self) -> u8 {
        ((self.fft_size.to_u4() & 0x0F) << 4) | (self.scs.to_u4() & 0x0F)
    }

    pub fn from_u8(val: u8) -> Result<Self, OranSectionType0Error> {
        let fft_size = OranFftSize::from_u4(val >> 4)?;
        let scs = OranScs::from_u4(val & 0x0F)?;
        Ok(Self { fft_size, scs })
    }
}

/// Common Header for C-Plane Section Type 0 (12 bytes total: 4 bytes radio header + 8 bytes section type 0 common).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OranSectionType0CommonHeader {
    pub radio_header: OranRadioHeader,
    pub num_sections: u8,
    pub section_type: u8,
    pub time_offset: u16,
    pub frame_structure: FrameStructure,
    pub cp_length: u16,
}

impl OranSectionType0CommonHeader {
    pub fn new(
        radio_header: OranRadioHeader,
        num_sections: u8,
        time_offset: u16,
        frame_structure: FrameStructure,
        cp_length: u16,
    ) -> Self {
        Self {
            radio_header,
            num_sections,
            section_type: ORAN_SECTION_TYPE_0,
            time_offset,
            frame_structure,
            cp_length,
        }
    }

    pub fn serialize(
        &self,
    ) -> Result<[u8; ORAN_SECTION_TYPE_0_COMMON_HEADER_LEN], OranSectionType0Error> {
        self.radio_header.validate()?;
        if self.section_type != ORAN_SECTION_TYPE_0 {
            return Err(OranSectionType0Error::UnsupportedSectionType(
                self.section_type,
            ));
        }
        let mut out = [0u8; ORAN_SECTION_TYPE_0_COMMON_HEADER_LEN];
        out[0..4].copy_from_slice(&self.radio_header.serialize());
        out[4] = self.num_sections;
        out[5] = self.section_type;
        out[6..8].copy_from_slice(&self.time_offset.to_be_bytes());
        out[8] = self.frame_structure.to_u8();
        out[9..11].copy_from_slice(&self.cp_length.to_be_bytes());
        out[11] = 0; // reserved octet
        Ok(out)
    }

    pub fn parse(data: &[u8]) -> Result<Self, OranSectionType0Error> {
        if data.len() < ORAN_SECTION_TYPE_0_COMMON_HEADER_LEN {
            return Err(OranSectionType0Error::Truncated {
                need: ORAN_SECTION_TYPE_0_COMMON_HEADER_LEN,
                got: data.len(),
            });
        }
        let radio_header = OranRadioHeader::parse(&data[0..4])?;
        let num_sections = data[4];
        let section_type = data[5];
        if section_type != ORAN_SECTION_TYPE_0 {
            return Err(OranSectionType0Error::UnsupportedSectionType(section_type));
        }
        let time_offset = u16::from_be_bytes([data[6], data[7]]);
        let frame_structure = FrameStructure::from_u8(data[8])?;
        let cp_length = u16::from_be_bytes([data[9], data[10]]);
        // data[11] is reserved

        Ok(Self {
            radio_header,
            num_sections,
            section_type,
            time_offset,
            frame_structure,
            cp_length,
        })
    }
}

/// Single Section Type 0 section body (8 bytes).
///
/// Indicates PRBs and symbols that are unused/blanked/idle for the given carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OranSectionType0Section {
    pub section_id: u16,
    /// `rb`: false selects every PRB, true selects every other PRB.
    pub rb: bool,
    /// `sym_inc`: whether the symbol number increments for subsequent sections.
    pub sym_inc: bool,
    pub start_prbc: u16,
    /// Number of contiguous PRBs blanked (0 indicates all remaining PRBs of the carrier).
    pub num_prbc: u8,
    /// 12-bit mask of resource elements within each PRB that are blanked (0x0FFF = all 12 REs).
    pub re_mask: u16,
    /// Number of consecutive symbols covered by this blanking section (1..14).
    pub num_symbol: u8,
    /// Extension flag (`ef`): true if section extensions follow.
    pub ef: bool,
    /// Reserved 15-bit field.
    pub reserved: u16,
}

impl OranSectionType0Section {
    pub fn new(section_id: u16, start_prbc: u16, num_prbc: u8, num_symbol: u8) -> Self {
        Self {
            section_id,
            rb: false,
            sym_inc: false,
            start_prbc,
            num_prbc,
            re_mask: 0x0FFF, // Default: all 12 subcarriers of the PRB blanked
            num_symbol,
            ef: false,
            reserved: 0,
        }
    }

    pub fn with_re_mask(mut self, re_mask: u16) -> Result<Self, OranSectionType0Error> {
        if re_mask > 0x0FFF {
            return Err(OranSectionType0Error::InvalidReMask(re_mask));
        }
        self.re_mask = re_mask;
        Ok(self)
    }

    pub fn with_every_other_rb(mut self, rb: bool) -> Self {
        self.rb = rb;
        self
    }

    pub fn with_sym_inc(mut self, sym_inc: bool) -> Self {
        self.sym_inc = sym_inc;
        self
    }

    pub fn validate(&self) -> Result<(), OranSectionType0Error> {
        if self.section_id > 0x0FFF {
            return Err(OranSectionType0Error::FieldOutOfRange {
                field: "sectionId",
                value: self.section_id as u32,
                max: 0x0FFF,
            });
        }
        if self.start_prbc > 0x03FF {
            return Err(OranSectionType0Error::FieldOutOfRange {
                field: "startPrbc",
                value: self.start_prbc as u32,
                max: 0x03FF,
            });
        }
        if self.re_mask > 0x0FFF {
            return Err(OranSectionType0Error::InvalidReMask(self.re_mask));
        }
        if self.num_symbol == 0 || self.num_symbol > 14 {
            return Err(OranSectionType0Error::FieldOutOfRange {
                field: "numSymbol",
                value: self.num_symbol as u32,
                max: 14,
            });
        }
        if self.reserved > 0x7FFF {
            return Err(OranSectionType0Error::FieldOutOfRange {
                field: "reserved",
                value: self.reserved as u32,
                max: 0x7FFF,
            });
        }
        Ok(())
    }

    /// Effective number of PRBs blanked given carrier bandwidth.
    pub fn effective_prb_count(&self, carrier_prbs: u16) -> u16 {
        if self.num_prbc == 0 {
            carrier_prbs.saturating_sub(self.start_prbc)
        } else {
            self.num_prbc as u16
        }
    }

    pub fn serialize(
        &self,
    ) -> Result<[u8; ORAN_SECTION_TYPE_0_SECTION_LEN], OranSectionType0Error> {
        self.validate()?;
        let mut out = [0u8; ORAN_SECTION_TYPE_0_SECTION_LEN];
        out[0] = (self.section_id >> 4) as u8;
        out[1] = (((self.section_id & 0x0F) as u8) << 4)
            | (u8::from(self.rb) << 3)
            | (u8::from(self.sym_inc) << 2)
            | (((self.start_prbc >> 8) & 0x03) as u8);
        out[2] = (self.start_prbc & 0xFF) as u8;
        out[3] = self.num_prbc;
        out[4] = (self.re_mask >> 4) as u8;
        out[5] = (((self.re_mask & 0x0F) as u8) << 4) | (self.num_symbol & 0x0F);
        out[6] = (u8::from(self.ef) << 7) | (((self.reserved >> 8) & 0x7F) as u8);
        out[7] = (self.reserved & 0xFF) as u8;
        Ok(out)
    }

    pub fn parse(data: &[u8]) -> Result<Self, OranSectionType0Error> {
        if data.len() < ORAN_SECTION_TYPE_0_SECTION_LEN {
            return Err(OranSectionType0Error::Truncated {
                need: ORAN_SECTION_TYPE_0_SECTION_LEN,
                got: data.len(),
            });
        }
        let section_id = (((data[0] as u16) << 4) | ((data[1] >> 4) as u16)) & 0x0FFF;
        let rb = (data[1] & 0x08) != 0;
        let sym_inc = (data[1] & 0x04) != 0;
        let start_prbc = (((data[1] & 0x03) as u16) << 8) | (data[2] as u16);
        let num_prbc = data[3];
        let re_mask = (((data[4] as u16) << 4) | ((data[5] >> 4) as u16)) & 0x0FFF;
        let num_symbol = data[5] & 0x0F;
        let ef = (data[6] & 0x80) != 0;
        let reserved = (((data[6] & 0x7F) as u16) << 8) | (data[7] as u16);

        if ef {
            return Err(OranSectionType0Error::SectionExtensionUnsupported(
                section_id,
            ));
        }

        let section = Self {
            section_id,
            rb,
            sym_inc,
            start_prbc,
            num_prbc,
            re_mask,
            num_symbol,
            ef,
            reserved,
        };
        section.validate()?;
        Ok(section)
    }
}

/// A complete C-Plane Section Type 0 message scheduling one or more blanking/guardband reservations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OranSectionType0Message {
    pub common_header: OranSectionType0CommonHeader,
    pub sections: Vec<OranSectionType0Section>,
}

impl OranSectionType0Message {
    pub fn new(
        common_header: OranSectionType0CommonHeader,
        sections: Vec<OranSectionType0Section>,
    ) -> Self {
        let mut msg = Self {
            common_header,
            sections,
        };
        msg.common_header.num_sections = msg.sections.len() as u8;
        msg
    }

    pub fn serialize(&self) -> Result<Vec<u8>, OranSectionType0Error> {
        if self.sections.len() > u8::MAX as usize {
            return Err(OranSectionType0Error::FieldOutOfRange {
                field: "numberOfSections",
                value: self.sections.len() as u32,
                max: u8::MAX as u32,
            });
        }
        let mut out = Vec::with_capacity(
            ORAN_SECTION_TYPE_0_COMMON_HEADER_LEN
                + ORAN_SECTION_TYPE_0_SECTION_LEN * self.sections.len(),
        );
        let mut hdr = self.common_header;
        hdr.num_sections = self.sections.len() as u8;
        out.extend_from_slice(&hdr.serialize()?);
        for section in &self.sections {
            out.extend_from_slice(&section.serialize()?);
        }
        Ok(out)
    }

    pub fn parse(data: &[u8]) -> Result<Self, OranSectionType0Error> {
        if data.len() < ORAN_SECTION_TYPE_0_COMMON_HEADER_LEN {
            return Err(OranSectionType0Error::Truncated {
                need: ORAN_SECTION_TYPE_0_COMMON_HEADER_LEN,
                got: data.len(),
            });
        }
        let common_header =
            OranSectionType0CommonHeader::parse(&data[0..ORAN_SECTION_TYPE_0_COMMON_HEADER_LEN])?;
        let declared = common_header.num_sections as usize;
        let body = &data[ORAN_SECTION_TYPE_0_COMMON_HEADER_LEN..];

        if body.len() < declared * ORAN_SECTION_TYPE_0_SECTION_LEN {
            return Err(OranSectionType0Error::Truncated {
                need: ORAN_SECTION_TYPE_0_COMMON_HEADER_LEN
                    + declared * ORAN_SECTION_TYPE_0_SECTION_LEN,
                got: data.len(),
            });
        }

        let mut sections = Vec::with_capacity(declared);
        for i in 0..declared {
            let start = i * ORAN_SECTION_TYPE_0_SECTION_LEN;
            let end = start + ORAN_SECTION_TYPE_0_SECTION_LEN;
            let section = OranSectionType0Section::parse(&body[start..end])?;
            sections.push(section);
        }

        Ok(Self {
            common_header,
            sections,
        })
    }
}

/// Operational reason for reserving or blanking radio resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlankingReason {
    /// Blanking for LTE / 5G NR Dynamic Spectrum Sharing (DSS) CRS/MBSFN coexistence.
    DssLteCoexistence,
    /// Radar or Dynamic Frequency Selection (DFS) exclusion zone protection.
    RadarProtection,
    /// RF transceiver and antenna array self-calibration quiet interval.
    CalibrationQuietWindow,
    /// Inter-Cell Interference Coordination (ICIC / eICIC) almost blank subframe.
    InterCellInterferenceCoordination,
    /// Carrier aggregation edge guardband protection.
    CarrierGuardband,
    /// Power amplifier energy saving micro-sleep.
    PowerSavingMicroSleep,
}

/// High-level blanking reservation specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlankingReservation {
    pub reason: BlankingReason,
    pub section_id: u16,
    pub start_symbol: u8,
    pub num_symbols: u8,
    pub start_prbc: u16,
    pub num_prbc: u8,
    pub re_mask: u16,
}

impl BlankingReservation {
    pub fn new(
        reason: BlankingReason,
        section_id: u16,
        start_symbol: u8,
        num_symbols: u8,
        start_prbc: u16,
        num_prbc: u8,
        re_mask: u16,
    ) -> Self {
        Self {
            reason,
            section_id,
            start_symbol,
            num_symbols,
            start_prbc,
            num_prbc,
            re_mask,
        }
    }

    /// Converts into a wire-format Section Type 0 section.
    pub fn to_section(&self) -> OranSectionType0Section {
        OranSectionType0Section {
            section_id: self.section_id,
            rb: false,
            sym_inc: false,
            start_prbc: self.start_prbc,
            num_prbc: self.num_prbc,
            re_mask: self.re_mask,
            num_symbol: self.num_symbols,
            ef: false,
            reserved: 0,
        }
    }
}

/// Collision details when a transmission allocation overlaps with a blanked reservation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlankingCollision {
    pub section_id: u16,
    pub reason: BlankingReason,
    pub symbol: u8,
    pub prb: u16,
    pub overlapping_re_mask: u16,
}

/// Detailed report on power amplifier energy saving achieved by Section Type 0 blanking.
#[derive(Debug, Clone, PartialEq)]
pub struct MicroSleepReport {
    /// Total Resource Elements in the slot (carrier_prbs * 12 * 14).
    pub total_slot_res: usize,
    /// Total blanked Resource Elements.
    pub blanked_res: usize,
    /// Blanked ratio across the entire slot (0.0 to 1.0).
    pub blanking_ratio: f64,
    /// Number of OFDM symbols where 100% of carrier PRBs are blanked.
    pub fully_blanked_symbols: usize,
    /// Duration in microseconds where PA can enter micro-sleep.
    pub sleep_duration_us: f64,
    /// Estimated energy savings in Joules for this slot (based on PA power rating).
    pub estimated_energy_saved_joules: f64,
    /// Effective duty cycle reduction percentage (0.0 to 100.0).
    pub duty_cycle_reduction_percent: f64,
}

/// 2D Time-Frequency Resource Grid Engine for Blanking and Guardband Management.
///
/// Tracks 14 OFDM symbols and up to 275+ PRBs per slot, mapping RE masks to detect
/// transmission conflicts and calculate power saving metrics.
#[derive(Debug, Clone)]
pub struct BlankingGrid {
    pub carrier_prbs: u16,
    pub scs: OranScs,
    reservations: Vec<BlankingReservation>,
    /// 14 symbols x carrier_prbs grid, storing (RE bitmask, Option<BlankingReason>).
    grid: Vec<Vec<(u16, Option<BlankingReason>)>>,
}

impl BlankingGrid {
    pub fn new(carrier_prbs: u16, scs: OranScs) -> Self {
        let grid = vec![vec![(0u16, None); carrier_prbs as usize]; NR_SYMBOLS_PER_SLOT];
        Self {
            carrier_prbs,
            scs,
            reservations: Vec::new(),
            grid,
        }
    }

    /// Adds a blanking reservation to the grid.
    pub fn add_reservation(
        &mut self,
        res: BlankingReservation,
    ) -> Result<(), OranSectionType0Error> {
        if res.start_symbol as usize >= NR_SYMBOLS_PER_SLOT {
            return Err(OranSectionType0Error::FieldOutOfRange {
                field: "start_symbol",
                value: res.start_symbol as u32,
                max: (NR_SYMBOLS_PER_SLOT - 1) as u32,
            });
        }
        let end_sym = (res.start_symbol + res.num_symbols) as usize;
        let end_sym = end_sym.min(NR_SYMBOLS_PER_SLOT);

        let prb_count = if res.num_prbc == 0 {
            self.carrier_prbs.saturating_sub(res.start_prbc) as usize
        } else {
            (res.num_prbc as usize).min(self.carrier_prbs.saturating_sub(res.start_prbc) as usize)
        };

        let start_prb = res.start_prbc as usize;
        for sym in (res.start_symbol as usize)..end_sym {
            for prb in start_prb..(start_prb + prb_count) {
                if prb < self.carrier_prbs as usize {
                    self.grid[sym][prb].0 |= res.re_mask & 0x0FFF;
                    self.grid[sym][prb].1 = Some(res.reason);
                }
            }
        }
        self.reservations.push(res);
        Ok(())
    }

    /// Checks if a scheduled transmission allocation collides with any blanked reservation.
    ///
    /// Returns the first detected collision if any overlap occurs.
    pub fn check_collision(
        &self,
        start_symbol: u8,
        num_symbol: u8,
        start_prbc: u16,
        num_prbc: u8,
        tx_re_mask: u16,
    ) -> Option<BlankingCollision> {
        let end_sym = ((start_symbol + num_symbol) as usize).min(NR_SYMBOLS_PER_SLOT);
        let prb_count = if num_prbc == 0 {
            self.carrier_prbs.saturating_sub(start_prbc) as usize
        } else {
            (num_prbc as usize).min(self.carrier_prbs.saturating_sub(start_prbc) as usize)
        };
        let start_prb = start_prbc as usize;

        for sym in (start_symbol as usize)..end_sym {
            for prb in start_prb..(start_prb + prb_count) {
                if prb < self.carrier_prbs as usize {
                    let (blank_mask, reason) = self.grid[sym][prb];
                    let overlap = blank_mask & tx_re_mask & 0x0FFF;
                    if overlap != 0 {
                        let res_id = self
                            .reservations
                            .iter()
                            .find(|r| {
                                sym >= r.start_symbol as usize
                                    && sym < (r.start_symbol + r.num_symbols) as usize
                                    && prb >= r.start_prbc as usize
                                    && (r.num_prbc == 0
                                        || prb < (r.start_prbc + r.num_prbc as u16) as usize)
                            })
                            .map(|r| r.section_id)
                            .unwrap_or(0);

                        return Some(BlankingCollision {
                            section_id: res_id,
                            reason: reason.unwrap_or(BlankingReason::DssLteCoexistence),
                            symbol: sym as u8,
                            prb: prb as u16,
                            overlapping_re_mask: overlap,
                        });
                    }
                }
            }
        }
        None
    }

    /// Evaluates power amplifier micro-sleep metrics for the slot.
    pub fn calculate_power_savings(&self, pa_power_watts: f64) -> MicroSleepReport {
        let total_slot_res =
            (self.carrier_prbs as usize) * NR_SUBCARRIERS_PER_PRB * NR_SYMBOLS_PER_SLOT;
        let mut blanked_res = 0usize;
        let mut fully_blanked_symbols = 0usize;

        for sym in 0..NR_SYMBOLS_PER_SLOT {
            let mut sym_blanked_re = 0usize;
            let full_symbol_re = (self.carrier_prbs as usize) * NR_SUBCARRIERS_PER_PRB;

            for prb in 0..(self.carrier_prbs as usize) {
                let mask = self.grid[sym][prb].0;
                sym_blanked_re += mask.count_ones() as usize;
            }

            blanked_res += sym_blanked_re;
            if sym_blanked_re == full_symbol_re {
                fully_blanked_symbols += 1;
            }
        }

        let blanking_ratio = if total_slot_res > 0 {
            blanked_res as f64 / total_slot_res as f64
        } else {
            0.0
        };

        let sym_duration_us = self.scs.nominal_symbol_duration_us();
        let sleep_duration_us = (fully_blanked_symbols as f64) * sym_duration_us;

        // Energy (Joules) = Power (Watts) * Time (Seconds)
        let estimated_energy_saved_joules = pa_power_watts * (sleep_duration_us * 1e-6);
        let duty_cycle_reduction_percent = blanking_ratio * 100.0;

        MicroSleepReport {
            total_slot_res,
            blanked_res,
            blanking_ratio,
            fully_blanked_symbols,
            sleep_duration_us,
            estimated_energy_saved_joules,
            duty_cycle_reduction_percent,
        }
    }

    /// Compiles all active reservations into an O-RAN Section Type 0 Message.
    pub fn compile_message(
        &self,
        radio_header: OranRadioHeader,
        time_offset: u16,
        fft_size: OranFftSize,
        cp_length: u16,
    ) -> Result<OranSectionType0Message, OranSectionType0Error> {
        let frame_structure = FrameStructure::new(fft_size, self.scs);
        let sections: Vec<OranSectionType0Section> =
            self.reservations.iter().map(|r| r.to_section()).collect();
        let common_header = OranSectionType0CommonHeader::new(
            radio_header,
            sections.len() as u8,
            time_offset,
            frame_structure,
            cp_length,
        );
        Ok(OranSectionType0Message::new(common_header, sections))
    }
}
