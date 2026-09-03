//! O-RAN WG4 Open Fronthaul Control / User Plane (O-RAN.WG4.CUS-Plane specification).
//!
//! The application layer carried inside eCPRI message type 0 (User Plane) and type 2
//! (Real-Time Control Plane) between an O-DU and an O-RU. Implements the extended
//! Antenna-Carrier identifier (eAxC ID) bit packing, the 4-byte radio application
//! common header (dataDirection / frameId / subframeId / slotId / symbolId), the
//! user data compression header, U-Plane PRB sections and C-Plane Section Type 1
//! sections, plus a per-flow monitor that detects out-of-order radio symbols.
//!
//! The eAxC ID replaces the transport level `PC_ID` (U-Plane) and `RTC_ID` (C-Plane)
//! fields of [`crate::ecpri`].

use std::collections::HashMap;
use std::fmt;

/// `payloadVersion` of the application common header defined by the CUS-Plane spec.
pub const ORAN_PAYLOAD_VERSION: u8 = 1;
/// Size of the radio application common header shared by C-Plane and U-Plane.
pub const ORAN_RADIO_HEADER_LEN: usize = 4;
/// Size of the C-Plane common header for Section Type 1 (radio header + 4 bytes).
pub const ORAN_CPLANE_HEADER_LEN: usize = 8;
/// Size of one C-Plane Section Type 1 section body.
pub const ORAN_CPLANE_SECTION_LEN: usize = 8;
/// Size of a U-Plane section header before any compression header.
pub const ORAN_UPLANE_SECTION_LEN: usize = 4;
/// OFDM symbols per slot with normal cyclic prefix (3GPP TS 38.211).
pub const NR_SYMBOLS_PER_SLOT: u8 = 14;
/// Subframes in one 10 ms 5G NR radio frame.
pub const NR_SUBFRAMES_PER_FRAME: u8 = 10;

/// Errors raised while decoding an O-RAN fronthaul application payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OranError {
    /// The buffer is shorter than the mandatory header or section fields.
    Truncated { need: usize, got: usize },
    /// `payloadVersion` is not the value mandated by the CUS-Plane spec.
    UnsupportedPayloadVersion(u8),
    /// The four eAxC ID subfield widths do not add up to 16 bits.
    InvalidEaxcFormat(u8),
    /// A subfield value does not fit the width configured for it.
    EaxcFieldOverflow {
        field: &'static str,
        value: u16,
        bits: u8,
    },
    /// Only Section Type 1 carries the header layout implemented here.
    UnsupportedSectionType(u8),
    /// The section sets `ef`, so section extensions follow that are not decoded.
    SectionExtensionUnsupported(u16),
    /// `numberOfSections` disagrees with the bytes actually present.
    SectionCountMismatch { declared: u8, parsed: usize },
    /// A field carries a value outside the range its bit width allows.
    FieldOutOfRange { field: &'static str, value: u32 },
}

impl fmt::Display for OranError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OranError::Truncated { need, got } => {
                write!(
                    f,
                    "O-RAN payload truncated: need {} bytes, got {}",
                    need, got
                )
            }
            OranError::UnsupportedPayloadVersion(v) => {
                write!(f, "Unsupported O-RAN payload version {}", v)
            }
            OranError::InvalidEaxcFormat(total) => write!(
                f,
                "eAxC ID subfield widths total {} bits instead of 16",
                total
            ),
            OranError::EaxcFieldOverflow { field, value, bits } => write!(
                f,
                "eAxC {} value {} does not fit in {} bits",
                field, value, bits
            ),
            OranError::UnsupportedSectionType(t) => {
                write!(f, "Unsupported O-RAN C-Plane section type {}", t)
            }
            OranError::SectionExtensionUnsupported(id) => {
                write!(f, "Section {} carries unsupported section extensions", id)
            }
            OranError::SectionCountMismatch { declared, parsed } => write!(
                f,
                "numberOfSections declared {} but {} sections are present",
                declared, parsed
            ),
            OranError::FieldOutOfRange { field, value } => {
                write!(f, "O-RAN field {} value {} is out of range", field, value)
            }
        }
    }
}

impl std::error::Error for OranError {}

