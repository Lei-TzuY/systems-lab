//! Integration tests for O-RAN fronthaul IQ sample compression.

use toy_tcpip::oran_iq_compression::{
    BfpCodec, CompressionError, IqSample, MuLawCodec, SUBCARRIERS_PER_PRB, SelectiveReCodec,
    UNCOMPRESSED_PRB_BYTES, fronthaul_bitrate_bps, measure_quality, mu_law_compress, mu_law_expand,
    pack_signed, unpack_signed,
};

/// One PRB with a peak of 1100, which forces a block exponent of 3 at 9-bit width.
fn test_prb() -> Vec<IqSample> {
    (0..SUBCARRIERS_PER_PRB)
        .map(|k| IqSample::new(100 * k as i16, -50 * k as i16))
        .collect()
}

#[test]
fn test_msb_first_bit_packing_round_trip() {
    // A single 9-bit value of 1 occupies the top nine bits of the stream.
    assert_eq!(pack_signed(&[1], 9).unwrap(), vec![0x00, 0x80]);
    // -1 is all ones in two's complement.
    assert_eq!(pack_signed(&[-1], 9).unwrap(), vec![0xFF, 0x80]);

    // Samples run continuously, so four 9-bit values need five octets, not eight.
    let values = vec![1, -1, 255, -256];
    let packed = pack_signed(&values, 9).unwrap();
    assert_eq!(packed.len(), 5);
    assert_eq!(unpack_signed(&packed, 9, 4).unwrap(), values);

    // Widths that are not byte multiples round trip at every size the fronthaul uses.
    for width in [4u8, 6, 8, 9, 12, 14, 16] {
        let max = (1i32 << (width - 1)) - 1;
        let min = -(1i32 << (width - 1));
        let values = vec![0, 1, -1, max, min, max / 2, min / 2];
        let packed = pack_signed(&values, width).unwrap();
        assert_eq!(packed.len(), (values.len() * width as usize).div_ceil(8));
        assert_eq!(
            unpack_signed(&packed, width, values.len()).unwrap(),
            values,
            "round trip failed at {} bits",
            width
        );
    }

    // A value wider than the packing field is a caller error, not a silent truncation.
    assert_eq!(
        pack_signed(&[256], 9),
        Err(CompressionError::ValueOutOfRange {
            value: 256,
            width: 9
        })
    );
    assert_eq!(
        unpack_signed(&[0x00], 9, 1),
        Err(CompressionError::Truncated { need: 2, got: 1 })
    );
    assert_eq!(
        pack_signed(&[0], 17),
        Err(CompressionError::UnsupportedIqWidth(17))
    );
    assert_eq!(
        pack_signed(&[0], 0),
        Err(CompressionError::UnsupportedIqWidth(0))
    );
}

#[test]
fn test_block_floating_point_exponent_and_round_trip() {
    let codec = BfpCodec::new(9).unwrap();
    let prb = test_prb();

    // A peak of 1100 needs three shifts to fit a 9-bit signed mantissa (max 255).
    assert_eq!(codec.block_exponent(&prb), 3);

    // udCompParam octet plus 24 mantissas of 9 bits = 1 + 27 bytes.
    assert_eq!(codec.compressed_prb_bytes(), 28);
    assert!(codec.compression_ratio() < 0.6);

    let compressed = codec.compress(&prb).unwrap();
    assert_eq!(compressed.len(), 28);
    // The exponent occupies the low nibble of the first octet.
    assert_eq!(compressed[0], 3);

    let recovered = codec.decompress(&compressed, 1).unwrap();
    assert_eq!(recovered.len(), SUBCARRIERS_PER_PRB);

    // Quantization to multiples of 2^3 bounds the error at 7.
    let quality = measure_quality(&prb, &recovered).unwrap();
    assert!(quality.max_absolute_error <= 7, "{:?}", quality);
    assert!(quality.snr_db > 40.0, "{:?}", quality);

    // Every PRB carries its own exponent, so a quiet PRB keeps full precision.
    let mut two_prbs = test_prb();
    two_prbs.extend((0..SUBCARRIERS_PER_PRB).map(|k| IqSample::new(k as i16, -(k as i16))));
    let compressed = codec.compress(&two_prbs).unwrap();
    assert_eq!(compressed.len(), 56);
    assert_eq!(compressed[0], 3, "loud PRB");
    assert_eq!(compressed[28], 0, "quiet PRB needs no shift");
    let recovered = codec.decompress(&compressed, 2).unwrap();
    assert_eq!(&recovered[12..], &two_prbs[12..], "quiet PRB is lossless");

    // At full width the exponent is zero and nothing is lost, but the extra
    // udCompParam octet makes the PRB larger than raw IQ.
    let wide = BfpCodec::new(16).unwrap();
    assert_eq!(wide.compressed_prb_bytes(), UNCOMPRESSED_PRB_BYTES + 1);
    assert!(wide.compression_ratio() > 1.0);
    let compressed = wide.compress(&prb).unwrap();
    assert_eq!(compressed[0], 0);
    assert_eq!(wide.decompress(&compressed, 1).unwrap(), prb);

    // Samples must arrive as whole PRBs, and a short buffer must not be decoded.
    assert_eq!(
        codec.compress(&prb[..5]),
        Err(CompressionError::NotPrbAligned(5))
    );
    assert_eq!(codec.compress(&[]), Err(CompressionError::NotPrbAligned(0)));
    assert_eq!(
        codec.decompress(&compressed[..10], 1),
        Err(CompressionError::Truncated { need: 28, got: 10 })
    );

    // The exponent has to fit the 4-bit udCompParam nibble, so 3-bit mantissas are refused.
    assert_eq!(
        BfpCodec::new(3),
        Err(CompressionError::UnsupportedIqWidth(3))
    );
    assert_eq!(
        BfpCodec::new(17),
        Err(CompressionError::UnsupportedIqWidth(17))
    );
}

