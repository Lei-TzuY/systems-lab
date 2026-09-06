//! 3GPP Rel-17 5G NR Uplink Data Compression (UDC) in PDCP Engine
//!
//! Conforms to:
//! - 3GPP TS 38.323 §5.14: Uplink Data Compression
//! - 3GPP TS 38.323 §6.2.3: UDC Feedback Control PDU
//! - 3GPP TS 38.323 §6.3.8: UDC Data PDU Header
//! - 3GPP TS 38.331: `Udc-Config-r17` RRC Information Element
//!
//! Pure standard Rust (`std`/`core` only), zero external dependencies.

/// Maximum sliding dictionary buffer sizes allowed by 3GPP TS 38.331.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdcBufferSize {
    /// 2048 bytes (2 KB)
    Buf2048 = 2048,
    /// 4096 bytes (4 KB)
    Buf4096 = 4096,
    /// 8192 bytes (8 KB)
    Buf8192 = 8192,
}

/// UDC Data PDU Header (TS 38.323 §6.3.8).
///
/// 1-octet header preceding the PDCP SDU:
/// - Bit 7: `FU` (Field Usage) — 0: Uncompressed, 1: Compressed
/// - Bit 6: `FR` (Field Reset) — 1: Buffer reset was performed prior to compression
/// - Bits 5..2: `Checksum` (4 bits) — CRC-4 computed over the uncompressed data
/// - Bits 1..0: Reserved (00)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UdcHeader {
    /// Field Usage: true if payload is compressed.
    pub fu: bool,
    /// Field Reset: true if dictionary was reset before processing.
    pub fr: bool,
    /// 4-bit Checksum over the uncompressed payload.
    pub checksum: u8,
}

impl UdcHeader {
    /// Create a new UDC header.
    pub fn new(fu: bool, fr: bool, checksum: u8) -> Self {
        Self {
            fu,
            fr,
            checksum: checksum & 0x0F,
        }
    }

    /// Serialize into 1-byte wire octet.
    pub fn serialize(&self) -> u8 {
        let fu_bit = if self.fu { 0x80 } else { 0x00 };
        let fr_bit = if self.fr { 0x40 } else { 0x00 };
        let cs_bits = (self.checksum & 0x0F) << 2;
        fu_bit | fr_bit | cs_bits
    }

    /// Parse from 1-byte wire octet.
    pub fn parse(octet: u8) -> Self {
        let fu = (octet & 0x80) != 0;
        let fr = (octet & 0x40) != 0;
        let checksum = (octet >> 2) & 0x0F;
        Self { fu, fr, checksum }
    }
}

/// Compute 4-bit CRC Checksum over uncompressed data buffer per TS 38.323 §5.14.2.
/// Generator polynomial: G(x) = x^4 + x + 1 (0x13).
pub fn compute_udc_crc4(data: &[u8]) -> u8 {
    let mut crc: u8 = 0x00;
    for &b in data {
        crc ^= b;
        for _ in 0..8 {
            if (crc & 0x80) != 0 {
                crc = (crc << 1) ^ 0x30;
            } else {
                crc <<= 1;
            }
        }
    }
    (crc >> 4) & 0x0F
}

/// UDC Feedback Control PDU (TS 38.323 §6.2.3).
///
/// Sent by the receiving PDCP entity (gNB) to request a dictionary buffer reset
/// when a checksum validation error indicates desynchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UdcFeedbackPdu {
    /// Feedback Error (FE) flag: true indicates decompression failure / reset requested.
    pub fe: bool,
}

impl UdcFeedbackPdu {
    /// Serialize into 2-octet PDCP Control PDU.
    /// Byte 0: [D/C = 0 (1b)][PDU Type = 010 (3b)][Reserved = 0000 (4b)] = 0x20
    /// Byte 1: [FE (1b)][Reserved = 0000000 (7b)]
    pub fn serialize(&self) -> [u8; 2] {
        let b0 = 0x20; // D/C=0, PDU Type=010
        let b1 = if self.fe { 0x80 } else { 0x00 };
        [b0, b1]
    }

