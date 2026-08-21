//! TCP reliability mechanism tests.
//!
//! Covers MSS segmentation of large application writes, congestion and flow control
//! actually governing transmission, fast retransmit on three duplicate ACKs, RFC 6298
//! RTO estimation with Karn's algorithm, sequence-number wraparound across 0xFFFF_FFFF,
//! and hostile / malformed segment handling.

mod common;

use common::{Fixture, payload};
use toy_tcpip::congestion::{CongestionControl, CongestionState, RttEstimator};
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::tcp::{SocketAddrV4, TcpConnection, TcpFlags, TcpSegment, TcpState};
use toy_tcpip::tcp_seq::{seq_diff, seq_ge, seq_gt, seq_le, seq_lt};

// =====================================================================================
// Sequence-number arithmetic and wraparound
// =====================================================================================

#[test]
fn test_serial_number_comparison_across_the_wrap_boundary() {
    let max = 0xFFFF_FFFFu32;
    let next = max.wrapping_add(1); // 0x0000_0000
    let plus5 = max.wrapping_add(6); // 0x0000_0005

    // Naive integer comparison would get every one of these backwards.
    assert!(seq_lt(max, next));
    assert!(seq_le(max, next));
    assert!(seq_gt(next, max));
    assert!(seq_ge(next, max));
    assert!(seq_lt(max, plus5));
    assert_eq!(seq_diff(next, max), 1);
    assert_eq!(seq_diff(plus5, max), 6);

    // Half the sequence space away is "before"; just under half is "after".
    let base = 0x8000_0000u32;
    assert!(seq_gt(base.wrapping_add(0x7FFF_FFFF), base));
    assert!(seq_lt(base.wrapping_sub(1), base));
}

#[test]
fn test_full_transfer_across_sequence_number_wraparound() {
    // The client's ISN is chosen so the stream crosses 0xFFFF_FFFF mid-transfer: the SYN
    // consumes 0xFFFF_FF00, and 4 KiB of data wraps the 32-bit sequence space.
    let mut fx = Fixture::new("lan_wrap", 256);
    let listener = fx.listen(80);
    let client = fx.connect_from(41000, 80, 0xFFFF_FF00);
    let (client, server) = fx.establish_existing(listener, client);

    let data = payload(4_096);
    let got = fx.transfer("client", client, "server", server, &data);

    assert_eq!(got.len(), data.len());
    assert_eq!(got, data, "stream corrupted across the sequence wrap");

    // The send sequence really did wrap past zero.
    let diag = fx
        .lab
        .host("client")
        .unwrap()
        .stack
        .tcp_diagnostics(client)
        .unwrap();
    assert!(
        diag.stats.bytes_sent >= 4_096,
        "expected the full payload to have been sent"
    );
}

#[test]
fn test_out_of_order_reassembly_across_the_wrap_boundary() {
    // Same wrap, but with reordering forced on the link so the reassembly queue has to
    // order segments whose sequence numbers straddle 0xFFFF_FFFF.
    let mut fx = Fixture::new("lan_wrap_ooo", 256);
    let listener = fx.listen(80);
    let client = fx.connect_from(41000, 80, 0xFFFF_FE00);
    let (client, server) = fx.establish_existing(listener, client);

    fx.reorder(6, 9);
    fx.reorder(12, 14);

    let data = payload(8_192);
    let got = fx.transfer("client", client, "server", server, &data);
    assert_eq!(got, data, "reassembly across the wrap produced wrong bytes");
}

// =====================================================================================
// MSS segmentation
// =====================================================================================

#[test]
fn test_large_write_is_segmented_to_the_negotiated_mss() {
    let mss = 512u16;
    let total = 100 * 1024usize; // 100 KiB in a single write() call
    let mut fx = Fixture::new("lan_mss", mss);
    let listener = fx.listen(80);
    let (client, server) = fx.establish(listener, 80);

    let data = payload(total);
    let got = fx.transfer("client", client, "server", server, &data);

    assert_eq!(got.len(), total, "receiver reconstructed the wrong length");
    assert_eq!(got, data, "receiver reconstructed the wrong bytes");

    let stats = fx.stats("client", client);
    let min_segments = total.div_ceil(mss as usize) as u64;
    assert!(
        stats.segments_sent >= min_segments,
        "expected at least {} segments for {} bytes at MSS {}, sent {}",
        min_segments,
        total,
        mss,
        stats.segments_sent
    );

    // And no segment exceeded the MSS: bytes/segment must stay within it.
    let data_segments = stats.segments_sent - stats.retransmissions;
    assert!(
        stats.bytes_sent <= data_segments * mss as u64,
        "some segment exceeded the negotiated MSS of {}",
        mss
    );
}