#[test]
fn test_selective_re_sending_follows_the_c_plane_mask() {
    // reMask bit 11 selects subcarrier 0, so 0xF00 carries subcarriers 0 to 3.
    let codec = SelectiveReCodec::new(9, 0xF00).unwrap();
    assert_eq!(codec.selected_res(), 4);
    // 1 exponent octet + 8 mantissas of 9 bits = 1 + 9 bytes, a third of a full PRB.
    assert_eq!(codec.compressed_prb_bytes(), 10);

    let prb = test_prb();
    let compressed = codec.compress(&prb).unwrap();
    assert_eq!(compressed.len(), 10);
    // Only subcarriers 0 to 3 are considered, so the peak is 300, not 1100.
    assert_eq!(compressed[0], 1);

    let recovered = codec.decompress(&compressed, 1).unwrap();
    assert_eq!(recovered.len(), SUBCARRIERS_PER_PRB);
    // Selected resource elements come back within the quantization step of 2.
    for k in 0..4 {
        assert!((recovered[k].i - prb[k].i).abs() <= 1, "subcarrier {}", k);
        assert!((recovered[k].q - prb[k].q).abs() <= 1, "subcarrier {}", k);
    }
    // Everything the mask excluded is restored as silence.
    for (k, sample) in recovered.iter().enumerate().skip(4) {
        assert_eq!(*sample, IqSample::default(), "subcarrier {}", k);
    }

    // A scattered mask selects the same count wherever the bits sit.
    let scattered = SelectiveReCodec::new(9, 0b1010_1010_1010).unwrap();
    assert_eq!(scattered.selected_res(), 6);
    assert_eq!(
        scattered.compressed_prb_bytes(),
        1 + (12 * 9usize).div_ceil(8)
    );
    let recovered = scattered
        .decompress(&scattered.compress(&prb).unwrap(), 1)
        .unwrap();
    assert_eq!(recovered[1], IqSample::default(), "odd subcarriers dropped");
    assert!(recovered[0].i.abs() <= 1);
    assert!(recovered[2].i > 0, "even subcarriers survive");

    // Bits above the 12-bit mask are ignored, and an empty mask carries nothing.
    assert_eq!(
        SelectiveReCodec::new(9, 0xF000),
        Err(CompressionError::EmptyResourceMask)
    );
    assert_eq!(
        SelectiveReCodec::new(9, 0),
        Err(CompressionError::EmptyResourceMask)
    );
}

