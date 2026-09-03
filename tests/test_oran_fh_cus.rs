//! Integration tests for the O-RAN WG4 Open Fronthaul C/U-Plane application layer.

use toy_tcpip::oran_fh_cus::{
    CPlaneMessage, CPlaneSection, DataDirection, EaxcId, EaxcIdFormat, ORAN_CPLANE_HEADER_LEN,
    ORAN_CPLANE_SECTION_LEN, ORAN_PAYLOAD_VERSION, OranError, OranFlowMonitor, OranRadioHeader,
    OranSectionType, UPlaneMessage, UPlaneSection, UdCompHeader, UdCompMethod,
};

// The binary literal below is grouped by eAxC subfield (2 / 4 / 4 / 6 bits), not by nibble.
#[allow(clippy::unusual_byte_groupings)]
#[test]
fn test_eaxc_id_bit_packing_and_format_validation() {
    let format = EaxcIdFormat::typical();
    assert_eq!(format.du_port_bits, 2);
    assert_eq!(format.ru_port_bits, 6);

    // DU_Port | BandSector | CC | RU_Port packed most significant first.
    let eaxc = EaxcId::new(1, 2, 3, 17);
    let raw = eaxc.pack(format).unwrap();
    assert_eq!(raw, 0x48D1);
    assert_eq!(raw, 0b01_0010_0011_010001);
    assert_eq!(EaxcId::unpack(raw, format), eaxc);

    // A different split of the same 16 bits reinterprets the same word.
    let wide_ru = EaxcIdFormat::new(1, 3, 4, 8).unwrap();
    let reinterpreted = EaxcId::unpack(raw, wide_ru);
    assert_eq!(reinterpreted.ru_port_id, 0xD1);
    assert_eq!(reinterpreted.du_port_id, 0);

    // Subfield widths must cover exactly 16 bits.
    assert_eq!(
        EaxcIdFormat::new(2, 4, 4, 4),
        Err(OranError::InvalidEaxcFormat(14))
    );

    // A value wider than its configured field is a configuration error, not a silent wrap.
    assert_eq!(
        EaxcId::new(4, 0, 0, 0).pack(format),
        Err(OranError::EaxcFieldOverflow {
            field: "DU_Port_ID",
            value: 4,
            bits: 2
        })
    );
    assert_eq!(EaxcId::new(3, 15, 15, 63).pack(format), Ok(0xFFFF));
}

#[test]
fn test_oran_radio_header_bit_layout_round_trip() {
    let header = OranRadioHeader::new(DataDirection::Downlink, 17, 3, 5, 9);
    let bytes = header.serialize();

    // dataDirection 1, payloadVersion 1, filterIndex 0.
    assert_eq!(bytes[0], 0x90);
    assert_eq!(bytes[1], 17);
    // subframeId 3 with the four high bits of slotId 5.
    assert_eq!(bytes[2], 0x31);
    // The two low bits of slotId straddle into the symbolId octet.
    assert_eq!(bytes[3], 0x49);

    let parsed = OranRadioHeader::parse(&bytes).unwrap();
    assert_eq!(parsed, header);
    assert_eq!(parsed.slot_id, 5);
    assert_eq!(parsed.symbol_id, 9);
    assert_eq!(parsed.payload_version, ORAN_PAYLOAD_VERSION);
    assert_eq!(parsed.data_direction, DataDirection::Downlink);

    let uplink = OranRadioHeader::new(DataDirection::Uplink, 0, 9, 63, 13);
    assert_eq!(uplink.serialize()[0] >> 7, 0);
    assert_eq!(OranRadioHeader::parse(&uplink.serialize()).unwrap(), uplink);

    // Only payloadVersion 1 is defined by the CUS-Plane specification.
    let mut wrong_version = bytes;
    wrong_version[0] = 0xA0;
    assert_eq!(
        OranRadioHeader::parse(&wrong_version),
        Err(OranError::UnsupportedPayloadVersion(2))
    );
    assert_eq!(
        OranRadioHeader::parse(&bytes[..3]),
        Err(OranError::Truncated { need: 4, got: 3 })
    );

    // A radio frame has ten subframes, so subframeId 10 cannot be scheduled.
    let bad = OranRadioHeader::new(DataDirection::Downlink, 0, 10, 0, 0);
    assert_eq!(
        bad.validate(),
        Err(OranError::FieldOutOfRange {
            field: "subframeId",
            value: 10
        })
    );

    // Symbols linearize across frame / subframe / slot for ordering checks (mu = 1).
    let first = OranRadioHeader::new(DataDirection::Downlink, 0, 0, 0, 0);
    let next_slot = OranRadioHeader::new(DataDirection::Downlink, 0, 0, 1, 0);
    let next_subframe = OranRadioHeader::new(DataDirection::Downlink, 0, 1, 0, 0);
    assert_eq!(first.symbol_index(1), 0);
    assert_eq!(next_slot.symbol_index(1), 14);
    assert_eq!(next_subframe.symbol_index(1), 28);
}

