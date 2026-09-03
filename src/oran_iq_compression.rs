//! O-RAN fronthaul IQ sample compression (O-RAN.WG4.CUS-Plane, user data compression annex).
//!
//! The `udCompHdr` of [`crate::oran_fh_cus`] announces how many bits an I or Q sample
//! occupies and which compression method produced them; this module is the codec behind
//! that header. It implements the MSB-first bit packer that O-RAN uses for non-byte-aligned
//! sample widths, block floating point with its per-PRB exponent, selective RE sending
//! driven by the C-Plane `reMask`, mu-law companding, and the error metrics needed to say
//! what a given IQ width actually costs in signal quality.

use std::fmt;

/// Subcarriers in one physical resource block.
pub const SUBCARRIERS_PER_PRB: usize = 12;
/// I and Q values in one PRB.
pub const IQ_VALUES_PER_PRB: usize = SUBCARRIERS_PER_PRB * 2;
/// Bytes an uncompressed 16-bit PRB occupies.
pub const UNCOMPRESSED_PRB_BYTES: usize = IQ_VALUES_PER_PRB * 2;
/// Narrowest IQ width whose block floating point exponent still fits `udCompParam`.
pub const MIN_BFP_IQ_WIDTH: u8 = 4;
/// Widest IQ width the fronthaul carries.
pub const MAX_IQ_WIDTH: u8 = 16;

/// Errors raised by the fronthaul compression codecs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompressionError {
    /// The IQ width is outside the range this codec supports.
    UnsupportedIqWidth(u8),
    /// Samples must be supplied as whole PRBs of 12 subcarriers.
    NotPrbAligned(usize),
    /// The compressed buffer is shorter than the PRBs it claims to hold.
    Truncated { need: usize, got: usize },
    /// A value does not fit the two's complement range of the packing width.
    ValueOutOfRange { value: i32, width: u8 },
    /// A `reMask` selecting no resource element carries no samples at all.
    EmptyResourceMask,
}

impl fmt::Display for CompressionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompressionError::UnsupportedIqWidth(w) => {
                write!(f, "Unsupported IQ sample width of {} bits", w)
            }
            CompressionError::NotPrbAligned(n) => write!(
                f,
                "{} IQ samples do not form whole PRBs of {} subcarriers",
                n, SUBCARRIERS_PER_PRB
            ),
            CompressionError::Truncated { need, got } => write!(
                f,
                "Compressed IQ data truncated: need {} bytes, got {}",
                need, got
            ),
            CompressionError::ValueOutOfRange { value, width } => {
                write!(f, "Value {} does not fit {} signed bits", value, width)
            }
            CompressionError::EmptyResourceMask => {
                write!(f, "reMask selects no resource elements")
            }
        }
    }
}

impl std::error::Error for CompressionError {}

/// One complex subcarrier sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IqSample {
    pub i: i16,
    pub q: i16,
}

impl IqSample {
    pub fn new(i: i16, q: i16) -> Self {
        IqSample { i, q }
    }

    /// Squared magnitude, used by the signal-to-noise metrics.
    pub fn power(&self) -> i64 {
        (self.i as i64) * (self.i as i64) + (self.q as i64) * (self.q as i64)
    }
}

/// Inclusive two's complement range of a signed field of `width` bits.
fn signed_range(width: u8) -> (i32, i32) {
    if width >= 32 {
        return (i32::MIN, i32::MAX);
    }
    let half = 1i32 << (width - 1);
    (-half, half - 1)
}

/// Restores a mantissa to sample scale, saturating rather than wrapping.
///
/// A well formed PRB never overflows, but a decoder must not wrap on crafted input.
fn scale_to_sample(mantissa: i32, exponent: u8) -> i16 {
    let scaled = (mantissa as i64) << exponent;
    scaled.clamp(i16::MIN as i64, i16::MAX as i64) as i16
}

/// Packs signed values into an MSB-first bit stream of `width` bits each.
///
/// O-RAN packs IQ samples continuously, so a 9-bit sample stream crosses octet
/// boundaries; the final octet is zero padded.
pub fn pack_signed(values: &[i32], width: u8) -> Result<Vec<u8>, CompressionError> {
    if width == 0 || width > MAX_IQ_WIDTH {
        return Err(CompressionError::UnsupportedIqWidth(width));
    }
    let (min, max) = signed_range(width);
    let mut out = vec![0u8; (values.len() * width as usize).div_ceil(8)];
    let mut bit_pos = 0usize;
    for &value in values {
        if value < min || value > max {
            return Err(CompressionError::ValueOutOfRange { value, width });
        }
        let masked = (value as u32) & ((1u32 << width) - 1);
        for bit in (0..width).rev() {
            if masked >> bit & 1 == 1 {
                out[bit_pos / 8] |= 0x80 >> (bit_pos % 8);
            }
            bit_pos += 1;
        }
    }
    Ok(out)
}