#[test]
fn test_mss_option_is_honoured_from_the_peer_advertisement() {
    // The server advertises a small MSS; the client must segment to *that* value.
    let mut fx = Fixture::new("lan_mss_opt", 1460);
    fx.lab.host_mut("server").unwrap().stack.set_tcp_mss(300);
    let listener = fx.listen(80);
    let (client, server) = fx.establish(listener, 80);

    let diag = fx
        .lab
        .host("client")
        .unwrap()
        .stack
        .tcp_diagnostics(client)
        .unwrap();
    let _ = diag;

    let data = payload(6_000);
    let got = fx.transfer("client", client, "server", server, &data);
    assert_eq!(got, data);

    let stats = fx.stats("client", client);
    assert!(
        stats.segments_sent >= 6_000 / 300,
        "client ignored the peer's advertised MSS: only {} segments for 6000 bytes",
        stats.segments_sent
    );
}

// =====================================================================================
// Congestion control actually gating transmission
// =====================================================================================

#[test]
fn test_bytes_in_flight_never_exceeds_the_smaller_of_cwnd_and_rwnd() {
    let mss = 512u16;
    let mut fx = Fixture::new("lan_inflight", mss);
    let listener = fx.listen(80);
    let (client, server) = fx.establish(listener, 80);

    let data = payload(64 * 1024);
    fx.write("client", client, &data);

    // Sample the invariant on every simulation round while the receiver drains normally,
    // so the window stays open and the sender is genuinely window-limited rather than
    // blocked behind an application that refuses to read.
    let mut received = Vec::new();
    let mut peak_in_flight = 0u32;
    let mut samples = 0usize;

    // Step the simulation one hop at a time. Sampling only at quiescence would always
    // observe an idle sender; stepping lets us catch the window while it is genuinely full.
    for _ in 0..200_000 {
        fx.lab.pump();

        let d = fx
            .lab
            .host("client")
            .unwrap()
            .stack
            .tcp_diagnostics(client)
            .unwrap();
        let limit = d.cwnd.min(d.send_window as u32);
        assert!(
            d.bytes_in_flight <= limit.max(mss as u32),
            "bytes_in_flight {} exceeded min(cwnd {}, rwnd {})",
            d.bytes_in_flight,
            d.cwnd,
            d.send_window
        );
        peak_in_flight = peak_in_flight.max(d.bytes_in_flight);
        samples += 1;

        let moved = fx.lab.step();
        received.extend(fx.drain("server", server));
        if received.len() >= data.len() {
            break;
        }
        if moved == 0 {
            fx.lab.advance_time(5);
        }
    }

    assert!(
        received.len() >= data.len(),
        "transfer stalled at {} bytes",
        received.len()
    );
    assert!(samples > 0, "the invariant was never sampled");
    assert!(
        peak_in_flight > mss as u32,
        "the sender never had more than one segment outstanding, so the window was never exercised"
    );
    assert_eq!(received, data);
}

#[test]
fn test_slow_start_grows_the_congestion_window_then_avoidance_takes_over() {
    let mut cc = CongestionControl::new(1460);
    cc.ssthresh = 5_840;
    assert_eq!(cc.state, CongestionState::SlowStart);
    let initial = cc.cwnd;

    cc.record_sent(1460);
    cc.on_ack(1460);
    assert!(cc.cwnd > initial, "slow start did not grow cwnd");

    cc.record_sent(1460);
    cc.on_ack(1460);
    assert_eq!(cc.state, CongestionState::CongestionAvoidance);

    // In avoidance the window grows by roughly one MSS per RTT, not per ACK.
    let before = cc.cwnd;
    cc.record_sent(1460);
    cc.on_ack(1460);
    assert!(
        cc.cwnd - before < 1460,
        "congestion avoidance grew cwnd exponentially"
    );
}