#[test]
fn test_ud_comp_header_and_prb_sizing() {
    // udIqWidth 0 on the wire means 16 bits per I or Q sample.
    let uncompressed = UdCompHeader::parse(0x00);
    assert_eq!(uncompressed.iq_width, 16);
    assert_eq!(uncompressed.method, UdCompMethod::NoCompression);
    assert_eq!(uncompressed.serialize(), 0x00);
    // 12 subcarriers x I and Q x 16 bits = 48 bytes per PRB.
    assert_eq!(uncompressed.prb_payload_bytes(), 48);

    let bfp = UdCompHeader::new(9, UdCompMethod::BlockFloatingPoint);
    assert_eq!(bfp.serialize(), 0x91);
    assert_eq!(UdCompHeader::parse(0x91), bfp);
    // 12 x 2 x 9 bits = 216 bits, rounded up to 27 bytes.
    assert_eq!(bfp.prb_payload_bytes(), 27);
    assert!(bfp.method.has_per_prb_comp_param());
    assert!(!uncompressed.method.has_per_prb_comp_param());

    assert_eq!(UdCompMethod::from_u4(0x3), UdCompMethod::MuLaw);
    assert_eq!(UdCompMethod::from_u4(0xE), UdCompMethod::Reserved(0xE));
    assert_eq!(UdCompMethod::Reserved(0xE).to_u4(), 0xE);
}

#[test]
fn test_u_plane_section_framing_with_dynamic_compression() {
    let header = OranRadioHeader::new(DataDirection::Downlink, 17, 3, 5, 9);
    let section = UPlaneSection::new(0x123, 0x0AB, 4, vec![0xA5; 8])
        .with_compression(UdCompHeader::new(9, UdCompMethod::BlockFloatingPoint));
    let message = UPlaneMessage::new(header, vec![section]);

    let wire = message.serialize().unwrap();
    assert_eq!(&wire[0..4], &[0x90, 0x11, 0x31, 0x49]);
    // sectionId 0x123 over 12 bits, rb and symInc clear, startPrbu 0x0AB over 10 bits.
    assert_eq!(&wire[4..8], &[0x12, 0x30, 0xAB, 0x04]);
    // udCompHdr followed by its reserved octet.
    assert_eq!(&wire[8..10], &[0x91, 0x00]);
    assert_eq!(wire.len(), 4 + 4 + 2 + 8);

    let parsed = UPlaneMessage::parse(&wire, true, 2).unwrap();
    assert_eq!(parsed, message);
    assert_eq!(parsed.sections[0].start_prbu, 0x0AB);
    assert_eq!(parsed.sections[0].num_prbu, 4);
    assert_eq!(parsed.sections[0].iq_samples.len(), 8);

    // Without dynamic compression the two compression octets are absent.
    let plain = UPlaneMessage::new(header, vec![UPlaneSection::new(1, 0, 2, vec![0x11; 4])]);
    let plain_wire = plain.serialize().unwrap();
    assert_eq!(plain_wire.len(), 4 + 4 + 4);
    assert_eq!(UPlaneMessage::parse(&plain_wire, false, 2).unwrap(), plain);

    // numPrbu 0 means every remaining PRB of the carrier.
    let all_prbs = UPlaneSection::new(7, 30, 0, vec![0x22; 6]);
    assert_eq!(all_prbs.prb_count(273), 243);
    assert_eq!(UPlaneSection::new(7, 30, 4, vec![]).prb_count(273), 4);

    // A section whose IQ payload is shorter than numPrbu implies is a truncated frame.
    let mut truncated = plain_wire.clone();
    truncated.truncate(plain_wire.len() - 1);
    assert_eq!(
        UPlaneMessage::parse(&truncated, false, 2),
        Err(OranError::Truncated { need: 12, got: 11 })
    );

    // sectionId is a 12-bit field.
    let overflow = UPlaneMessage::new(header, vec![UPlaneSection::new(0x1000, 0, 1, vec![])]);
    assert_eq!(
        overflow.serialize(),
        Err(OranError::FieldOutOfRange {
            field: "sectionId",
            value: 0x1000
        })
    );
}