    /// Parse from 2-octet PDCP Control PDU bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 2 {
            return Err("UDC Feedback PDU buffer too short");
        }
        if (bytes[0] >> 4) != 0x02 {
            return Err("Invalid PDCP Control PDU Type (expected 010 for UDC Feedback)");
        }
        let fe = (bytes[1] & 0x80) != 0;
        Ok(Self { fe })
    }
}

/// Configuration parameters for UDC (TS 38.331 `Udc-Config-r17`).
#[derive(Debug, Clone, PartialEq)]
pub struct UdcConfig {
    /// Sliding window buffer capacity in bytes.
    pub buffer_size: UdcBufferSize,
    /// Minimum SDU length in bytes to attempt compression (e.g. 16 bytes).
    pub min_compression_len: usize,
    /// Optional static pre-defined dictionary.
    pub predefined_dictionary: Option<Vec<u8>>,
}

impl Default for UdcConfig {
    fn default() -> Self {
        Self {
            buffer_size: UdcBufferSize::Buf4096,
            min_compression_len: 16,
            predefined_dictionary: None,
        }
    }
}

/// Sliding dictionary buffer maintaining the past uncompressed byte history.
#[derive(Debug, Clone)]
pub struct SlidingDictionary {
    buffer: Vec<u8>,
    capacity: usize,
}