#[test]
fn test_timeout_collapses_the_window_and_halves_ssthresh() {
    let mut cc = CongestionControl::new(1000);
    cc.cwnd = 16_000;
    cc.state = CongestionState::CongestionAvoidance;

    cc.on_timeout();
    assert_eq!(cc.cwnd, 1_000, "timeout must collapse cwnd to one MSS");
    assert_eq!(cc.ssthresh, 8_000, "ssthresh must halve on timeout");
    assert_eq!(cc.state, CongestionState::SlowStart);
}

#[test]
fn test_receive_window_shrinks_as_the_application_falls_behind() {
    let mut fx = Fixture::new("lan_rwnd", 512);
    let listener = fx.listen(80);
    let (client, server) = fx.establish(listener, 80);

    let opening = fx
        .lab
        .host("server")
        .unwrap()
        .stack
        .tcp_diagnostics(server)
        .unwrap()
        .receive_window;

    // Push data but never read it, so the receive buffer fills.
    fx.write("client", client, &payload(16 * 1024));
    fx.run_until(25, 60_000, |lab| {
        lab.host("server")
            .unwrap()
            .stack
            .tcp_diagnostics(server)
            .map(|d| d.rx_pending >= 8_192)
            .unwrap_or(false)
    });

    let squeezed = fx
        .lab
        .host("server")
        .unwrap()
        .stack
        .tcp_diagnostics(server)
        .unwrap()
        .receive_window;
    assert!(
        squeezed < opening,
        "advertised window stayed at {} while {} bytes sat unread",
        squeezed,
        8_192
    );

    // Once the application reads, the window reopens and the transfer completes.
    let mut drained = fx.drain("server", server);
    let reopened = fx
        .lab
        .host("server")
        .unwrap()
        .stack
        .tcp_diagnostics(server)
        .unwrap()
        .receive_window;
    assert!(reopened > squeezed, "window did not reopen after reading");

    fx.run_until(25, 120_000, |lab| {
        lab.host("server")
            .unwrap()
            .stack
            .tcp_stats(server)
            .map(|s| s.bytes_received as usize >= 16 * 1024)
            .unwrap_or(false)
    });
    drained.extend(fx.drain("server", server));
    assert_eq!(
        drained.len(),
        16 * 1024,
        "flow-controlled transfer lost bytes"
    );
}

// =====================================================================================
// Fast retransmit
// =====================================================================================

