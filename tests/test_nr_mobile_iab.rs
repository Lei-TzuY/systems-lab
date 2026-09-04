//! Integration Tests for 3GPP Release 18 Mobile Integrated Access and Backhaul (Mobile IAB) & BAP Engine.

use toy_tcpip::nr_mobile_iab::*;

#[test]
fn test_bap_data_pdu_binary_codec_and_bitfield_layout() {
    let address = BapAddress::new(0x3A5).expect("Valid 10-bit address: 933");
    let path_id = BapPathId::new(0x1F4).expect("Valid 10-bit path ID: 500");
    let payload = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03];

    let pdu = BapDataPdu::new(address, path_id, payload.clone());
    let encoded = pdu.encode();

    assert_eq!(encoded.len(), 3 + payload.len());

    // Inspect 3-byte header exact bit layout (TS 38.340 §6.2.2):
    // address = 0x3A5 = 0b11_1010_0101
    // path_id = 0x1F4 = 0b01_1111_0100
    //
    // Octet 1: D/C=1 (bit 7), R=000 (bits 6..4), Address[9:6] = 0b1110 = 0x0E (bits 3..0)
    // -> 0x80 | 0x0E = 0x8E
    assert_eq!(encoded[0], 0x8E);

    // Octet 2: Address[5:0] = 0b100101 = 0x25 (bits 7..2), PathId[9:8] = 0b01 = 0x01 (bits 1..0)
    // -> (0x25 << 2) | 0x01 = 0x94 | 0x01 = 0x95
    assert_eq!(encoded[1], 0x95);

    // Octet 3: PathId[7:0] = 0b1111_0100 = 0xF4
    assert_eq!(encoded[2], 0xF4);

    // Payload starts at byte 3
    assert_eq!(&encoded[3..], &payload[..]);

    // Decode and verify roundtrip
    let decoded = BapDataPdu::decode(&encoded).expect("Decode BAP Data PDU");
    assert_eq!(decoded.destination_address, address);
    assert_eq!(decoded.path_id, path_id);
    assert_eq!(decoded.payload, payload);

    // Truncated packet error check
    assert!(matches!(
        BapDataPdu::decode(&encoded[0..2]),
        Err(MobileIabError::PacketTooShort { .. })
    ));

    // D/C bit = 0 (Control PDU) passed to Data PDU decoder should fail
    let mut corrupt = encoded.clone();
    corrupt[0] &= 0x7F; // clear D/C bit
    assert!(matches!(
        BapDataPdu::decode(&corrupt),
        Err(MobileIabError::InvalidControlPduType(..))
    ));
}

#[test]
fn test_bap_control_pdu_flow_control_and_failure_indications() {
    // 1. Flow Control Feedback per BH RLC Channel
    let fc_bh = BapControlPdu::FlowControlFeedbackBhRlc {
        bh_rlc_channel_id: 105,
        available_buffer_bytes: 524_288, // 512 KB
    };
    let enc_fc_bh = fc_bh.encode();
    assert_eq!(
        (enc_fc_bh[0] >> 3) & 0x0F,
        BapControlPduType::FlowControlFeedbackBhRlc.to_u8()
    );
    assert_eq!(enc_fc_bh[0] & 0x80, 0); // D/C = 0

    let dec_fc_bh = BapControlPdu::decode(&enc_fc_bh).expect("Decode Flow Control BH");
    assert_eq!(dec_fc_bh, fc_bh);

    // 2. Flow Control Feedback per Routing ID
    let routing_id = BapRoutingId::new(BapAddress::new(250).unwrap(), BapPathId::new(45).unwrap());
    let fc_route = BapControlPdu::FlowControlFeedbackRoutingId {
        routing_id,
        available_buffer_bytes: 1_048_576, // 1 MB
    };
    let enc_fc_route = fc_route.encode();
    let dec_fc_route = BapControlPdu::decode(&enc_fc_route).expect("Decode Flow Control Route");
    assert_eq!(dec_fc_route, fc_route);

    // 3. BH RLC Channel Failure Indication
    let fail_chan = BapControlPdu::BhRlcChannelFailureIndication {
        failed_bh_rlc_channel_id: 88,
    };
    let enc_fail_chan = fail_chan.encode();
    let dec_fail_chan = BapControlPdu::decode(&enc_fail_chan).expect("Decode Channel Failure");
    assert_eq!(dec_fail_chan, fail_chan);

    // 4. BH Routing ID Failure Indication
    let fail_route = BapControlPdu::BhRoutingIdFailureIndication {
        failed_routing_id: routing_id,
    };
    let enc_fail_route = fail_route.encode();
    let dec_fail_route = BapControlPdu::decode(&enc_fail_route).expect("Decode Route Failure");
    assert_eq!(dec_fail_route, fail_route);

    // 5. Flow Control Polling
    let poll = BapControlPdu::FlowControlPolling { query_id: 1001 };
    let enc_poll = poll.encode();
    let dec_poll = BapControlPdu::decode(&enc_poll).expect("Decode Poll");
    assert_eq!(dec_poll, poll);
}