#[test]
fn test_c_plane_section_type_1_scheduling_message() {
    let header = OranRadioHeader::new(DataDirection::Downlink, 17, 3, 5, 0);
    let section = CPlaneSection::new(1, 0, 100, 14, 0x0101);
    let message = CPlaneMessage::new(
        header,
        UdCompHeader::new(16, UdCompMethod::NoCompression),
        vec![section],
    );

    let wire = message.serialize().unwrap();
    assert_eq!(wire.len(), ORAN_CPLANE_HEADER_LEN + ORAN_CPLANE_SECTION_LEN);
    assert_eq!(&wire[0..4], &[0x90, 0x11, 0x31, 0x40]);
    assert_eq!(wire[4], 1, "numberOfSections");
    assert_eq!(wire[5], 1, "sectionType 1");
    assert_eq!(wire[6], 0x00, "udCompHdr: 16-bit IQ, no compression");
    // sectionId 1, startPrbc 0, numPrbc 100, reMask 0xFFF, numSymbol 14, ef 0, beamId 0x0101.
    assert_eq!(
        &wire[8..16],
        &[0x00, 0x10, 0x00, 0x64, 0xFF, 0xFE, 0x01, 0x01]
    );

    let parsed = CPlaneMessage::parse(&wire).unwrap();
    assert_eq!(parsed, message);
    assert_eq!(parsed.section_type, OranSectionType::DlUlRadioChannel);
    assert_eq!(parsed.sections[0].re_mask, 0x0FFF);
    assert_eq!(parsed.sections[0].num_symbol, 14);
    assert_eq!(parsed.sections[0].beam_id, 0x0101);
    assert!(!parsed.sections[0].extension_flag);

    // 100 PRBs across 14 symbols.
    assert_eq!(parsed.scheduled_prbs(273), 1400);

    // numberOfSections must agree with the sections actually carried.
    let mut miscounted = wire.clone();
    miscounted[4] = 2;
    assert_eq!(
        CPlaneMessage::parse(&miscounted),
        Err(OranError::SectionCountMismatch {
            declared: 2,
            parsed: 1
        })
    );

    // Section extensions change the section length, so an ef section is not decoded blind.
    let mut extended = wire.clone();
    extended[14] |= 0x80;
    assert_eq!(
        CPlaneMessage::parse(&extended),
        Err(OranError::SectionExtensionUnsupported(1))
    );

    // Section Type 3 extends the common header with timeOffset / frameStructure / cpLength.
    let mut prach = wire.clone();
    prach[5] = 3;
    assert_eq!(
        CPlaneMessage::parse(&prach),
        Err(OranError::UnsupportedSectionType(3))
    );
    assert_eq!(
        OranSectionType::from_u8(3),
        OranSectionType::PrachMixedNumerology
    );
    assert_eq!(OranSectionType::from_u8(4), OranSectionType::Reserved(4));
    assert_eq!(OranSectionType::UeScheduling.to_u8(), 5);

    // beamId is a 15-bit field: bit 15 of that octet is the extension flag.
    let mut oversized = section;
    oversized.beam_id = 0x8000;
    assert_eq!(
        oversized.serialize(),
        Err(OranError::FieldOutOfRange {
            field: "beamId",
            value: 0x8000
        })
    );
}

#[test]
fn test_oran_flow_monitor_detects_unscheduled_and_late_symbols() {
    let format = EaxcIdFormat::typical();
    let eaxc = EaxcId::new(1, 2, 3, 17).pack(format).unwrap();
    let mut monitor = OranFlowMonitor::new(format, 1, 273);

    // The C-Plane schedules symbols 0..13 of frame 0, subframe 0, slot 0.
    let c_header = OranRadioHeader::new(DataDirection::Downlink, 0, 0, 0, 0);
    let schedule = CPlaneMessage::new(
        c_header,
        UdCompHeader::new(16, UdCompMethod::NoCompression),
        vec![CPlaneSection::new(1, 0, 100, 14, 1)],
    );
    monitor.observe_c_plane(eaxc, &schedule);

    let u_plane = |symbol: u8, slot: u8| {
        UPlaneMessage::new(
            OranRadioHeader::new(DataDirection::Downlink, 0, 0, slot, symbol),
            vec![UPlaneSection::new(1, 0, 4, vec![0; 8])],
        )
    };

    // Symbol 3 was scheduled and arrives in order.
    monitor.observe_u_plane(eaxc, &u_plane(3, 0));
    let stats = monitor.stats(eaxc).unwrap();
    assert_eq!(stats.u_plane_messages, 1);
    assert_eq!(stats.c_plane_messages, 1);
    assert_eq!(stats.prb_count, 4);
    assert_eq!(stats.unscheduled_symbols, 0);
    assert_eq!(stats.out_of_order_symbols, 0);

    // Symbol 2 arrives after symbol 3: an O-RU cannot play it out any more.
    monitor.observe_u_plane(eaxc, &u_plane(2, 0));
    assert_eq!(monitor.stats(eaxc).unwrap().out_of_order_symbols, 1);
    assert_eq!(monitor.stats(eaxc).unwrap().last_symbol_index, Some(3));

    // Slot 1 symbol 0 was never scheduled by a C-Plane section.
    monitor.observe_u_plane(eaxc, &u_plane(0, 1));
    let stats = monitor.stats(eaxc).unwrap();
    assert_eq!(stats.unscheduled_symbols, 1);
    assert_eq!(stats.out_of_order_symbols, 1);
    assert_eq!(stats.u_plane_messages, 3);
    assert_eq!(stats.prb_count, 12);
    assert_eq!(stats.last_symbol_index, Some(14));

    // Flows are keyed by the packed eAxC word and decode back to their subfields.
    assert_eq!(monitor.flow_count(), 1);
    assert_eq!(monitor.decode_eaxc(eaxc), EaxcId::new(1, 2, 3, 17));
    assert!(monitor.stats(0x0000).is_none());
}