/// Bit widths of the four eAxC ID subfields; they must total 16 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EaxcIdFormat {
    pub du_port_bits: u8,
    pub band_sector_bits: u8,
    pub cc_bits: u8,
    pub ru_port_bits: u8,
}

impl EaxcIdFormat {
    pub fn new(
        du_port_bits: u8,
        band_sector_bits: u8,
        cc_bits: u8,
        ru_port_bits: u8,
    ) -> Result<Self, OranError> {
        let total = du_port_bits + band_sector_bits + cc_bits + ru_port_bits;
        if total != 16 {
            return Err(OranError::InvalidEaxcFormat(total));
        }
        Ok(EaxcIdFormat {
            du_port_bits,
            band_sector_bits,
            cc_bits,
            ru_port_bits,
        })
    }

    /// Common O-RU configuration: 2 bits O-DU port, 4 band/sector, 4 carrier, 6 RU port.
    pub fn typical() -> Self {
        EaxcIdFormat {
            du_port_bits: 2,
            band_sector_bits: 4,
            cc_bits: 4,
            ru_port_bits: 6,
        }
    }

    fn mask(bits: u8) -> u16 {
        if bits >= 16 {
            u16::MAX
        } else {
            (1u16 << bits) - 1
        }
    }
}

/// Extended Antenna-Carrier identifier: four packed subfields in one 16-bit word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EaxcId {
    pub du_port_id: u16,
    pub band_sector_id: u16,
    pub cc_id: u16,
    pub ru_port_id: u16,
}

impl EaxcId {
    pub fn new(du_port_id: u16, band_sector_id: u16, cc_id: u16, ru_port_id: u16) -> Self {
        EaxcId {
            du_port_id,
            band_sector_id,
            cc_id,
            ru_port_id,
        }
    }

    /// Packs the subfields most significant first: DU_Port | BandSector | CC | RU_Port.
    pub fn pack(&self, format: EaxcIdFormat) -> Result<u16, OranError> {
        let fields: [(&'static str, u16, u8); 4] = [
            ("DU_Port_ID", self.du_port_id, format.du_port_bits),
            (
                "BandSector_ID",
                self.band_sector_id,
                format.band_sector_bits,
            ),
            ("CC_ID", self.cc_id, format.cc_bits),
            ("RU_Port_ID", self.ru_port_id, format.ru_port_bits),
        ];
        let mut packed: u16 = 0;
        for (field, value, bits) in fields {
            if value > EaxcIdFormat::mask(bits) {
                return Err(OranError::EaxcFieldOverflow { field, value, bits });
            }
            packed = (packed << bits) | value;
        }
        Ok(packed)
    }

    pub fn unpack(raw: u16, format: EaxcIdFormat) -> Self {
        let ru_shift = 0;
        let cc_shift = format.ru_port_bits;
        let band_shift = cc_shift + format.cc_bits;
        let du_shift = band_shift + format.band_sector_bits;
        EaxcId {
            du_port_id: (raw >> du_shift) & EaxcIdFormat::mask(format.du_port_bits),
            band_sector_id: (raw >> band_shift) & EaxcIdFormat::mask(format.band_sector_bits),
            cc_id: (raw >> cc_shift) & EaxcIdFormat::mask(format.cc_bits),
            ru_port_id: (raw >> ru_shift) & EaxcIdFormat::mask(format.ru_port_bits),
        }
    }
}

/// Direction of a fronthaul message relative to the radio link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataDirection {
    /// 0: uplink, O-RU towards O-DU.
    Uplink,
    /// 1: downlink, O-DU towards O-RU.
    Downlink,
}

impl DataDirection {
    pub fn from_bit(bit: u8) -> Self {
        if bit & 0x01 == 1 {
            DataDirection::Downlink
        } else {
            DataDirection::Uplink
        }
    }

    pub fn to_bit(self) -> u8 {
        match self {
            DataDirection::Uplink => 0,
            DataDirection::Downlink => 1,
        }
    }
}