#[test]
fn test_fast_retransmit_after_three_duplicate_acks() {
    // Deterministic hole: five segments go out, the second is dropped, the receiver's
    // cumulative ACKs repeat, and the sender must resend without waiting for the RTO.
    let local = SocketAddrV4 {
        ip: Ipv4Address::new(10, 0, 0, 1),
        port: 40000,
    };
    let remote = SocketAddrV4 {
        ip: Ipv4Address::new(10, 0, 0, 2),
        port: 80,
    };

    let mut sender = TcpConnection::new_client(local, remote, 1000);
    sender.state = TcpState::Established;
    sender.snd_una = 1000;
    sender.snd_nxt = 1000;
    sender.rcv_nxt = 5000;
    sender.snd_wnd = 65535;
    sender.peer_mss = 100;
    sender.congestion.mss = 100;
    sender.congestion.cwnd = 100 * 10;
    // A long RTO guarantees anything we observe is a *fast* retransmit, not a timeout.
    sender.rtt.rto = 100_000.0;

    sender.write(&payload(500));
    let segments = sender.poll_output(0);
    assert_eq!(segments.len(), 5, "expected five 100-byte segments");
    assert_eq!(sender.retransmit_queue.len(), 5);

    // SEG 1 (seq 1000..1100) is delivered; SEG 2 (1100..1200) is dropped; SEG 3, 4, 5
    // arrive and each makes the receiver repeat ACK 1100.
    let ack = |ack_num: u32| {
        TcpSegment::serialize(
            remote.ip,
            local.ip,
            remote.port,
            local.port,
            5000,
            ack_num,
            TcpFlags::ack(),
            65535,
            &[],
        )
    };

    let first = ack(1100);
    let parsed = TcpSegment::parse(remote.ip, local.ip, &first, true).unwrap();
    sender.handle_segment_at(&parsed, 10);
    assert_eq!(sender.snd_una, 1100);
    assert_eq!(sender.retransmit_queue.len(), 4);

    let dup = ack(1100);
    for (i, expected_count) in [(1u32, 1u64), (2, 2)] {
        let parsed = TcpSegment::parse(remote.ip, local.ip, &dup, true).unwrap();
        let resp = sender.handle_segment_at(&parsed, 20 + i as u64);
        assert!(resp.is_none(), "retransmitted too early on dup ACK {}", i);
        assert_eq!(sender.stats.duplicate_acks, expected_count);
    }

    let parsed = TcpSegment::parse(remote.ip, local.ip, &dup, true).unwrap();
    let resp = sender
        .handle_segment_at(&parsed, 30)
        .expect("third duplicate ACK must trigger a fast retransmit");

    let retransmitted = TcpSegment::parse(local.ip, remote.ip, &resp, true).unwrap();
    assert_eq!(
        retransmitted.seq_num, 1100,
        "fast retransmit resent the wrong sequence range"
    );
    assert_eq!(retransmitted.payload.len(), 100);
    assert_eq!(sender.congestion.state, CongestionState::FastRecovery);
    assert_eq!(sender.stats.fast_retransmits, 1);
    assert_eq!(sender.stats.timeouts, 0, "this must not have been an RTO");
}

#[test]
fn test_duplicate_acks_are_ignored_when_nothing_is_outstanding() {
    // A duplicate ACK with an empty retransmission queue is a pure window update and must
    // not trip fast retransmit.
    let local = SocketAddrV4 {
        ip: Ipv4Address::new(10, 0, 0, 1),
        port: 40000,
    };
    let remote = SocketAddrV4 {
        ip: Ipv4Address::new(10, 0, 0, 2),
        port: 80,
    };
    let mut conn = TcpConnection::new_client(local, remote, 1000);
    conn.state = TcpState::Established;
    conn.snd_una = 1000;
    conn.snd_nxt = 1000;
    conn.rcv_nxt = 5000;

    let raw = TcpSegment::serialize(
        remote.ip,
        local.ip,
        remote.port,
        local.port,
        5000,
        1000,
        TcpFlags::ack(),
        65535,
        &[],
    );
    for _ in 0..5 {
        let seg = TcpSegment::parse(remote.ip, local.ip, &raw, true).unwrap();
        assert!(conn.handle_segment_at(&seg, 10).is_none());
    }
    assert_eq!(conn.stats.duplicate_acks, 0);
    assert_eq!(conn.stats.fast_retransmits, 0);
}

// =====================================================================================
// RTO estimation (RFC 6298) and Karn's algorithm
// =====================================================================================

#[test]
fn test_rfc6298_rto_computation_and_backoff() {
    let mut rtt = RttEstimator::new();
    assert_eq!(rtt.rto, 1000.0, "initial RTO must be 1 second");

    // First sample: SRTT = R, RTTVAR = R/2, RTO = SRTT + 4*RTTVAR.
    rtt.update_sample(100.0);
    assert_eq!(rtt.srtt, Some(100.0));
    assert_eq!(rtt.rttvar, Some(50.0));
    assert_eq!(rtt.rto, 300.0);

    // Subsequent samples smooth with alpha = 1/8, beta = 1/4.
    rtt.update_sample(120.0);
    let srtt = rtt.srtt.unwrap();
    assert!((srtt - 102.5).abs() < 0.001, "SRTT was {}", srtt);
    let rttvar = rtt.rttvar.unwrap();
    assert!((rttvar - 42.5).abs() < 0.001, "RTTVAR was {}", rttvar);

    // Backoff doubles, and stays clamped at the ceiling.
    let before = rtt.rto;
    rtt.backoff();
    assert_eq!(rtt.rto, before * 2.0);
    for _ in 0..40 {
        rtt.backoff();
    }
    assert_eq!(rtt.rto, rtt.max_rto, "backoff must clamp at max_rto");
}