/// Reads `count` sign-extended values of `width` bits from an MSB-first bit stream.
pub fn unpack_signed(data: &[u8], width: u8, count: usize) -> Result<Vec<i32>, CompressionError> {
    if width == 0 || width > MAX_IQ_WIDTH {
        return Err(CompressionError::UnsupportedIqWidth(width));
    }
    let need = (count * width as usize).div_ceil(8);
    if data.len() < need {
        return Err(CompressionError::Truncated {
            need,
            got: data.len(),
        });
    }
    let mut out = Vec::with_capacity(count);
    let mut bit_pos = 0usize;
    for _ in 0..count {
        let mut raw = 0u32;
        for _ in 0..width {
            let bit = (data[bit_pos / 8] >> (7 - (bit_pos % 8))) & 1;
            raw = (raw << 1) | bit as u32;
            bit_pos += 1;
        }
        // Sign-extend from the packing width into i32.
        let sign_bit = 1u32 << (width - 1);
        let value = if raw & sign_bit != 0 {
            (raw | !((1u32 << width) - 1)) as i32
        } else {
            raw as i32
        };
        out.push(value);
    }
    Ok(out)
}

/// Block floating point codec: one shared exponent per PRB, mantissas at `iq_width` bits.
///
/// The exponent travels in the low nibble of the per-PRB `udCompParam` octet, which is
/// why widths below [`MIN_BFP_IQ_WIDTH`] are rejected: a narrower mantissa would need an
/// exponent larger than the 4 bits available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BfpCodec {
    pub iq_width: u8,
}

impl BfpCodec {
    pub fn new(iq_width: u8) -> Result<Self, CompressionError> {
        if !(MIN_BFP_IQ_WIDTH..=MAX_IQ_WIDTH).contains(&iq_width) {
            return Err(CompressionError::UnsupportedIqWidth(iq_width));
        }
        Ok(BfpCodec { iq_width })
    }

    /// Bytes one compressed PRB occupies: the `udCompParam` octet plus packed mantissas.
    pub fn compressed_prb_bytes(&self) -> usize {
        1 + (IQ_VALUES_PER_PRB * self.iq_width as usize).div_ceil(8)
    }

    /// Shared exponent for one PRB: the smallest shift that fits every mantissa.
    pub fn block_exponent(&self, prb: &[IqSample]) -> u8 {
        let (_, max_mantissa) = signed_range(self.iq_width);
        let peak = prb
            .iter()
            .map(|s| (s.i as i32).unsigned_abs().max((s.q as i32).unsigned_abs()))
            .max()
            .unwrap_or(0) as i32;
        let mut exponent = 0u8;
        while (peak >> exponent) > max_mantissa {
            exponent += 1;
        }
        exponent
    }

    /// Compresses whole PRBs; every PRB is prefixed by its own exponent octet.
    pub fn compress(&self, samples: &[IqSample]) -> Result<Vec<u8>, CompressionError> {
        if samples.is_empty() || !samples.len().is_multiple_of(SUBCARRIERS_PER_PRB) {
            return Err(CompressionError::NotPrbAligned(samples.len()));
        }
        let mut out =
            Vec::with_capacity(samples.len() / SUBCARRIERS_PER_PRB * self.compressed_prb_bytes());
        for prb in samples.chunks(SUBCARRIERS_PER_PRB) {
            let exponent = self.block_exponent(prb);
            // udCompParam: the exponent occupies the low nibble, the high nibble is reserved.
            out.push(exponent & 0x0F);
            let mantissas: Vec<i32> = prb
                .iter()
                .flat_map(|s| [(s.i as i32) >> exponent, (s.q as i32) >> exponent])
                .collect();
            out.extend_from_slice(&pack_signed(&mantissas, self.iq_width)?);
        }
        Ok(out)
    }