/// 4-byte radio application common header shared by the C-Plane and the U-Plane.
///
/// ```text
///  0                   1                   2                   3
///  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-----+-------+---------------+-------+-------+-+-+-----------+
/// |D| ver |filtIdx|    frameId    |subFrmI| slotId    |  symbolId |
/// +-+-----+-------+---------------+-------+-------+-+-+-----------+
/// ```
///
/// In a C-Plane message the last field is `startSymbolId` instead of `symbolId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OranRadioHeader {
    pub data_direction: DataDirection,
    pub payload_version: u8,
    /// Filter index selecting the channel filter (0 = standard channel).
    pub filter_index: u8,
    pub frame_id: u8,
    pub subframe_id: u8,
    pub slot_id: u8,
    /// `symbolId` for the U-Plane, `startSymbolId` for the C-Plane.
    pub symbol_id: u8,
}

impl OranRadioHeader {
    pub fn new(
        data_direction: DataDirection,
        frame_id: u8,
        subframe_id: u8,
        slot_id: u8,
        symbol_id: u8,
    ) -> Self {
        OranRadioHeader {
            data_direction,
            payload_version: ORAN_PAYLOAD_VERSION,
            filter_index: 0,
            frame_id,
            subframe_id,
            slot_id,
            symbol_id,
        }
    }

    pub fn validate(&self) -> Result<(), OranError> {
        if self.subframe_id >= NR_SUBFRAMES_PER_FRAME {
            return Err(OranError::FieldOutOfRange {
                field: "subframeId",
                value: self.subframe_id as u32,
            });
        }
        if self.slot_id > 0x3F {
            return Err(OranError::FieldOutOfRange {
                field: "slotId",
                value: self.slot_id as u32,
            });
        }
        if self.symbol_id > 0x3F {
            return Err(OranError::FieldOutOfRange {
                field: "symbolId",
                value: self.symbol_id as u32,
            });
        }
        Ok(())
    }

    pub fn serialize(&self) -> [u8; ORAN_RADIO_HEADER_LEN] {
        let mut out = [0u8; ORAN_RADIO_HEADER_LEN];
        out[0] = (self.data_direction.to_bit() << 7)
            | ((self.payload_version & 0x07) << 4)
            | (self.filter_index & 0x0F);
        out[1] = self.frame_id;
        // slotId straddles the octet boundary: 4 high bits here, 2 low bits in the next octet.
        out[2] = ((self.subframe_id & 0x0F) << 4) | ((self.slot_id >> 2) & 0x0F);
        out[3] = ((self.slot_id & 0x03) << 6) | (self.symbol_id & 0x3F);
        out
    }

    pub fn parse(data: &[u8]) -> Result<Self, OranError> {
        if data.len() < ORAN_RADIO_HEADER_LEN {
            return Err(OranError::Truncated {
                need: ORAN_RADIO_HEADER_LEN,
                got: data.len(),
            });
        }
        let payload_version = (data[0] >> 4) & 0x07;
        if payload_version != ORAN_PAYLOAD_VERSION {
            return Err(OranError::UnsupportedPayloadVersion(payload_version));
        }
        Ok(OranRadioHeader {
            data_direction: DataDirection::from_bit(data[0] >> 7),
            payload_version,
            filter_index: data[0] & 0x0F,
            frame_id: data[1],
            subframe_id: (data[2] >> 4) & 0x0F,
            slot_id: ((data[2] & 0x0F) << 2) | (data[3] >> 6),
            symbol_id: data[3] & 0x3F,
        })
    }

    /// Position of this symbol inside a 256-frame hyperframe, for ordering checks.
    ///
    /// `numerology` is the 3GPP subcarrier spacing exponent: 2^mu slots per subframe.
    pub fn symbol_index(&self, numerology: u8) -> u32 {
        let slots_per_subframe = 1u32 << numerology;
        let subframe =
            self.frame_id as u32 * NR_SUBFRAMES_PER_FRAME as u32 + self.subframe_id as u32;
        (subframe * slots_per_subframe + self.slot_id as u32) * NR_SYMBOLS_PER_SLOT as u32
            + self.symbol_id as u32
    }
}

/// IQ sample compression method (`udCompMeth`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UdCompMethod {
    NoCompression,
    BlockFloatingPoint,
    BlockScaling,
    MuLaw,
    ModulationCompression,
    BfpSelectiveRe,
    ModCompSelectiveRe,
    Reserved(u8),
}