#[test]
fn test_karns_algorithm_rejects_ambiguous_samples_from_retransmissions() {
    // A segment that had to be retransmitted must not feed the RTT estimator: its ACK
    // cannot be attributed to a particular transmission.
    let mut fx = Fixture::new("lan_karn", 256);
    let listener = fx.listen(80);
    let (client, server) = fx.establish(listener, 80);

    let baseline = fx
        .lab
        .host("client")
        .unwrap()
        .stack
        .tcp_diagnostics(client)
        .unwrap();
    let srtt_before = baseline.srtt_ms.expect("handshake produced an RTT sample");

    // Drop several data frames so retransmission is forced.
    fx.drop_frames(&[6, 7, 8, 9, 10]);

    let data = payload(8_192);
    let got = fx.transfer("client", client, "server", server, &data);
    assert_eq!(got, data);

    let after = fx
        .lab
        .host("client")
        .unwrap()
        .stack
        .tcp_diagnostics(client)
        .unwrap();
    assert!(
        after.stats.retransmissions > 0,
        "the scenario did not actually force a retransmission"
    );
    // With every RTT in the lab measuring as the same simulated interval, a Karn
    // violation would show up as SRTT inflating toward the backed-off RTO.
    let srtt_after = after.srtt_ms.unwrap();
    assert!(
        srtt_after < srtt_before.max(1.0) + after.rto_ms,
        "SRTT {} looks like it absorbed an ambiguous retransmission sample",
        srtt_after
    );
}

// =====================================================================================
// Hostile and malformed input
// =====================================================================================

#[test]
fn test_ack_of_unsent_data_is_rejected() {
    let local = SocketAddrV4 {
        ip: Ipv4Address::new(10, 0, 0, 1),
        port: 40000,
    };
    let remote = SocketAddrV4 {
        ip: Ipv4Address::new(10, 0, 0, 2),
        port: 80,
    };
    let mut conn = TcpConnection::new_client(local, remote, 1000);
    conn.state = TcpState::Established;
    conn.snd_una = 1000;
    conn.snd_nxt = 1000;
    conn.rcv_nxt = 5000;

    let raw = TcpSegment::serialize(
        remote.ip,
        local.ip,
        remote.port,
        local.port,
        5000,
        999_999, // acknowledges bytes we never sent
        TcpFlags::ack(),
        65535,
        &[],
    );
    let seg = TcpSegment::parse(remote.ip, local.ip, &raw, true).unwrap();
    conn.handle_segment_at(&seg, 10);

    assert_eq!(
        conn.snd_una, 1000,
        "snd_una advanced on an ACK of unsent data"
    );
    assert_eq!(conn.stats.invalid_segments, 1);
    assert_eq!(conn.state, TcpState::Established);
}

#[test]
fn test_off_window_reset_does_not_tear_down_the_connection() {
    let local = SocketAddrV4 {
        ip: Ipv4Address::new(10, 0, 0, 1),
        port: 40000,
    };
    let remote = SocketAddrV4 {
        ip: Ipv4Address::new(10, 0, 0, 2),
        port: 80,
    };
    let mut conn = TcpConnection::new_client(local, remote, 1000);
    conn.state = TcpState::Established;
    conn.snd_una = 1000;
    conn.snd_nxt = 1000;
    conn.rcv_nxt = 5000;

    // A blind reset far outside the receive window must be ignored (RFC 5961).
    let raw = TcpSegment::serialize(
        remote.ip,
        local.ip,
        remote.port,
        local.port,
        999_999,
        0,
        TcpFlags::rst(),
        0,
        &[],
    );
    let seg = TcpSegment::parse(remote.ip, local.ip, &raw, true).unwrap();
    conn.handle_segment_at(&seg, 10);
    assert_eq!(
        conn.state,
        TcpState::Established,
        "off-window RST killed the connection"
    );
    assert_eq!(conn.stats.invalid_segments, 1);

    // An in-window reset does close it.
    let raw = TcpSegment::serialize(
        remote.ip,
        local.ip,
        remote.port,
        local.port,
        5000,
        0,
        TcpFlags::rst(),
        0,
        &[],
    );
    let seg = TcpSegment::parse(remote.ip, local.ip, &raw, true).unwrap();
    conn.handle_segment_at(&seg, 20);
    assert_eq!(conn.state, TcpState::Closed);
}

