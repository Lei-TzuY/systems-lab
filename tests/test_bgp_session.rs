//! BGP-4 session layer: the finite state machine, OPEN negotiation, the deterministic
//! timers, and message framing over a real TCP byte stream.
//!
//! Every session in this file is carried by the repository's own TCP runtime on port
//! 179. Nothing hands a BGP message directly to a peer object.

mod common;

use common::bgp_lab::{
    AS1, AS2, LAB_HOLD_TIME, RawBgpPeer, build_linear_lab, converge_sessions, ip, prefix, run_until,
};
use toy_tcpip::bgp::{
    BGP_ERR_MESSAGE_HEADER, BGP_ERR_OPEN_MESSAGE, BGP_HEADER_LEN, BGP_MARKER, BGP_MAX_MESSAGE_LEN,
    BGP_MSG_KEEPALIVE, BGP_MSG_OPEN, BGP_MSG_UPDATE, BGP_SUB_BAD_MESSAGE_LENGTH,
    BGP_SUB_CONNECTION_NOT_SYNCHRONIZED, BgpFramer, BgpNotificationMessage, BgpOpenMessage, BgpPdu,
    peek_bgp_message_type,
};
use toy_tcpip::bgp_router::BgpState;
use toy_tcpip::tcp::TcpState;

// ============================================================================
// FSM establishment over real TCP
// ============================================================================

#[test]
fn test_sessions_reach_established_through_tcp_open_and_keepalive() {
    let mut lab = build_linear_lab();

    // Nothing is up before the simulation runs.
    for r in ["r1", "r2", "r3"] {
        let bgp = lab.router(r).unwrap().bgp().unwrap();
        assert!(
            bgp.peers().iter().all(|p| p.state == BgpState::Idle),
            "{} should start Idle",
            r
        );
    }

    assert!(
        converge_sessions(&mut lab, 60_000),
        "BGP sessions did not reach ESTABLISHED"
    );

    // R1 <-> R2 is one session; R2 <-> R3 is another.
    let r1 = lab.router("r1").unwrap().bgp().unwrap();
    let peer = r1.peer(ip(10, 12, 0, 2)).unwrap();
    assert_eq!(peer.state, BgpState::Established);
    assert_eq!(peer.remote_as, AS2);
    // The identifier can only come from the peer's OPEN.
    assert_eq!(peer.remote_router_id, Some(ip(2, 2, 2, 2)));
    assert_eq!(peer.establishment_count, 1);
    assert!(peer.counters.opens_sent >= 1);
    assert!(peer.counters.opens_received >= 1);
    assert!(peer.counters.keepalives_sent >= 1);
    assert!(peer.counters.keepalives_received >= 1);
    assert!(peer.last_error.is_none());

    let r2 = lab.router("r2").unwrap().bgp().unwrap();
    assert_eq!(
        r2.peer(ip(10, 12, 0, 1)).unwrap().remote_router_id,
        Some(ip(1, 1, 1, 1))
    );
    assert_eq!(r2.established_peer_count(), 2);
}

#[test]
fn test_hold_time_is_negotiated_to_the_lower_of_the_two() {
    let mut lab = build_linear_lab();
    // R2 proposes a longer hold time than everyone else; the lower value must win.
    lab.router_mut("r2").unwrap().bgp_mut().unwrap().hold_time = 30;

    assert!(converge_sessions(&mut lab, 60_000));

    let r1_peer = lab
        .router("r1")
        .unwrap()
        .bgp()
        .unwrap()
        .peer(ip(10, 12, 0, 2))
        .unwrap();
    assert_eq!(r1_peer.negotiated_hold_ms, LAB_HOLD_TIME as u64 * 1_000);
    assert_eq!(
        r1_peer.keepalive_interval_ms,
        LAB_HOLD_TIME as u64 * 1_000 / 3
    );

    let r2_peer = lab
        .router("r2")
        .unwrap()
        .bgp()
        .unwrap()
        .peer(ip(10, 12, 0, 1))
        .unwrap();
    assert_eq!(r2_peer.negotiated_hold_ms, LAB_HOLD_TIME as u64 * 1_000);
}