impl UdCompMethod {
    pub fn from_u4(code: u8) -> Self {
        match code & 0x0F {
            0x0 => UdCompMethod::NoCompression,
            0x1 => UdCompMethod::BlockFloatingPoint,
            0x2 => UdCompMethod::BlockScaling,
            0x3 => UdCompMethod::MuLaw,
            0x4 => UdCompMethod::ModulationCompression,
            0x5 => UdCompMethod::BfpSelectiveRe,
            0x6 => UdCompMethod::ModCompSelectiveRe,
            other => UdCompMethod::Reserved(other),
        }
    }

    pub fn to_u4(self) -> u8 {
        match self {
            UdCompMethod::NoCompression => 0x0,
            UdCompMethod::BlockFloatingPoint => 0x1,
            UdCompMethod::BlockScaling => 0x2,
            UdCompMethod::MuLaw => 0x3,
            UdCompMethod::ModulationCompression => 0x4,
            UdCompMethod::BfpSelectiveRe => 0x5,
            UdCompMethod::ModCompSelectiveRe => 0x6,
            UdCompMethod::Reserved(c) => c & 0x0F,
        }
    }

    /// Block floating point methods prefix every PRB with a shared exponent byte.
    pub fn has_per_prb_comp_param(self) -> bool {
        matches!(
            self,
            UdCompMethod::BlockFloatingPoint
                | UdCompMethod::BlockScaling
                | UdCompMethod::ModulationCompression
                | UdCompMethod::BfpSelectiveRe
                | UdCompMethod::ModCompSelectiveRe
        )
    }
}

/// `udCompHdr`: IQ bit width plus compression method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UdCompHeader {
    /// Bits per I or Q sample; the wire value 0 means 16 bits.
    pub iq_width: u8,
    pub method: UdCompMethod,
}

impl UdCompHeader {
    pub fn new(iq_width: u8, method: UdCompMethod) -> Self {
        UdCompHeader { iq_width, method }
    }

    pub fn serialize(&self) -> u8 {
        let width = if self.iq_width >= 16 {
            0
        } else {
            self.iq_width
        };
        ((width & 0x0F) << 4) | self.method.to_u4()
    }

    pub fn parse(byte: u8) -> Self {
        let raw_width = (byte >> 4) & 0x0F;
        UdCompHeader {
            iq_width: if raw_width == 0 { 16 } else { raw_width },
            method: UdCompMethod::from_u4(byte),
        }
    }

    /// Bytes an uncompressed PRB of 12 subcarriers occupies at this IQ width.
    pub fn prb_payload_bytes(&self) -> usize {
        // 12 subcarriers x 2 (I and Q) x iq_width bits, rounded up to whole bytes.
        (12 * 2 * self.iq_width as usize).div_ceil(8)
    }
}

/// One U-Plane data section: a contiguous run of physical resource blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UPlaneSection {
    pub section_id: u16,
    /// `rb`: false selects every PRB, true selects every other PRB.
    pub every_other_rb: bool,
    /// `symInc`: the symbol number increments for the next section.
    pub sym_inc: bool,
    pub start_prbu: u16,
    /// `numPrbu`; the wire value 0 means "all PRBs from `start_prbu`".
    pub num_prbu: u8,
    /// Present when dynamic compression is configured for the flow.
    pub ud_comp_header: Option<UdCompHeader>,
    pub iq_samples: Vec<u8>,
}

impl UPlaneSection {
    pub fn new(section_id: u16, start_prbu: u16, num_prbu: u8, iq_samples: Vec<u8>) -> Self {
        UPlaneSection {
            section_id,
            every_other_rb: false,
            sym_inc: false,
            start_prbu,
            num_prbu,
            ud_comp_header: None,
            iq_samples,
        }
    }

    pub fn with_compression(mut self, header: UdCompHeader) -> Self {
        self.ud_comp_header = Some(header);
        self
    }

    /// PRBs described by this section; 0 on the wire means every remaining PRB.
    pub fn prb_count(&self, carrier_prbs: u16) -> u16 {
        if self.num_prbu == 0 {
            carrier_prbs.saturating_sub(self.start_prbu)
        } else {
            self.num_prbu as u16
        }
    }