    /// Restores `prb_count` PRBs of samples, scaling each mantissa back by its exponent.
    pub fn decompress(
        &self,
        data: &[u8],
        prb_count: usize,
    ) -> Result<Vec<IqSample>, CompressionError> {
        let need = prb_count * self.compressed_prb_bytes();
        if data.len() < need {
            return Err(CompressionError::Truncated {
                need,
                got: data.len(),
            });
        }
        let mut out = Vec::with_capacity(prb_count * SUBCARRIERS_PER_PRB);
        for prb in data[..need].chunks(self.compressed_prb_bytes()) {
            let exponent = prb[0] & 0x0F;
            let mantissas = unpack_signed(&prb[1..], self.iq_width, IQ_VALUES_PER_PRB)?;
            for pair in mantissas.chunks(2) {
                out.push(IqSample::new(
                    scale_to_sample(pair[0], exponent),
                    scale_to_sample(pair[1], exponent),
                ));
            }
        }
        Ok(out)
    }

    /// Compressed size relative to 16-bit uncompressed samples, smaller is better.
    pub fn compression_ratio(&self) -> f64 {
        self.compressed_prb_bytes() as f64 / UNCOMPRESSED_PRB_BYTES as f64
    }
}

/// Block floating point with selective RE sending (`udCompMeth` 0x5).
///
/// Only the resource elements whose bit is set in the C-Plane `reMask` are carried, so an
/// allocation using a fraction of the subcarriers costs a fraction of the fronthaul bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectiveReCodec {
    pub bfp: BfpCodec,
    /// 12-bit mask, bit 11 selects subcarrier 0.
    pub re_mask: u16,
}

impl SelectiveReCodec {
    pub fn new(iq_width: u8, re_mask: u16) -> Result<Self, CompressionError> {
        if re_mask & 0x0FFF == 0 {
            return Err(CompressionError::EmptyResourceMask);
        }
        Ok(SelectiveReCodec {
            bfp: BfpCodec::new(iq_width)?,
            re_mask: re_mask & 0x0FFF,
        })
    }

    /// Subcarriers per PRB the mask actually selects.
    pub fn selected_res(&self) -> usize {
        (self.re_mask & 0x0FFF).count_ones() as usize
    }

    pub fn compressed_prb_bytes(&self) -> usize {
        1 + (self.selected_res() * 2 * self.bfp.iq_width as usize).div_ceil(8)
    }

    fn is_selected(&self, subcarrier: usize) -> bool {
        self.re_mask >> (SUBCARRIERS_PER_PRB - 1 - subcarrier) & 1 == 1
    }

    /// Compresses only the masked resource elements of each PRB.
    pub fn compress(&self, samples: &[IqSample]) -> Result<Vec<u8>, CompressionError> {
        if samples.is_empty() || !samples.len().is_multiple_of(SUBCARRIERS_PER_PRB) {
            return Err(CompressionError::NotPrbAligned(samples.len()));
        }
        let mut out = Vec::new();
        for prb in samples.chunks(SUBCARRIERS_PER_PRB) {
            let selected: Vec<IqSample> = prb
                .iter()
                .enumerate()
                .filter(|(idx, _)| self.is_selected(*idx))
                .map(|(_, s)| *s)
                .collect();
            let exponent = self.bfp.block_exponent(&selected);
            out.push(exponent & 0x0F);
            let mantissas: Vec<i32> = selected
                .iter()
                .flat_map(|s| [(s.i as i32) >> exponent, (s.q as i32) >> exponent])
                .collect();
            out.extend_from_slice(&pack_signed(&mantissas, self.bfp.iq_width)?);
        }
        Ok(out)
    }

    /// Restores PRBs, leaving unselected resource elements at zero.
    pub fn decompress(
        &self,
        data: &[u8],
        prb_count: usize,
    ) -> Result<Vec<IqSample>, CompressionError> {
        let need = prb_count * self.compressed_prb_bytes();
        if data.len() < need {
            return Err(CompressionError::Truncated {
                need,
                got: data.len(),
            });
        }
        let selected = self.selected_res();
        let mut out = Vec::with_capacity(prb_count * SUBCARRIERS_PER_PRB);
        for prb in data[..need].chunks(self.compressed_prb_bytes()) {
            let exponent = prb[0] & 0x0F;
            let mantissas = unpack_signed(&prb[1..], self.bfp.iq_width, selected * 2)?;
            let mut carried = mantissas.chunks(2);
            for subcarrier in 0..SUBCARRIERS_PER_PRB {
                if self.is_selected(subcarrier) {
                    let pair = carried.next().unwrap_or(&[0, 0]);
                    out.push(IqSample::new(
                        scale_to_sample(pair[0], exponent),
                        scale_to_sample(pair[1], exponent),
                    ));
                } else {
                    out.push(IqSample::default());
                }
            }
        }
        Ok(out)
    }
}

