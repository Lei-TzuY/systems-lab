use toy_tcpip::roce::{PfcPauseFrame, RdmaQueuePair, RocePacket, ETHERTYPE_FLOW_CONTROL, IB_OPCODE_RC_ACK, IB_OPCODE_RC_RDMA_WRITE_ONLY, IB_OPCODE_RC_SEND_ONLY, PFC_MULTICAST_MAC, PFC_OPCODE, ROCEV2_UDP_PORT};

#[test]
fn test_roce_send_rdma_write_and_ack() {
    let mut qp_client = RdmaQueuePair::new(1001, 2002, 100);
    let mut qp_server = RdmaQueuePair::new(2002, 1001, 100);

    // 1. Send operation
    let send_pkt = qp_client.send_message(b"High Throughput RDMA Tensor Chunk");
    let raw_send = send_pkt.serialize();

    let parsed_send = RocePacket::parse(&raw_send).unwrap();
    assert_eq!(parsed_send.bth.opcode, IB_OPCODE_RC_SEND_ONLY);
    assert_eq!(parsed_send.bth.dest_qp, 2002);
    assert_eq!(parsed_send.bth.psn, 100);
    assert_eq!(parsed_send.payload, b"High Throughput RDMA Tensor Chunk");
    assert_eq!(ROCEV2_UDP_PORT, 4791);

    let recv_ok = qp_server.receive_packet(&parsed_send);
    assert_eq!(recv_ok, true);
    assert_eq!(qp_server.expected_recv_psn, 101);

    // 2. RDMA Write with RETH header
    let write_pkt = RocePacket::build_rdma_write(2002, 101, 0x7FFF_0000_1000, 0xAABBCCDD, b"Direct Memory Buffer Write");
    let raw_write = write_pkt.serialize();

    let parsed_write = RocePacket::parse(&raw_write).unwrap();
    assert_eq!(parsed_write.bth.opcode, IB_OPCODE_RC_RDMA_WRITE_ONLY);
    let reth = parsed_write.reth.unwrap();
    assert_eq!(reth.virtual_addr, 0x7FFF_0000_1000);
    assert_eq!(reth.rkey, 0xAABBCCDD);
    assert_eq!(reth.dma_len, 26);

    // 3. ACK
    let ack_pkt = RocePacket::build_ack(1001, 101);
    let raw_ack = ack_pkt.serialize();
    let parsed_ack = RocePacket::parse(&raw_ack).unwrap();
    assert_eq!(parsed_ack.bth.opcode, IB_OPCODE_RC_ACK);
    assert_eq!(parsed_ack.bth.dest_qp, 1001);
}

#[test]
fn test_pfc_pause_frame_lossless_ethernet() {
    let pfc = PfcPauseFrame::new(&[3], 65535);
    let raw = pfc.serialize();

    let parsed = PfcPauseFrame::parse(&raw).unwrap();
    assert_eq!(parsed.class_enable_vector, 0b00001000); // Priority 3
    assert_eq!(parsed.pause_times[3], 65535);
    assert_eq!(PFC_MULTICAST_MAC.0, [0x01, 0x80, 0xC2, 0x00, 0x00, 0x01]);
    assert_eq!(ETHERTYPE_FLOW_CONTROL, 0x8808);
    assert_eq!(PFC_OPCODE, 0x0101);
}