    fn serialize_into(&self, out: &mut Vec<u8>) -> Result<(), OranError> {
        if self.section_id > 0x0FFF {
            return Err(OranError::FieldOutOfRange {
                field: "sectionId",
                value: self.section_id as u32,
            });
        }
        if self.start_prbu > 0x03FF {
            return Err(OranError::FieldOutOfRange {
                field: "startPrbu",
                value: self.start_prbu as u32,
            });
        }
        out.push((self.section_id >> 4) as u8);
        out.push(
            (((self.section_id & 0x0F) as u8) << 4)
                | (u8::from(self.every_other_rb) << 3)
                | (u8::from(self.sym_inc) << 2)
                | ((self.start_prbu >> 8) & 0x03) as u8,
        );
        out.push((self.start_prbu & 0xFF) as u8);
        out.push(self.num_prbu);
        if let Some(comp) = self.ud_comp_header {
            out.push(comp.serialize());
            out.push(0); // reserved octet following udCompHdr
        }
        out.extend_from_slice(&self.iq_samples);
        Ok(())
    }
}

/// A complete U-Plane message: radio header plus one or more PRB sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UPlaneMessage {
    pub header: OranRadioHeader,
    pub sections: Vec<UPlaneSection>,
}

impl UPlaneMessage {
    pub fn new(header: OranRadioHeader, sections: Vec<UPlaneSection>) -> Self {
        UPlaneMessage { header, sections }
    }

    pub fn serialize(&self) -> Result<Vec<u8>, OranError> {
        self.header.validate()?;
        let mut out = Vec::with_capacity(ORAN_RADIO_HEADER_LEN + 8 * self.sections.len());
        out.extend_from_slice(&self.header.serialize());
        for section in &self.sections {
            section.serialize_into(&mut out)?;
        }
        Ok(out)
    }

    /// Decodes a U-Plane message.
    ///
    /// The section payload length is not carried on the wire, so the caller supplies
    /// `iq_bytes_per_prb` from the flow's static configuration (and `dynamic_compression`
    /// to say whether each section is prefixed by a `udCompHdr` and its reserved octet).
    pub fn parse(
        data: &[u8],
        dynamic_compression: bool,
        iq_bytes_per_prb: usize,
    ) -> Result<Self, OranError> {
        let header = OranRadioHeader::parse(data)?;
        let mut offset = ORAN_RADIO_HEADER_LEN;
        let mut sections = Vec::new();

        while offset < data.len() {
            let fixed = ORAN_UPLANE_SECTION_LEN + if dynamic_compression { 2 } else { 0 };
            if data.len() - offset < fixed {
                return Err(OranError::Truncated {
                    need: offset + fixed,
                    got: data.len(),
                });
            }
            let b0 = data[offset];
            let b1 = data[offset + 1];
            let section_id = ((b0 as u16) << 4) | ((b1 >> 4) as u16);
            let every_other_rb = b1 & 0x08 != 0;
            let sym_inc = b1 & 0x04 != 0;
            let start_prbu = (((b1 & 0x03) as u16) << 8) | data[offset + 2] as u16;
            let num_prbu = data[offset + 3];
            offset += ORAN_UPLANE_SECTION_LEN;

            let ud_comp_header = if dynamic_compression {
                let comp = UdCompHeader::parse(data[offset]);
                offset += 2; // udCompHdr + reserved
                Some(comp)
            } else {
                None
            };

            // numPrbu 0 means "all remaining PRBs", so the rest of the buffer is IQ data.
            let iq_len = if num_prbu == 0 {
                data.len() - offset
            } else {
                num_prbu as usize * iq_bytes_per_prb
            };
            if data.len() - offset < iq_len {
                return Err(OranError::Truncated {
                    need: offset + iq_len,
                    got: data.len(),
                });
            }
            let iq_samples = data[offset..offset + iq_len].to_vec();
            offset += iq_len;

            sections.push(UPlaneSection {
                section_id,
                every_other_rb,
                sym_inc,
                start_prbu,
                num_prbu,
                ud_comp_header,
                iq_samples,
            });
        }

        Ok(UPlaneMessage { header, sections })
    }
}