#[test]
fn test_bap_multi_hop_routing_and_failover_recovery() {
    let mut table = BapRoutingTable::new();
    let dest = BapAddress::new(200).unwrap();
    let path = BapPathId::new(10).unwrap();

    // Primary next-hop: Node 2, Channel 101. Backup next-hop: Node 3, Channel 102.
    table.insert_route(dest, path, 2, 101, Some(3), Some(102));

    // Normal resolution selects primary
    let res1 = table.resolve(dest, path).expect("Resolve primary route");
    assert_eq!(res1.next_hop_node_id, 2);
    assert_eq!(res1.egress_bh_rlc_channel_id, 101);
    assert!(!res1.is_using_backup);

    // Radio Link Failure on primary Channel 101
    let affected = table.mark_channel_failure(101);
    assert_eq!(affected, 1);

    // Immediate failover to backup
    let res2 = table
        .resolve(dest, path)
        .expect("Resolve backup route after RLF");
    assert_eq!(res2.next_hop_node_id, 3);
    assert_eq!(res2.egress_bh_rlc_channel_id, 102);
    assert!(res2.is_using_backup);

    // Link restored
    table.restore_channel(101);
    let res3 = table
        .resolve(dest, path)
        .expect("Resolve after channel restored");
    assert_eq!(res3.next_hop_node_id, 2);
    assert_eq!(res3.egress_bh_rlc_channel_id, 101);
    assert!(!res3.is_using_backup);

    // Default route fallback
    table.set_default_route(99, 999, None, None);
    let unreg_dest = BapAddress::new(777).unwrap();
    let unreg_path = BapPathId::new(888).unwrap();
    let res_def = table
        .resolve(unreg_dest, unreg_path)
        .expect("Resolve default route");
    assert_eq!(res_def.next_hop_node_id, 99);
    assert_eq!(res_def.egress_bh_rlc_channel_id, 999);
}

#[test]
fn test_iab_half_duplex_tdm_slot_multiplexing_and_guard_symbols() {
    // Slot 0: MT Hard, DU NotAvailable (Downlink backhaul reception from parent)
    let slot0 = IabTdmSlotFormat::new(
        0,
        IabResourceAvailability::Hard,
        IabResourceAvailability::NotAvailable,
        1, // 1 guard symbol
    )
    .expect("Slot 0 configuration");
    assert!(slot0.is_mt_available_for_backhaul());
    assert!(!slot0.is_du_available_for_transmission());
    assert_eq!(slot0.guard_symbols, 1);

    // Slot 1: DU Hard, MT NotAvailable (Access transmission to child UEs)
    let slot1 = IabTdmSlotFormat::new(
        1,
        IabResourceAvailability::NotAvailable,
        IabResourceAvailability::Hard,
        2, // 2 guard symbols for switching
    )
    .expect("Slot 1 configuration");
    assert!(!slot1.is_mt_available_for_backhaul());
    assert!(slot1.is_du_available_for_transmission());

    // Slot 2: DU Soft, MT Soft (Dynamically scheduled)
    let slot2 = IabTdmSlotFormat::new(
        2,
        IabResourceAvailability::Soft,
        IabResourceAvailability::Soft,
        1,
    )
    .expect("Slot 2 configuration");
    assert!(slot2.is_mt_available_for_backhaul());
    assert!(slot2.is_du_available_for_transmission());

    // Half-Duplex Mutual Exclusion: configuring both MT Hard and DU Hard must fail
    let collision = IabTdmSlotFormat::new(
        3,
        IabResourceAvailability::Hard,
        IabResourceAvailability::Hard,
        1,
    );
    assert!(matches!(
        collision,
        Err(MobileIabError::ResourceCollision { .. })
    ));
}