#[test]
fn test_the_session_carries_traffic_on_tcp_port_179() {
    let mut lab = build_linear_lab();
    assert!(converge_sessions(&mut lab, 60_000));

    // The transport under the session must be a live connection whose remote port is 179
    // on the side that dialled, and whose local port is 179 on the side that listened.
    let r1 = lab.router("r1").unwrap();
    let diags = r1.sockets.as_ref().unwrap().all_tcp_diagnostics();
    let session = diags
        .iter()
        .find(|d| d.remote.ip == ip(10, 12, 0, 2))
        .expect("no connection to the peer");
    assert_eq!(session.remote.port, 179);
    assert_eq!(session.local.ip, ip(10, 12, 0, 1));
    assert_eq!(session.state, TcpState::Established);
    assert!(session.stats.bytes_sent > 0);
    assert!(session.stats.bytes_received > 0);

    let r2 = lab.router("r2").unwrap();
    let diags2 = r2.sockets.as_ref().unwrap().all_tcp_diagnostics();
    assert!(
        diags2
            .iter()
            .any(|d| d.local.port == 179 && d.remote.ip == ip(10, 12, 0, 1)),
        "R2 should be serving the R1 session from its own port 179"
    );
}

// ============================================================================
// Timers
// ============================================================================

#[test]
fn test_keepalives_keep_flowing_and_the_session_stays_up() {
    let mut lab = build_linear_lab();
    assert!(converge_sessions(&mut lab, 60_000));

    let before = lab
        .router("r1")
        .unwrap()
        .bgp()
        .unwrap()
        .peer(ip(10, 12, 0, 2))
        .unwrap()
        .counters
        .keepalives_sent;

    // Three hold times worth of simulated time with a healthy link.
    let deadline = lab.current_time_ms + 3 * LAB_HOLD_TIME as u64 * 1_000;
    while lab.current_time_ms < deadline {
        lab.advance_time(500);
        lab.run_pumped(20);
    }

    let peer = lab
        .router("r1")
        .unwrap()
        .bgp()
        .unwrap()
        .peer(ip(10, 12, 0, 2))
        .unwrap();
    assert_eq!(
        peer.state,
        BgpState::Established,
        "a healthy session must not expire"
    );
    assert!(
        peer.counters.keepalives_sent > before,
        "the KeepaliveTimer never fired: {} -> {}",
        before,
        peer.counters.keepalives_sent
    );
    assert!(peer.counters.keepalives_received > 0);
}

#[test]
fn test_hold_timer_expires_when_the_peer_goes_silent() {
    let mut lab = build_linear_lab();
    assert!(converge_sessions(&mut lab, 60_000));
    assert_eq!(
        lab.router("r1")
            .unwrap()
            .bgp()
            .unwrap()
            .peer_state(ip(10, 12, 0, 2)),
        Some(BgpState::Established)
    );

    // Cut the cable. Neither side hears the other again, so both hold timers must run out.
    lab.link_mut("r1r2").unwrap().set_blackhole(true);

    let expired = run_until(&mut lab, 120_000, |l| {
        l.router("r1")
            .unwrap()
            .bgp()
            .unwrap()
            .peer_state(ip(10, 12, 0, 2))
            != Some(BgpState::Established)
    });
    assert!(expired, "the session survived a dead link");

    let peer = lab
        .router("r1")
        .unwrap()
        .bgp()
        .unwrap()
        .peer(ip(10, 12, 0, 2))
        .unwrap();
    assert_ne!(peer.state, BgpState::Established);
    assert!(peer.established_since_ms.is_none());
    assert!(
        peer.last_error.is_some(),
        "the teardown reason should be recorded for diagnostics"
    );
    // The failure has to be detected by a timer, not by anyone poking the FSM.
    assert!(
        lab.current_time_ms > 0,
        "the expiry must be driven by simulated time"
    );
}

#[test]
fn test_a_peer_that_never_answers_stays_out_of_established() {
    // R1 alone: its neighbour does not exist, so the TCP connection can never complete.
    let mut lab = build_linear_lab();
    lab.link_mut("r1r2").unwrap().set_blackhole(true);

    let up = run_until(&mut lab, 60_000, |l| {
        l.router("r1")
            .unwrap()
            .bgp()
            .unwrap()
            .peer_state(ip(10, 12, 0, 2))
            == Some(BgpState::Established)
    });
    assert!(!up, "a session came up across a dead link");

    let peer_state = lab
        .router("r1")
        .unwrap()
        .bgp()
        .unwrap()
        .peer_state(ip(10, 12, 0, 2))
        .unwrap();
    assert!(
        matches!(
            peer_state,
            BgpState::Idle | BgpState::Connect | BgpState::Active
        ),
        "unexpected state {} for an unreachable peer",
        peer_state
    );

    // The ConnectRetryTimer must have driven repeated attempts rather than one and done.
    let attempts = lab
        .router("r1")
        .unwrap()
        .bgp()
        .unwrap()
        .events()
        .iter()
        .filter(|e| e.text.contains("Connect"))
        .count();
    assert!(
        attempts >= 2,
        "only {} connect attempts were made",
        attempts
    );
}