/// C-Plane section types (CUS-Plane Table "sectionType").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OranSectionType {
    /// 0: unused resource blocks or symbols (transmission blanking).
    UnusedResourceBlocks,
    /// 1: most downlink and uplink radio channels.
    DlUlRadioChannel,
    /// 3: PRACH and mixed-numerology channels.
    PrachMixedNumerology,
    /// 5: UE scheduling information.
    UeScheduling,
    /// 6: channel information.
    ChannelInformation,
    /// 7: LAA (licensed assisted access).
    Laa,
    Reserved(u8),
}

impl OranSectionType {
    pub fn from_u8(code: u8) -> Self {
        match code {
            0 => OranSectionType::UnusedResourceBlocks,
            1 => OranSectionType::DlUlRadioChannel,
            3 => OranSectionType::PrachMixedNumerology,
            5 => OranSectionType::UeScheduling,
            6 => OranSectionType::ChannelInformation,
            7 => OranSectionType::Laa,
            other => OranSectionType::Reserved(other),
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            OranSectionType::UnusedResourceBlocks => 0,
            OranSectionType::DlUlRadioChannel => 1,
            OranSectionType::PrachMixedNumerology => 3,
            OranSectionType::UeScheduling => 5,
            OranSectionType::ChannelInformation => 6,
            OranSectionType::Laa => 7,
            OranSectionType::Reserved(c) => c,
        }
    }
}

/// One C-Plane Section Type 1 section describing a scheduled PRB allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CPlaneSection {
    pub section_id: u16,
    pub every_other_rb: bool,
    pub sym_inc: bool,
    pub start_prbc: u16,
    /// `numPrbc`; the wire value 0 means "all PRBs of the carrier".
    pub num_prbc: u8,
    /// 12-bit resource element bitmask inside each PRB.
    pub re_mask: u16,
    /// Number of consecutive symbols this allocation covers (4 bits).
    pub num_symbol: u8,
    /// `ef`: section extensions follow this section.
    pub extension_flag: bool,
    /// 15-bit beam identifier selecting the beamforming weight set.
    pub beam_id: u16,
}

impl CPlaneSection {
    pub fn new(
        section_id: u16,
        start_prbc: u16,
        num_prbc: u8,
        num_symbol: u8,
        beam_id: u16,
    ) -> Self {
        CPlaneSection {
            section_id,
            every_other_rb: false,
            sym_inc: false,
            start_prbc,
            num_prbc,
            re_mask: 0x0FFF,
            num_symbol,
            extension_flag: false,
            beam_id,
        }
    }

    pub fn serialize(&self) -> Result<[u8; ORAN_CPLANE_SECTION_LEN], OranError> {
        if self.section_id > 0x0FFF {
            return Err(OranError::FieldOutOfRange {
                field: "sectionId",
                value: self.section_id as u32,
            });
        }
        if self.start_prbc > 0x03FF {
            return Err(OranError::FieldOutOfRange {
                field: "startPrbc",
                value: self.start_prbc as u32,
            });
        }
        if self.re_mask > 0x0FFF {
            return Err(OranError::FieldOutOfRange {
                field: "reMask",
                value: self.re_mask as u32,
            });
        }
        if self.num_symbol > 0x0F {
            return Err(OranError::FieldOutOfRange {
                field: "numSymbol",
                value: self.num_symbol as u32,
            });
        }
        if self.beam_id > 0x7FFF {
            return Err(OranError::FieldOutOfRange {
                field: "beamId",
                value: self.beam_id as u32,
            });
        }

        let mut out = [0u8; ORAN_CPLANE_SECTION_LEN];
        out[0] = (self.section_id >> 4) as u8;
        out[1] = (((self.section_id & 0x0F) as u8) << 4)
            | (u8::from(self.every_other_rb) << 3)
            | (u8::from(self.sym_inc) << 2)
            | ((self.start_prbc >> 8) & 0x03) as u8;
        out[2] = (self.start_prbc & 0xFF) as u8;
        out[3] = self.num_prbc;
        // reMask occupies 12 bits, numSymbol the low nibble of the next octet.
        out[4] = (self.re_mask >> 4) as u8;
        out[5] = (((self.re_mask & 0x0F) as u8) << 4) | (self.num_symbol & 0x0F);
        out[6] = (u8::from(self.extension_flag) << 7) | ((self.beam_id >> 8) & 0x7F) as u8;
        out[7] = (self.beam_id & 0xFF) as u8;
        Ok(out)
    }