/// Bias added by the G.711 mu-law companding curve before segment extraction.
const MU_LAW_BIAS: i32 = 0x84;
/// Largest magnitude the 14-bit mu-law curve represents.
const MU_LAW_CLIP: i32 = 32635;

/// Compresses one sample with the G.711 mu-law curve (`udCompMeth` 0x3).
///
/// The companding curve is logarithmic, so it keeps small samples accurate and lets the
/// error grow with amplitude; one 16-bit sample becomes one octet.
pub fn mu_law_compress(sample: i16) -> u8 {
    let mut value = sample as i32;
    let sign = if value < 0 {
        value = -value;
        0x80u8
    } else {
        0x00
    };
    if value > MU_LAW_CLIP {
        value = MU_LAW_CLIP;
    }
    value += MU_LAW_BIAS;

    // The segment is the position of the highest set bit above the bias.
    let mut segment = 0u8;
    let mut probe = value >> 8;
    while probe != 0 && segment < 7 {
        segment += 1;
        probe >>= 1;
    }
    let mantissa = ((value >> (segment as i32 + 3)) & 0x0F) as u8;
    !(sign | (segment << 4) | mantissa)
}

/// Expands one mu-law octet back to a 16-bit sample.
pub fn mu_law_expand(byte: u8) -> i16 {
    let inverted = !byte;
    let sign = inverted & 0x80;
    let segment = (inverted >> 4) & 0x07;
    let mantissa = inverted & 0x0F;
    let magnitude = (((mantissa as i32) << 3) + MU_LAW_BIAS) << segment;
    let value = magnitude - MU_LAW_BIAS;
    if sign != 0 {
        -value as i16
    } else {
        value as i16
    }
}

/// Mu-law codec over a stream of IQ samples: two octets per subcarrier, no exponent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MuLawCodec;

impl MuLawCodec {
    pub fn compress(&self, samples: &[IqSample]) -> Vec<u8> {
        let mut out = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            out.push(mu_law_compress(sample.i));
            out.push(mu_law_compress(sample.q));
        }
        out
    }

    pub fn decompress(&self, data: &[u8]) -> Vec<IqSample> {
        data.chunks_exact(2)
            .map(|pair| IqSample::new(mu_law_expand(pair[0]), mu_law_expand(pair[1])))
            .collect()
    }
}

/// Quality of a compress / decompress round trip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompressionQuality {
    pub sample_count: usize,
    /// Largest absolute difference on any I or Q value.
    pub max_absolute_error: i32,
    /// Ratio of signal power to error power, in decibels.
    pub snr_db: f64,
    /// Error-vector magnitude as a percentage, the figure 3GPP transmitter limits use.
    pub evm_percent: f64,
}

/// Compares recovered samples against the originals.
///
/// Returns `None` when the inputs have different lengths, since no meaningful
/// per-sample comparison exists then.
pub fn measure_quality(
    original: &[IqSample],
    recovered: &[IqSample],
) -> Option<CompressionQuality> {
    if original.len() != recovered.len() || original.is_empty() {
        return None;
    }
    let mut max_absolute_error = 0i32;
    let mut signal_power = 0f64;
    let mut error_power = 0f64;
    for (a, b) in original.iter().zip(recovered.iter()) {
        let di = a.i as i32 - b.i as i32;
        let dq = a.q as i32 - b.q as i32;
        max_absolute_error = max_absolute_error.max(di.abs()).max(dq.abs());
        signal_power += a.power() as f64;
        error_power += (di as f64) * (di as f64) + (dq as f64) * (dq as f64);
    }
    let snr_db = if error_power == 0.0 {
        f64::INFINITY
    } else {
        10.0 * (signal_power / error_power).log10()
    };
    let evm_percent = if signal_power == 0.0 {
        0.0
    } else {
        (error_power / signal_power).sqrt() * 100.0
    };
    Some(CompressionQuality {
        sample_count: original.len(),
        max_absolute_error,
        snr_db,
        evm_percent,
    })
}

/// Fronthaul bit rate a compressed carrier needs, in bits per second.
///
/// `prbs` is the carrier bandwidth in resource blocks, `symbols_per_second` follows from
/// the numerology (14 symbols per slot, `2^mu` slots per millisecond).
pub fn fronthaul_bitrate_bps(
    prbs: u32,
    symbols_per_second: u32,
    layers: u32,
    compressed_prb_bytes: usize,
) -> u64 {
    prbs as u64 * symbols_per_second as u64 * layers as u64 * compressed_prb_bytes as u64 * 8
}