#[test]
fn test_multi_hop_cumulative_timing_advance() {
    // Chain: IAB-Donor -> Parent IAB (Hop 1) -> Mobile IAB (Hop 2) -> Access UE (Hop 3)
    let mut ta = MultiHopTimingAdvance::new(1_000); // 1000 ns base system offset

    ta.add_hop_propagation_delay_ns(5_000); // Hop 1: 5 µs (approx 1.5 km)
    ta.add_hop_propagation_delay_ns(3_000); // Hop 2: 3 µs (approx 900 m)
    ta.add_hop_propagation_delay_ns(1_500); // Hop 3: 1.5 µs (approx 450 m)

    // Cumulative TA = 2 * (5000 + 3000 + 1500) + 1000 = 2 * 9500 + 1000 = 20,000 ns (20 µs)
    let cumulative_ns = ta.calculate_cumulative_ta_ns();
    assert_eq!(cumulative_ns, 20_000);

    // In 5G NR units (approx 520 ns per step): 20000 / 520 = 38 steps
    let ta_units = ta.to_nr_ta_units();
    assert_eq!(ta_units, 38);
}

#[test]
fn test_mobile_iab_inter_donor_group_handover() {
    let source_addr = BapAddress::new(100).unwrap();
    let mut mobile_node = MobileIabEngine::new(10, true, source_addr, 128);

    // Register two access UEs attached to the Mobile IAB DU
    mobile_node.register_access_bearer(AccessUeBearer {
        ue_id: 0xAAAA,
        drb_id: 1,
        qfi: 9,
        bh_rlc_channel_id: 201,
    });
    mobile_node.register_access_bearer(AccessUeBearer {
        ue_id: 0xBBBB,
        drb_id: 2,
        qfi: 5,
        bh_rlc_channel_id: 202,
    });

    assert_eq!(mobile_node.access_ue_bearers.len(), 2);

    // Configure initial route through Source Donor (Donor 1, Next-Hop 1, Channel 10)
    let dest_server = BapAddress::new(500).unwrap();
    let path_source = BapPathId::new(1).unwrap();
    mobile_node
        .routing_table
        .insert_route(dest_server, path_source, 1, 10, None, None);

    // Normal data routing prior to migration
    let pdu_pre = BapDataPdu::new(dest_server, path_source, vec![0x01, 0x02]);
    let res_pre = mobile_node
        .route_data_pdu(pdu_pre)
        .expect("Route pre-handover");
    assert_eq!(res_pre.next_hop_node_id, 1);
    assert_eq!(res_pre.egress_bh_rlc_channel_id, 10);

    // Step 1: Prepare Inter-Donor Migration to Target Donor (Donor 2, new address 200, path 20)
    let target_addr = BapAddress::new(200).unwrap();
    let path_target = BapPathId::new(20).unwrap();
    mobile_node
        .prepare_inter_donor_migration(2, target_addr, vec![path_target])
        .expect("Prepare migration");

    // Step 2: Execute MT Group Handover at t = 10_000 µs
    mobile_node
        .execute_mt_group_handover(10_000)
        .expect("Execute handover");

    // During MT handover, incoming access UE packets are safely buffered without dropping!
    let pdu_during = BapDataPdu::new(dest_server, path_target, vec![0xAA, 0xBB, 0xCC]);
    let res_during = mobile_node
        .route_data_pdu(pdu_during)
        .expect("Route during handover");
    assert_eq!(res_during.next_hop_node_id, 0); // buffered
    assert_eq!(mobile_node.ingress_buffer.len(), 1);

    // Step 3: Complete MT Handover at t = 14_500 µs (4.5 ms interruption)
    // Target donor routes: Dest 500 via Target Donor 2 (Next-Hop 2, Channel 20)
    let new_routes = vec![(dest_server, path_target, 2, 20)];
    mobile_node
        .complete_mt_group_handover(14_500, new_routes)
        .expect("Complete handover");

    assert_eq!(
        mobile_node.migration_state,
        MobileIabMigrationState::MigrationCompleted
    );
    assert_eq!(mobile_node.metrics.total_group_handovers, 1);

    // Verify buffered packet was automatically drained over target donor path!
    assert_eq!(mobile_node.ingress_buffer.len(), 0);
    assert_eq!(mobile_node.metrics.total_bap_data_pdus_routed, 2);

    // Both access UEs remained attached throughout migration
    assert!(mobile_node.access_ue_bearers.contains_key(&0xAAAA));
    assert!(mobile_node.access_ue_bearers.contains_key(&0xBBBB));
}