    pub fn parse(data: &[u8]) -> Result<Self, OranError> {
        if data.len() < ORAN_CPLANE_SECTION_LEN {
            return Err(OranError::Truncated {
                need: ORAN_CPLANE_SECTION_LEN,
                got: data.len(),
            });
        }
        let section_id = ((data[0] as u16) << 4) | ((data[1] >> 4) as u16);
        let section = CPlaneSection {
            section_id,
            every_other_rb: data[1] & 0x08 != 0,
            sym_inc: data[1] & 0x04 != 0,
            start_prbc: (((data[1] & 0x03) as u16) << 8) | data[2] as u16,
            num_prbc: data[3],
            re_mask: ((data[4] as u16) << 4) | ((data[5] >> 4) as u16),
            num_symbol: data[5] & 0x0F,
            extension_flag: data[6] & 0x80 != 0,
            beam_id: (((data[6] & 0x7F) as u16) << 8) | data[7] as u16,
        };
        if section.extension_flag {
            return Err(OranError::SectionExtensionUnsupported(section_id));
        }
        Ok(section)
    }
}

/// A C-Plane Section Type 1 message scheduling one or more PRB allocations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CPlaneMessage {
    /// `symbol_id` of this header carries `startSymbolId`.
    pub header: OranRadioHeader,
    pub section_type: OranSectionType,
    pub ud_comp_header: UdCompHeader,
    pub sections: Vec<CPlaneSection>,
}

impl CPlaneMessage {
    pub fn new(
        header: OranRadioHeader,
        ud_comp_header: UdCompHeader,
        sections: Vec<CPlaneSection>,
    ) -> Self {
        CPlaneMessage {
            header,
            section_type: OranSectionType::DlUlRadioChannel,
            ud_comp_header,
            sections,
        }
    }

    pub fn serialize(&self) -> Result<Vec<u8>, OranError> {
        if self.section_type != OranSectionType::DlUlRadioChannel {
            return Err(OranError::UnsupportedSectionType(self.section_type.to_u8()));
        }
        self.header.validate()?;
        if self.sections.len() > u8::MAX as usize {
            return Err(OranError::FieldOutOfRange {
                field: "numberOfSections",
                value: self.sections.len() as u32,
            });
        }
        let mut out = Vec::with_capacity(
            ORAN_CPLANE_HEADER_LEN + ORAN_CPLANE_SECTION_LEN * self.sections.len(),
        );
        out.extend_from_slice(&self.header.serialize());
        out.push(self.sections.len() as u8);
        out.push(self.section_type.to_u8());
        out.push(self.ud_comp_header.serialize());
        out.push(0); // reserved
        for section in &self.sections {
            out.extend_from_slice(&section.serialize()?);
        }
        Ok(out)
    }

    /// Decodes a Section Type 1 C-Plane message.
    ///
    /// Other section types extend the common header with type-specific fields
    /// (`timeOffset`, `frameStructure`, `cpLength`) and are rejected here.
    pub fn parse(data: &[u8]) -> Result<Self, OranError> {
        if data.len() < ORAN_CPLANE_HEADER_LEN {
            return Err(OranError::Truncated {
                need: ORAN_CPLANE_HEADER_LEN,
                got: data.len(),
            });
        }
        let header = OranRadioHeader::parse(data)?;
        let declared = data[4];
        let section_type = OranSectionType::from_u8(data[5]);
        if section_type != OranSectionType::DlUlRadioChannel {
            return Err(OranError::UnsupportedSectionType(data[5]));
        }
        let ud_comp_header = UdCompHeader::parse(data[6]);

        let body = &data[ORAN_CPLANE_HEADER_LEN..];
        if !body.len().is_multiple_of(ORAN_CPLANE_SECTION_LEN) || body.is_empty() {
            return Err(OranError::Truncated {
                need: ORAN_CPLANE_HEADER_LEN + ORAN_CPLANE_SECTION_LEN,
                got: data.len(),
            });
        }
        let mut sections = Vec::with_capacity(body.len() / ORAN_CPLANE_SECTION_LEN);
        for chunk in body.chunks(ORAN_CPLANE_SECTION_LEN) {
            sections.push(CPlaneSection::parse(chunk)?);
        }
        if sections.len() != declared as usize {
            return Err(OranError::SectionCountMismatch {
                declared,
                parsed: sections.len(),
            });
        }

        Ok(CPlaneMessage {
            header,
            section_type,
            ud_comp_header,
            sections,
        })
    }