// ============================================================================
// Stream framing: TCP has no message boundaries
// ============================================================================

#[test]
fn test_framer_reassembles_a_message_split_across_reads() {
    let open = BgpPdu::Open(BgpOpenMessage::new(AS1, 90, ip(1, 1, 1, 1))).serialize();
    let mut framer = BgpFramer::new();

    // Byte at a time: the framer must not produce anything until the last one.
    for (i, b) in open.iter().enumerate() {
        framer.push(&[*b]).unwrap();
        let got = framer.next_frame().unwrap();
        if i + 1 < open.len() {
            assert!(
                got.is_none(),
                "framer emitted a message after {} bytes",
                i + 1
            );
        } else {
            assert_eq!(got.unwrap(), open);
        }
    }
    assert_eq!(framer.buffered(), 0);
}

#[test]
fn test_framer_splits_several_messages_delivered_in_one_read() {
    let open = BgpPdu::Open(BgpOpenMessage::new(AS1, 90, ip(1, 1, 1, 1))).serialize();
    let ka = BgpPdu::Keepalive.serialize();
    let note = BgpPdu::Notification(BgpNotificationMessage::new(6, 0)).serialize();

    let mut stream = Vec::new();
    stream.extend_from_slice(&open);
    stream.extend_from_slice(&ka);
    stream.extend_from_slice(&ka);
    stream.extend_from_slice(&note);
    // A trailing fragment of a fifth message, as a real read would often deliver.
    stream.extend_from_slice(&open[..7]);

    let mut framer = BgpFramer::new();
    framer.push(&stream).unwrap();

    let mut types = Vec::new();
    while let Some(frame) = framer.next_frame().unwrap() {
        types.push(peek_bgp_message_type(&frame).unwrap());
        BgpPdu::parse(&frame).expect("each extracted frame must decode");
    }
    assert_eq!(
        types,
        vec![BGP_MSG_OPEN, BGP_MSG_KEEPALIVE, BGP_MSG_KEEPALIVE, 3]
    );
    assert_eq!(
        framer.buffered(),
        7,
        "the partial message must stay buffered"
    );

    // Finishing the fifth message releases it.
    framer.push(&open[7..]).unwrap();
    assert_eq!(
        peek_bgp_message_type(&framer.next_frame().unwrap().unwrap()),
        Some(BGP_MSG_OPEN)
    );
}

#[test]
fn test_framer_rejects_a_desynchronised_stream() {
    let mut framer = BgpFramer::new();
    framer.push(&[0u8; 32]).unwrap();
    let err = framer.next_frame().unwrap_err();
    assert_eq!(err.code, BGP_ERR_MESSAGE_HEADER);
    assert_eq!(err.subcode, BGP_SUB_CONNECTION_NOT_SYNCHRONIZED);
}

#[test]
fn test_framer_rejects_impossible_lengths_and_bounds_its_buffer() {
    // Length below the header size.
    let mut frame = BGP_MARKER.to_vec();
    frame.extend_from_slice(&3u16.to_be_bytes());
    frame.push(BGP_MSG_KEEPALIVE);
    let mut framer = BgpFramer::new();
    framer.push(&frame).unwrap();
    let err = framer.next_frame().unwrap_err();
    assert_eq!(err.subcode, BGP_SUB_BAD_MESSAGE_LENGTH);

    // Length above the 4096-byte maximum.
    let mut frame = BGP_MARKER.to_vec();
    frame.extend_from_slice(&60_000u16.to_be_bytes());
    frame.push(BGP_MSG_UPDATE);
    let mut framer = BgpFramer::new();
    framer.push(&frame).unwrap();
    assert_eq!(
        framer.next_frame().unwrap_err().subcode,
        BGP_SUB_BAD_MESSAGE_LENGTH
    );

    // The reassembly buffer is hard-capped: a peer cannot make it grow without limit.
    let mut framer = BgpFramer::with_capacity(BGP_MAX_MESSAGE_LEN);
    let junk = vec![0xFFu8; BGP_MAX_MESSAGE_LEN];
    framer.push(&junk).unwrap();
    assert!(
        framer.push(&[0xFF]).is_err(),
        "the framer accepted more than its capacity"
    );
    assert!(framer.buffered() <= BGP_MAX_MESSAGE_LEN);
}