#[test]
fn test_mu_law_companding_curve() {
    // Silence encodes to the classic 0xFF and expands back to exactly zero.
    assert_eq!(mu_law_compress(0), 0xFF);
    assert_eq!(mu_law_expand(0xFF), 0);

    // A mid-scale sample: segment 3, mantissa 1.
    assert_eq!(mu_law_compress(1000), 0xCE);
    assert_eq!(mu_law_expand(0xCE), 988);
    // The curve is symmetric about zero.
    assert_eq!(mu_law_compress(-1000), 0x4E);
    assert_eq!(mu_law_expand(0x4E), -988);

    // Full scale clips into the top segment.
    assert_eq!(mu_law_compress(32767), 0x80);
    assert_eq!(mu_law_expand(0x80), 32124);
    assert_eq!(mu_law_compress(i16::MIN), 0x00);
    assert_eq!(mu_law_expand(0x00), -32124);

    // mu-law has two codes for zero; negative zero re-encodes to the positive one.
    assert_eq!(mu_law_expand(0x7F), 0);
    assert_eq!(mu_law_compress(mu_law_expand(0x7F)), 0xFF);

    // Every other octet decodes and re-encodes to itself: the curve is a stable quantizer.
    for byte in 0..=255u8 {
        if byte == 0x7F {
            continue;
        }
        assert_eq!(
            mu_law_compress(mu_law_expand(byte)),
            byte,
            "byte {:#04X}",
            byte
        );
    }

    // Companding is logarithmic: quiet samples keep far more accuracy than loud ones.
    let quiet_error = (mu_law_expand(mu_law_compress(100)) - 100).abs();
    let loud_error = (mu_law_expand(mu_law_compress(20_000)) - 20_000).abs();
    assert!(
        quiet_error < loud_error,
        "{} vs {}",
        quiet_error,
        loud_error
    );
    assert!(quiet_error <= 4);

    // One octet per I or Q value: exactly half of the uncompressed PRB.
    let codec = MuLawCodec;
    let prb = test_prb();
    let compressed = codec.compress(&prb);
    assert_eq!(compressed.len(), UNCOMPRESSED_PRB_BYTES / 2);
    let recovered = codec.decompress(&compressed);
    assert_eq!(recovered.len(), SUBCARRIERS_PER_PRB);
    assert!(measure_quality(&prb, &recovered).unwrap().snr_db > 30.0);
}

#[test]
fn test_quality_metrics_and_fronthaul_bitrate() {
    let prb = test_prb();

    // An exact copy has no error at all.
    let perfect = measure_quality(&prb, &prb).unwrap();
    assert_eq!(perfect.max_absolute_error, 0);
    assert_eq!(perfect.evm_percent, 0.0);
    assert!(perfect.snr_db.is_infinite());
    assert_eq!(perfect.sample_count, SUBCARRIERS_PER_PRB);

    // Wider mantissas buy signal quality; the metrics have to show it.
    let narrow = BfpCodec::new(6).unwrap();
    let wide = BfpCodec::new(14).unwrap();
    let narrow_quality = measure_quality(
        &prb,
        &narrow
            .decompress(&narrow.compress(&prb).unwrap(), 1)
            .unwrap(),
    )
    .unwrap();
    let wide_quality = measure_quality(
        &prb,
        &wide.decompress(&wide.compress(&prb).unwrap(), 1).unwrap(),
    )
    .unwrap();
    assert!(wide_quality.snr_db > narrow_quality.snr_db);
    assert!(wide_quality.max_absolute_error < narrow_quality.max_absolute_error);
    assert!(wide_quality.evm_percent < narrow_quality.evm_percent);
    // 6-bit mantissas force an exponent of 6, so the error stays inside that 64-wide step
    // - a fifth of a percent of full scale, though 6.6% of this PRB's own amplitude.
    assert!(
        narrow_quality.max_absolute_error < 64,
        "{:?}",
        narrow_quality
    );
    assert!(narrow_quality.evm_percent < 10.0, "{:?}", narrow_quality);

    // A 14-bit mantissa holds this PRB's peak of 1100 outright, so BFP is lossless here.
    assert_eq!(wide_quality.max_absolute_error, 0);
    assert_eq!(wide_quality.evm_percent, 0.0);
    assert!(wide_quality.snr_db.is_infinite());

    // Mismatched or empty inputs have no meaningful comparison.
    assert!(measure_quality(&prb, &prb[..6]).is_none());
    assert!(measure_quality(&[], &[]).is_none());

    // 100 MHz carrier (273 PRBs), mu = 1 (28000 symbols/s), 4 layers.
    let compressed_bps = fronthaul_bitrate_bps(273, 28_000, 4, 28);
    let raw_bps = fronthaul_bitrate_bps(273, 28_000, 4, UNCOMPRESSED_PRB_BYTES);
    assert_eq!(compressed_bps, 6_849_024_000);
    assert_eq!(raw_bps, 11_741_184_000);
    // 9-bit BFP fits a 4-layer 100 MHz cell into a 10G fronthaul link; raw IQ does not.
    assert!(compressed_bps < 10_000_000_000);
    assert!(raw_bps > 10_000_000_000);
}