    /// Total PRBs scheduled across every section of this message.
    pub fn scheduled_prbs(&self, carrier_prbs: u16) -> u32 {
        self.sections
            .iter()
            .map(|s| {
                let prbs = if s.num_prbc == 0 {
                    carrier_prbs.saturating_sub(s.start_prbc) as u32
                } else {
                    s.num_prbc as u32
                };
                prbs * s.num_symbol.max(1) as u32
            })
            .sum()
    }
}

/// Per-eAxC counters produced by [`OranFlowMonitor`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OranFlowStats {
    pub u_plane_messages: u64,
    pub c_plane_messages: u64,
    pub prb_count: u64,
    /// U-Plane symbols that arrived after a later symbol had already been seen.
    pub out_of_order_symbols: u64,
    /// U-Plane symbols with no preceding C-Plane section scheduling them.
    pub unscheduled_symbols: u64,
    pub last_symbol_index: Option<u32>,
}

/// Tracks C-Plane and U-Plane activity per antenna-carrier flow.
///
/// An O-RU expects the C-Plane section that schedules a symbol to arrive before the
/// U-Plane data for it; this monitor flags both ordering violations and unscheduled data.
#[derive(Debug, Clone)]
pub struct OranFlowMonitor {
    pub format: EaxcIdFormat,
    /// 3GPP numerology exponent used to linearize (frame, subframe, slot, symbol).
    pub numerology: u8,
    pub carrier_prbs: u16,
    flows: HashMap<u16, OranFlowStats>,
    scheduled: HashMap<u16, Vec<u32>>,
}

impl OranFlowMonitor {
    pub fn new(format: EaxcIdFormat, numerology: u8, carrier_prbs: u16) -> Self {
        OranFlowMonitor {
            format,
            numerology,
            carrier_prbs,
            flows: HashMap::new(),
            scheduled: HashMap::new(),
        }
    }

    /// Records a C-Plane message and the symbols it schedules for the flow.
    pub fn observe_c_plane(&mut self, eaxc_raw: u16, message: &CPlaneMessage) {
        let start = message.header.symbol_index(self.numerology);
        let span = message
            .sections
            .iter()
            .map(|s| s.num_symbol.max(1) as u32)
            .max()
            .unwrap_or(1);
        let scheduled = self.scheduled.entry(eaxc_raw).or_default();
        for offset in 0..span {
            scheduled.push(start + offset);
        }
        let stats = self.flows.entry(eaxc_raw).or_default();
        stats.c_plane_messages += 1;
    }

    /// Records a U-Plane message, flagging late symbols and unscheduled data.
    pub fn observe_u_plane(&mut self, eaxc_raw: u16, message: &UPlaneMessage) {
        let index = message.header.symbol_index(self.numerology);
        let prbs: u64 = message
            .sections
            .iter()
            .map(|s| s.prb_count(self.carrier_prbs) as u64)
            .sum();
        let scheduled = self
            .scheduled
            .get(&eaxc_raw)
            .is_some_and(|symbols| symbols.contains(&index));

        let stats = self.flows.entry(eaxc_raw).or_default();
        stats.u_plane_messages += 1;
        stats.prb_count += prbs;
        if let Some(last) = stats.last_symbol_index
            && index < last
        {
            stats.out_of_order_symbols += 1;
        }
        if !scheduled {
            stats.unscheduled_symbols += 1;
        }
        stats.last_symbol_index = Some(stats.last_symbol_index.map_or(index, |l| l.max(index)));
    }

    pub fn stats(&self, eaxc_raw: u16) -> Option<&OranFlowStats> {
        self.flows.get(&eaxc_raw)
    }

    /// Decodes the eAxC identifier of a tracked flow.
    pub fn decode_eaxc(&self, eaxc_raw: u16) -> EaxcId {
        EaxcId::unpack(eaxc_raw, self.format)
    }

    pub fn flow_count(&self) -> usize {
        self.flows.len()
    }
}