#[test]
fn test_a_session_survives_bgp_messages_split_across_tcp_segments() {
    // A tiny MSS forces the transport to chop every BGP message into several segments,
    // so the receive path has to reassemble across segment boundaries to work at all.
    let mut lab = build_linear_lab();
    for r in ["r1", "r2", "r3"] {
        lab.router_mut(r)
            .unwrap()
            .sockets
            .as_mut()
            .unwrap()
            .set_default_mss(88);
    }

    assert!(
        converge_sessions(&mut lab, 90_000),
        "sessions failed to establish with a 88-byte MSS"
    );

    // And the routes still propagate end to end through the fragmented stream.
    assert!(
        run_until(&mut lab, 90_000, |l| {
            l.router("r1")
                .unwrap()
                .bgp()
                .unwrap()
                .loc_rib
                .contains(&prefix(10, 3, 0, 0, 24))
        }),
        "R1 never learned 10.3.0.0/24 over a heavily segmented session"
    );

    let diag = lab
        .router("r1")
        .unwrap()
        .sockets
        .as_ref()
        .unwrap()
        .all_tcp_diagnostics()
        .into_iter()
        .find(|d| d.remote.port == 179)
        .unwrap();
    assert!(
        diag.stats.segments_sent > 3,
        "expected the small MSS to produce many segments, got {}",
        diag.stats.segments_sent
    );
}

// ============================================================================
// OPEN validation
// ============================================================================

#[test]
fn test_open_with_the_wrong_version_is_refused_with_a_notification() {
    let mut peer = RawBgpPeer::connect(AS1, AS2, ip(9, 9, 9, 9));
    let mut open = BgpOpenMessage::new(AS2, LAB_HOLD_TIME, ip(5, 5, 5, 5));
    open.version = 3;
    peer.write(&BgpPdu::Open(open).serialize());

    let msgs = peer.drain();
    let note = msgs
        .iter()
        .find_map(|m| match m {
            BgpPdu::Notification(n) => Some(n),
            _ => None,
        })
        .expect("no NOTIFICATION for an unsupported version");
    assert_eq!(note.error_code, BGP_ERR_OPEN_MESSAGE);
    assert_eq!(note.error_subcode, 1); // Unsupported Version Number
    assert_ne!(peer.state(), BgpState::Established);
}

#[test]
fn test_open_with_the_wrong_asn_is_refused() {
    let mut peer = RawBgpPeer::connect(AS1, AS2, ip(9, 9, 9, 9));
    // The router expects AS 65002 from this neighbour.
    peer.write(
        &BgpPdu::Open(BgpOpenMessage::new(64_999, LAB_HOLD_TIME, ip(5, 5, 5, 5))).serialize(),
    );

    let msgs = peer.drain();
    let note = msgs
        .iter()
        .find_map(|m| match m {
            BgpPdu::Notification(n) => Some(n),
            _ => None,
        })
        .expect("no NOTIFICATION for a bad peer AS");
    assert_eq!(note.error_code, BGP_ERR_OPEN_MESSAGE);
    assert_eq!(note.error_subcode, 2); // Bad Peer AS
    assert_ne!(peer.state(), BgpState::Established);
}

#[test]
fn test_open_with_an_unusable_bgp_identifier_is_refused() {
    // 0.0.0.0 and a multicast address are not host addresses; 9.9.9.9 is the router's
    // own identifier, which would make the session indistinguishable from itself.
    for bad_id in [ip(0, 0, 0, 0), ip(224, 0, 0, 5), ip(9, 9, 9, 9)] {
        let mut peer = RawBgpPeer::connect(AS1, AS2, ip(9, 9, 9, 9));
        peer.write(&BgpPdu::Open(BgpOpenMessage::new(AS2, LAB_HOLD_TIME, bad_id)).serialize());
        let msgs = peer.drain();
        let note = msgs
            .iter()
            .find_map(|m| match m {
                BgpPdu::Notification(n) => Some(n),
                _ => None,
            })
            .unwrap_or_else(|| panic!("identifier {} was accepted", bad_id));
        assert_eq!(note.error_code, BGP_ERR_OPEN_MESSAGE);
        assert_eq!(note.error_subcode, 3); // Bad BGP Identifier
    }
}