impl SlidingDictionary {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            capacity,
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn append(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
        if self.buffer.len() > self.capacity {
            let overflow = self.buffer.len() - self.capacity;
            self.buffer.drain(0..overflow);
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buffer
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

/// UDC Compressor operating on the UE transmission side.
#[derive(Debug, Clone)]
pub struct UdcCompressor {
    config: UdcConfig,
    dictionary: SlidingDictionary,
    needs_reset: bool,
    total_raw_bytes: usize,
    total_compressed_bytes: usize,
}

impl UdcCompressor {
    pub fn new(config: UdcConfig) -> Self {
        let mut dict = SlidingDictionary::new(config.buffer_size as usize);
        if let Some(ref predef) = config.predefined_dictionary {
            dict.append(predef);
        }

        Self {
            config,
            dictionary: dict,
            needs_reset: false,
            total_raw_bytes: 0,
            total_compressed_bytes: 0,
        }
    }

    /// Trigger a dictionary reset upon receiving a UDC Feedback PDU.
    pub fn trigger_reset(&mut self) {
        self.needs_reset = true;
    }

    /// Compress a PDCP SDU.
    /// Returns the complete UDC PDU: `[UDC Header (1B)][Compressed or Uncompressed SDU]`.
    pub fn compress_sdu(&mut self, sdu: &[u8]) -> Vec<u8> {
        let checksum = compute_udc_crc4(sdu);
        let was_reset = self.needs_reset;

        if was_reset {
            self.dictionary.clear();
            if let Some(ref predef) = self.config.predefined_dictionary {
                self.dictionary.append(predef);
            }
            self.needs_reset = false;
        }

        self.total_raw_bytes += sdu.len();

        // Attempt compression if size >= threshold
        if sdu.len() >= self.config.min_compression_len {
            let compressed_payload = self.lz77_compress(sdu);

            // Only use compression if it actually yields smaller payload
            if compressed_payload.len() < sdu.len() {
                let header = UdcHeader::new(true, was_reset, checksum);
                let mut pdu = Vec::with_capacity(1 + compressed_payload.len());
                pdu.push(header.serialize());
                pdu.extend_from_slice(&compressed_payload);

                self.total_compressed_bytes += pdu.len();
                self.dictionary.append(sdu);
                return pdu;
            }
        }

        // Send uncompressed with FU=0
        let header = UdcHeader::new(false, was_reset, checksum);
        let mut pdu = Vec::with_capacity(1 + sdu.len());
        pdu.push(header.serialize());
        pdu.extend_from_slice(sdu);

        self.total_compressed_bytes += pdu.len();
        self.dictionary.append(sdu);
        pdu
    }

    /// Perform sliding window LZ77 compression with robust tag-based framing.
    fn lz77_compress(&self, sdu: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let dict = self.dictionary.as_slice();
        let dict_len = dict.len();

        let mut i = 0;
        let mut literal_start = 0;

        while i < sdu.len() {
            let mut best_len = 0;
            let mut best_dist = 0;

            if dict_len >= 4 && sdu.len() - i >= 4 {
                let max_possible_match = (sdu.len() - i).min(130);
                let target_prefix = &sdu[i..i + 4];

                for pos in (0..dict_len.saturating_sub(3)).rev() {
                    if &dict[pos..pos + 4] == target_prefix {
                        let mut match_len = 4;
                        while match_len < max_possible_match
                            && pos + match_len < dict_len
                            && dict[pos + match_len] == sdu[i + match_len]
                        {
                            match_len += 1;
                        }
                        if match_len > best_len {
                            best_len = match_len;
                            best_dist = dict_len - pos;
                            if best_len >= 130 {
                                break;
                            }
                        }
                    }
                }
            }

            if best_len >= 4 && best_dist <= 65535 {
                // First flush any pending literal run before this match
                if i > literal_start {
                    let mut lit_pos = literal_start;
                    while lit_pos < i {
                        let chunk_len = (i - lit_pos).min(128);
                        out.push((chunk_len - 1) as u8); // Tag bit 7 is 0
                        out.extend_from_slice(&sdu[lit_pos..lit_pos + chunk_len]);
                        lit_pos += chunk_len;
                    }
                }

                // Emit match token: Tag bit 7 is 1, lower 7 bits = (match_len - 3)
                let tag = 0x80 | ((best_len - 3) as u8);
                out.push(tag);
                out.push((best_dist >> 8) as u8);
                out.push((best_dist & 0xFF) as u8);

                i += best_len;
                literal_start = i;
            } else {
                i += 1;
            }
        }

        // Flush any trailing literals
        if i > literal_start {
            let mut lit_pos = literal_start;
            while lit_pos < i {
                let chunk_len = (i - lit_pos).min(128);
                out.push((chunk_len - 1) as u8);
                out.extend_from_slice(&sdu[lit_pos..lit_pos + chunk_len]);
                lit_pos += chunk_len;
            }
        }

        out
    }

    /// Get current compression ratio (compressed / uncompressed).
    pub fn compression_ratio(&self) -> f32 {
        if self.total_raw_bytes == 0 {
            1.0
        } else {
            self.total_compressed_bytes as f32 / self.total_raw_bytes as f32
        }
    }
}

/// UDC Decompressor operating on the gNB reception side.
#[derive(Debug, Clone)]
pub struct UdcDecompressor {
    config: UdcConfig,
    dictionary: SlidingDictionary,
    pub is_desynchronized: bool,
    total_decompressed_bytes: usize,
}

impl UdcDecompressor {
    pub fn new(config: UdcConfig) -> Self {
        let mut dict = SlidingDictionary::new(config.buffer_size as usize);
        if let Some(ref predef) = config.predefined_dictionary {
            dict.append(predef);
        }

        Self {
            config,
            dictionary: dict,
            is_desynchronized: false,
            total_decompressed_bytes: 0,
        }
    }

    /// Decompress a received UDC PDU.
    /// Returns the reconstructed uncompressed SDU.
    /// If checksum validation fails, triggers desynchronization and returns `Err`.
    pub fn decompress_pdu(&mut self, pdu: &[u8]) -> Result<Vec<u8>, &'static str> {
        if pdu.is_empty() {
            return Err("Empty UDC PDU payload");
        }

        let header = UdcHeader::parse(pdu[0]);
        let payload = &pdu[1..];

        // 1. Handle Field Reset (FR)
        if header.fr {
            self.dictionary.clear();
            if let Some(ref predef) = self.config.predefined_dictionary {
                self.dictionary.append(predef);
            }
            self.is_desynchronized = false;
        }

        // 2. Reject if desynchronized and not reset
        if self.is_desynchronized {
            return Err("Decompressor is in desynchronized state; awaiting reset");
        }

        // 3. Decompress or pass-through
        let reconstructed = if header.fu {
            match self.lz77_decompress(payload) {
                Ok(r) => r,
                Err(_) => {
                    self.is_desynchronized = true;
                    return Err("Decompression parsing failed: desynchronization detected");
                }
            }
        } else {
            payload.to_vec()
        };

        // 4. Verify 4-bit CRC Checksum
        let computed_cs = compute_udc_crc4(&reconstructed);
        if computed_cs != header.checksum {
            self.is_desynchronized = true;
            return Err("Checksum mismatch: desynchronization detected");
        }

        // 5. Update local dictionary
        self.dictionary.append(&reconstructed);
        self.total_decompressed_bytes += reconstructed.len();

        Ok(reconstructed)
    }

    /// Decompress LZ77 byte stream using tag-based framing.
    fn lz77_decompress(&self, payload: &[u8]) -> Result<Vec<u8>, &'static str> {
        let mut out = Vec::new();
        let dict = self.dictionary.as_slice();
        let dict_len = dict.len();

        let mut i = 0;
        while i < payload.len() {
            let tag = payload[i];
            i += 1;

            if (tag & 0x80) == 0 {
                // Literal run: length = tag + 1
                let run_len = (tag as usize) + 1;
                if i + run_len > payload.len() {
                    return Err("Truncated literal run in UDC payload");
                }
                out.extend_from_slice(&payload[i..i + run_len]);
                i += run_len;
            } else {
                // Match token: length = (tag & 0x7F) + 3
                let match_len = (tag & 0x7F) as usize + 3;
                if i + 2 > payload.len() {
                    return Err("Truncated match token in UDC payload");
                }
                let dist = ((payload[i] as usize) << 8) | (payload[i + 1] as usize);
                i += 2;

                if dist == 0 || dist > dict_len {
                    return Err("Match distance exceeds dictionary buffer");
                }
                let start_pos = dict_len - dist;
                if start_pos + match_len > dict_len {
                    return Err("Match length exceeds dictionary buffer boundary");
                }
                out.extend_from_slice(&dict[start_pos..start_pos + match_len]);
            }
        }
        Ok(out)
    }

    /// Generate a UDC Feedback Control PDU requesting a dictionary reset.
    pub fn generate_reset_feedback(&self) -> UdcFeedbackPdu {
        UdcFeedbackPdu { fe: true }
    }
}

/// High-level 3GPP Rel-17 UDC PDCP Engine coupling compressor and decompressor.
#[derive(Debug, Clone)]
pub struct UdcEngine {
    pub compressor: UdcCompressor,
    pub decompressor: UdcDecompressor,
}

impl UdcEngine {
    pub fn new(config: UdcConfig) -> Self {
        Self {
            compressor: UdcCompressor::new(config.clone()),
            decompressor: UdcDecompressor::new(config),
        }
    }

    /// Compress uplink SDU at UE.
    pub fn compress_uplink(&mut self, sdu: &[u8]) -> Vec<u8> {
        self.compressor.compress_sdu(sdu)
    }

    /// Decompress uplink SDU at gNB.
    pub fn decompress_uplink(&mut self, pdu: &[u8]) -> Result<Vec<u8>, &'static str> {
        self.decompressor.decompress_pdu(pdu)
    }

    /// Process feedback received at UE from gNB.
    pub fn handle_feedback(&mut self, feedback: &UdcFeedbackPdu) {
        if feedback.fe {
            self.compressor.trigger_reset();
        }
    }
}