#[test]
fn test_malformed_and_truncated_segments_never_panic() {
    let src = Ipv4Address::new(10, 0, 0, 1);
    let dst = Ipv4Address::new(10, 0, 0, 2);

    // Truncated at every length below a minimal header, plus junk and a bad checksum.
    let good = TcpSegment::serialize(src, dst, 1234, 80, 1, 1, TcpFlags::ack(), 65535, b"data");
    for len in 0..good.len() {
        let _ = TcpSegment::parse(src, dst, &good[..len], true);
    }

    let mut corrupt = good.clone();
    corrupt[16] ^= 0xFF; // wreck the checksum
    assert!(
        TcpSegment::parse(src, dst, &corrupt, true).is_err(),
        "a corrupted checksum was accepted"
    );

    // A data offset claiming more header than the segment contains.
    let mut bad_offset = good.clone();
    bad_offset[12] = 0xF0; // data offset = 15 words = 60 bytes
    let _ = TcpSegment::parse(src, dst, &bad_offset, true);

    // Random bytes of assorted lengths.
    for len in [1usize, 5, 19, 20, 21, 40, 100] {
        let junk: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(37)).collect();
        let _ = TcpSegment::parse(src, dst, &junk, true);
    }
}

#[test]
fn test_stack_survives_a_flood_of_garbage_frames() {
    // Feed the stack malformed Ethernet/IPv4/TCP bytes; it must stay usable afterwards.
    let mut fx = Fixture::new("lan_fuzz", 512);
    let listener = fx.listen(80);

    for seed in 0..400u32 {
        let len = 14 + (seed as usize % 90);
        let frame: Vec<u8> = (0..len)
            .map(|i| ((seed.wrapping_mul(2_654_435_761).wrapping_add(i as u32)) >> 16) as u8)
            .collect();
        let _ = fx
            .lab
            .host_mut("server")
            .unwrap()
            .stack
            .process_frame(&frame);
    }

    // The stack still completes a normal connection and transfer.
    let (client, server) = fx.establish(listener, 80);
    let data = payload(2_048);
    assert_eq!(fx.transfer("client", client, "server", server, &data), data);
}

#[test]
fn test_out_of_order_queue_is_bounded_under_adversarial_reordering() {
    let local = SocketAddrV4 {
        ip: Ipv4Address::new(10, 0, 0, 1),
        port: 40000,
    };
    let remote = SocketAddrV4 {
        ip: Ipv4Address::new(10, 0, 0, 2),
        port: 80,
    };
    let mut conn = TcpConnection::new_server(local, remote, 1000);
    conn.state = TcpState::Established;
    conn.snd_una = 1000;
    conn.snd_nxt = 1000;
    conn.rcv_nxt = 5000;

    // Never send the segment that would fill the hole, so nothing can ever be delivered.
    let block = vec![0xABu8; 1_000];
    for i in 1..2_000u32 {
        let seq = 5000u32.wrapping_add(i * 1_000);
        let raw = TcpSegment::serialize(
            remote.ip,
            local.ip,
            remote.port,
            local.port,
            seq,
            1000,
            TcpFlags::ack(),
            65535,
            &block,
        );
        let seg = TcpSegment::parse(remote.ip, local.ip, &raw, true).unwrap();
        conn.handle_segment_at(&seg, i as u64);
    }

    assert!(
        conn.ooo_bytes() <= toy_tcpip::tcp::MAX_OOO_BYTES,
        "out-of-order queue grew to {} bytes, past the {} byte cap",
        conn.ooo_bytes(),
        toy_tcpip::tcp::MAX_OOO_BYTES
    );
    assert_eq!(conn.rcv_nxt, 5000, "nothing should have been delivered");
    assert!(conn.rx_buffer.is_empty());
}