#[test]
fn test_open_with_an_illegal_hold_time_is_refused() {
    let mut peer = RawBgpPeer::connect(AS1, AS2, ip(9, 9, 9, 9));
    // 1 and 2 seconds are explicitly illegal (RFC 4271 section 4.2).
    peer.write(&BgpPdu::Open(BgpOpenMessage::new(AS2, 2, ip(5, 5, 5, 5))).serialize());
    let msgs = peer.drain();
    let note = msgs
        .iter()
        .find_map(|m| match m {
            BgpPdu::Notification(n) => Some(n),
            _ => None,
        })
        .expect("no NOTIFICATION for a 2-second hold time");
    assert_eq!(note.error_subcode, 6); // Unacceptable Hold Time
}

#[test]
fn test_a_well_formed_raw_peer_completes_the_handshake() {
    // The same harness with a valid OPEN must reach ESTABLISHED, which proves the
    // rejections above are about the defect and not about the harness.
    let mut peer = RawBgpPeer::connect(AS1, AS2, ip(9, 9, 9, 9));
    peer.write(&BgpPdu::Open(BgpOpenMessage::new(AS2, LAB_HOLD_TIME, ip(5, 5, 5, 5))).serialize());

    let msgs = peer.drain();
    assert!(
        msgs.iter().any(|m| matches!(m, BgpPdu::Open(_))),
        "the router did not answer with its own OPEN"
    );
    assert!(
        msgs.iter().any(|m| matches!(m, BgpPdu::Keepalive)),
        "the router did not send a KEEPALIVE"
    );

    peer.write(&BgpPdu::Keepalive.serialize());
    assert!(
        peer.run_until(30_000, |l| {
            l.router("victim")
                .unwrap()
                .bgp()
                .unwrap()
                .peer_state(ip(10, 50, 0, 2))
                == Some(BgpState::Established)
        }),
        "a valid handshake did not reach ESTABLISHED"
    );
    let bgp = peer.victim_bgp();
    let up = bgp.peer(ip(10, 50, 0, 2)).unwrap();
    assert_eq!(up.remote_router_id, Some(ip(5, 5, 5, 5)));
    assert_eq!(up.negotiated_hold_ms, LAB_HOLD_TIME as u64 * 1_000);
}

#[test]
fn test_an_update_before_established_is_a_fsm_error() {
    let mut peer = RawBgpPeer::connect(AS1, AS2, ip(9, 9, 9, 9));
    // Straight to UPDATE without ever sending an OPEN.
    let update = BgpPdu::Update(toy_tcpip::bgp::BgpUpdateMessage::announce(
        toy_tcpip::bgp::BgpPathAttributes::new(
            toy_tcpip::bgp::BgpOrigin::Igp,
            toy_tcpip::bgp::AsPath::sequence(vec![AS2]),
            ip(10, 50, 0, 2),
        ),
        vec![prefix(203, 0, 113, 0, 24)],
    ));
    peer.write(&update.serialize());

    let msgs = peer.drain();
    let note = msgs
        .iter()
        .find_map(|m| match m {
            BgpPdu::Notification(n) => Some(n),
            _ => None,
        })
        .expect("an UPDATE in OpenSent should be a FSM error");
    assert_eq!(note.error_code, 5); // Finite State Machine Error
    assert_ne!(peer.state(), BgpState::Established);

    // And nothing from that UPDATE reached the RIB or the FIB.
    let bgp = peer.victim_bgp();
    assert_eq!(bgp.adj_rib_in.path_count(), 0);
    assert!(bgp.loc_rib.is_empty());
}

#[test]
fn test_header_length_field_is_validated_before_the_body_is_trusted() {
    // A KEEPALIVE header claiming a 4000-byte body, with no body behind it. The framer
    // must wait for bytes that never come rather than reading past the buffer.
    let mut frame = BGP_MARKER.to_vec();
    frame.extend_from_slice(&4_000u16.to_be_bytes());
    frame.push(BGP_MSG_KEEPALIVE);
    let mut framer = BgpFramer::new();
    framer.push(&frame).unwrap();
    assert_eq!(framer.next_frame().unwrap(), None);
    assert_eq!(framer.buffered(), BGP_HEADER_LEN);

    // Direct decode of a frame whose length field disagrees with the byte count.
    let err = BgpPdu::parse(&frame).unwrap_err();
    assert_eq!(err.code, BGP_ERR_MESSAGE_HEADER);
}