#[test]
fn test_flow_control_credit_management_and_buffer_overflow() {
    let addr = BapAddress::new(100).unwrap();
    let mut node = MobileIabEngine::new(5, false, addr, 2); // capacity = 2 packets

    let dest = BapAddress::new(200).unwrap();
    let path = BapPathId::new(1).unwrap();
    node.routing_table
        .insert_route(dest, path, 2, 10, None, None);

    // Set available flow credits to 10 bytes
    node.available_flow_credits_bytes = 10;

    // Send 6-byte packet: succeeds and consumes credits (4 remaining)
    let p1 = BapDataPdu::new(dest, path, vec![1, 2, 3, 4, 5, 6]);
    let res1 = node.route_data_pdu(p1).expect("Route p1");
    assert_eq!(res1.next_hop_node_id, 2);
    assert_eq!(node.available_flow_credits_bytes, 4);

    // Send 8-byte packet: exceeds 4 bytes credit -> buffered in queue (slot 1)
    let p2 = BapDataPdu::new(dest, path, vec![1; 8]);
    let res2 = node.route_data_pdu(p2).expect("Buffer p2");
    assert_eq!(res2.next_hop_node_id, 0);
    assert_eq!(node.ingress_buffer.len(), 1);

    // Send another 8-byte packet: buffered in queue (slot 2 - buffer full)
    let p3 = BapDataPdu::new(dest, path, vec![2; 8]);
    let res3 = node.route_data_pdu(p3).expect("Buffer p3");
    assert_eq!(res3.next_hop_node_id, 0);
    assert_eq!(node.ingress_buffer.len(), 2);

    // Send 4th packet: buffer full -> BufferOverflow error!
    let p4 = BapDataPdu::new(dest, path, vec![3; 8]);
    let err_overflow = node.route_data_pdu(p4);
    assert!(matches!(
        err_overflow,
        Err(MobileIabError::BufferOverflow { .. })
    ));

    // Replenish credits via BAP Control PDU (Flow Control Feedback: 1000 bytes available)
    let ctrl_credit = BapControlPdu::FlowControlFeedbackBhRlc {
        bh_rlc_channel_id: 10,
        available_buffer_bytes: 1000,
    };
    node.process_control_pdu(ctrl_credit)
        .expect("Process credit control PDU");

    // Queue should have drained immediately!
    assert_eq!(node.ingress_buffer.len(), 0);
    assert_eq!(node.metrics.total_bap_data_pdus_routed, 3);
}