#[test]
fn test_retransmission_gives_up_instead_of_looping_forever() {
    // A peer that never answers must not keep the sender retransmitting indefinitely.
    let mut fx = Fixture::new("lan_blackhole", 512);
    let listener = fx.listen(80);
    let client = fx.connect(80);

    // Black-hole every frame on the link.
    fx.drop_frames(&(1..4_000).collect::<Vec<usize>>());

    let gave_up = fx.run_until(500, 3_000_000, |lab| {
        lab.host("client")
            .unwrap()
            .stack
            .tcp_state(client)
            .map(|s| s == TcpState::Closed)
            .unwrap_or(false)
    });

    assert!(
        gave_up,
        "the client retransmitted forever instead of aborting"
    );
    let _ = listener;
}

#[test]
fn test_send_buffer_is_bounded_and_reports_short_writes() {
    // A single oversized write must not grow the send buffer without limit.
    let mut fx = Fixture::new("lan_sndbuf", 512);
    let listener = fx.listen(80);
    let (client, server) = fx.establish(listener, 80);

    let huge = payload(toy_tcpip::tcp::SND_BUFFER_CAPACITY * 2);
    let accepted = fx
        .lab
        .host_mut("client")
        .unwrap()
        .stack
        .tcp_write(client, &huge)
        .expect("first write should be accepted in part");

    assert!(
        accepted <= toy_tcpip::tcp::SND_BUFFER_CAPACITY,
        "accepted {} bytes into a {} byte send buffer",
        accepted,
        toy_tcpip::tcp::SND_BUFFER_CAPACITY
    );
    assert!(accepted > 0, "no bytes were accepted at all");

    // The bytes that were accepted still arrive intact.
    let mut received = Vec::new();
    for _ in 0..20_000 {
        fx.lab.run_pumped(5);
        received.extend(fx.drain("server", server));
        if received.len() >= accepted {
            break;
        }
        fx.lab.advance_time(25);
    }
    assert_eq!(received.len(), accepted);
    assert_eq!(received, huge[..accepted]);
}

#[test]
fn test_write_after_close_is_refused() {
    let mut fx = Fixture::new("lan_write_after_close", 512);
    let listener = fx.listen(80);
    let (client, _server) = fx.establish(listener, 80);

    fx.close("client", client);
    let err = fx
        .lab
        .host_mut("client")
        .unwrap()
        .stack
        .tcp_write(client, b"too late")
        .unwrap_err();
    assert_eq!(err, toy_tcpip::socket::SocketError::NotConnected);
}

#[test]
fn test_data_piggybacked_on_the_handshake_ack_is_sequence_checked() {
    // A final handshake ACK carrying data at the wrong sequence number must not be
    // spliced into the receive stream.
    let local = SocketAddrV4 {
        ip: Ipv4Address::new(10, 0, 0, 2),
        port: 80,
    };
    let remote = SocketAddrV4 {
        ip: Ipv4Address::new(10, 0, 0, 1),
        port: 40000,
    };

    let mut server = TcpConnection::new_server(local, remote, 5000);

    // Client SYN.
    let syn = TcpSegment::serialize(
        remote.ip,
        local.ip,
        remote.port,
        local.port,
        1000,
        0,
        TcpFlags::syn(),
        65535,
        &[],
    );
    let syn_seg = TcpSegment::parse(remote.ip, local.ip, &syn, true).unwrap();
    server.handle_segment_at(&syn_seg, 0).expect("SYN-ACK");
    assert_eq!(server.rcv_nxt, 1001);

    // Final ACK carrying data at a sequence far past what we expect.
    let mut flags = TcpFlags::ack();
    flags.psh = true;
    let bogus = TcpSegment::serialize(
        remote.ip,
        local.ip,
        remote.port,
        local.port,
        900_000,
        5001,
        flags,
        65535,
        b"out of window",
    );
    let bogus_seg = TcpSegment::parse(remote.ip, local.ip, &bogus, true).unwrap();
    server.handle_segment_at(&bogus_seg, 10);

    assert_eq!(server.state, TcpState::Established);
    assert!(
        server.rx_buffer.is_empty(),
        "out-of-window data on the handshake ACK reached the application"
    );
    assert_eq!(server.rcv_nxt, 1001, "rcv_nxt advanced on bogus data");
}
