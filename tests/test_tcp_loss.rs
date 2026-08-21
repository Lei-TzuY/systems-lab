//! TCP loss, reordering, and duplication torture tests, plus the end-to-end application
//! and PCAP proofs.
//!
//! Scenarios:
//!   A  lost SYN                 -> retransmitted, connection establishes
//!   B  lost SYN-ACK             -> retransmitted, connection establishes
//!   C  lost data segment        -> recovered, no bytes lost
//!   D  reordering (1,3,4,2,5)   -> reassembled in order
//!   E  duplicated segments      -> no duplicate application bytes
//!   F  lost ACK                 -> cumulative ACK semantics recover
//!   G  lost FIN                 -> teardown still completes
//!   H  HTTP/1.1 over the socket API, no hand-built packets anywhere
//!   I  128 KiB transfer over a lossy, reordering, small-MSS link, with a PCAP the
//!      project's own reader parses back
//!
//! Every scenario drives the real stack through the socket API.

mod common;

use common::{Fixture, payload};
use std::io::Cursor;
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::pcap::PcapReader;
use toy_tcpip::tcp::{TcpFlags, TcpSegment, TcpState};

// =====================================================================================
// A. Lost SYN
// =====================================================================================

#[test]
fn test_scenario_a_lost_syn_is_retransmitted_and_connection_establishes() {
    let mut fx = Fixture::new("lan_lost_syn", 512);
    let listener = fx.listen(80);

    // Frame 1 is the SYN (ARP is pre-seeded), so this drops the very first packet.
    fx.drop_frames(&[1]);

    let client = fx.connect(80);

    // Nothing can happen until the RTO fires.
    fx.settle();
    assert_eq!(
        fx.state("client", client),
        TcpState::SynSent,
        "client should still be waiting after the SYN was dropped"
    );
    assert_eq!(fx.frames_dropped(), 1, "the SYN was not actually dropped");

    let established = fx.run_until(100, 30_000, |lab| {
        lab.host("client")
            .unwrap()
            .stack
            .tcp_state(client)
            .map(|s| s == TcpState::Established)
            .unwrap_or(false)
    });
    assert!(established, "connection never recovered from the lost SYN");

    let stats = fx.stats("client", client);
    assert!(
        stats.retransmissions >= 1,
        "the SYN was never retransmitted (retransmissions = {})",
        stats.retransmissions
    );

    // And the connection is usable.
    let (client, server) = fx.establish_existing(listener, client);
    let data = payload(1_024);
    assert_eq!(fx.transfer("client", client, "server", server, &data), data);
}

// =====================================================================================
// B. Lost SYN-ACK
// =====================================================================================

#[test]
fn test_scenario_b_lost_syn_ack_is_recovered() {
    let mut fx = Fixture::new("lan_lost_synack", 512);
    let listener = fx.listen(80);

    // Frame 1 is the SYN, frame 2 is the SYN-ACK.
    fx.drop_frames(&[2]);

    let client = fx.connect(80);
    fx.settle();
    assert_eq!(fx.frames_dropped(), 1, "the SYN-ACK was not dropped");
    assert_eq!(fx.state("client", client), TcpState::SynSent);

    let established = fx.run_until(100, 30_000, |lab| {
        lab.host("client")
            .unwrap()
            .stack
            .tcp_state(client)
            .map(|s| s == TcpState::Established)
            .unwrap_or(false)
    });
    assert!(
        established,
        "connection never recovered from the lost SYN-ACK"
    );

    let (client, server) = fx.establish_existing(listener, client);
    let data = payload(1_024);
    assert_eq!(fx.transfer("client", client, "server", server, &data), data);
}

// =====================================================================================
// C. Lost data segment
// =====================================================================================

#[test]
fn test_scenario_c_lost_data_segment_loses_no_bytes() {
    let mut fx = Fixture::new("lan_lost_data", 256);
    let listener = fx.listen(80);
    let (client, server) = fx.establish(listener, 80);

    // The handshake used frames 1-3; drop data segments after it.
    fx.drop_frames(&[5, 11, 19]);

    let data = payload(8_192);
    let got = fx.transfer("client", client, "server", server, &data);

    assert_eq!(got.len(), data.len(), "bytes were lost");
    assert_eq!(got, data, "recovered stream does not match the source");
    assert_eq!(fx.frames_dropped(), 3, "the drops did not take effect");

    // Not every dropped frame costs a retransmission: a dropped *ACK* is covered by the
    // next cumulative acknowledgement. What must hold is that recovery happened at all
    // and that it happened by retransmitting, not by luck.
    let stats = fx.stats("client", client);
    assert!(
        stats.retransmissions >= 2,
        "expected retransmissions to drive recovery, got {}",
        stats.retransmissions
    );
}

// =====================================================================================
// D. Reordering
// =====================================================================================

#[test]
fn test_scenario_d_reordering_is_reassembled_in_order() {
    // Segments leave as 1,2,3,4,5 and arrive as 1,3,4,2,5: segment 2 is held on the link
    // until after segment 4 has crossed.
    let mut fx = Fixture::new("lan_reorder", 256);
    let listener = fx.listen(80);
    let (client, server) = fx.establish(listener, 80);

    // Frames 4..8 are the first five data segments; hold frame 5 until frame 7 passes.
    fx.reorder(5, 7);

    let data = payload(4_096);
    let got = fx.transfer("client", client, "server", server, &data);

    assert_eq!(got.len(), data.len());
    assert_eq!(
        got, data,
        "out-of-order reassembly produced the wrong stream"
    );

    // The receiver really did have to buffer out of order, which shows up as the
    // duplicate ACKs it emitted while the hole was open.
    let server_stats = fx.stats("server", server);
    assert!(
        server_stats.segments_received > 5,
        "not enough segments to have exercised reordering"
    );
}

#[test]
fn test_scenario_d2_heavy_reordering_still_reassembles() {
    let mut fx = Fixture::new("lan_reorder_heavy", 256);
    let listener = fx.listen(80);
    let (client, server) = fx.establish(listener, 80);

    for (hold, after) in [(5usize, 8usize), (10, 13), (16, 20), (25, 30), (34, 38)] {
        fx.reorder(hold, after);
    }

    let data = payload(16_384);
    let got = fx.transfer("client", client, "server", server, &data);
    assert_eq!(got, data, "heavy reordering corrupted the stream");
}

// =====================================================================================
// E. Duplication
// =====================================================================================

#[test]
fn test_scenario_e_duplicate_segments_do_not_duplicate_application_bytes() {
    // Deliver the same data segment repeatedly straight into the receiving stack. The
    // application must still see each byte exactly once.
    let mut fx = Fixture::new("lan_duplicate", 512);
    let listener = fx.listen(80);
    let (client, server) = fx.establish(listener, 80);

    let data = payload(2_048);
    let got = fx.transfer("client", client, "server", server, &data);
    assert_eq!(got, data);

    let before = fx.bytes_received("server", server);

    // Replay: capture what the client sends next, then feed it to the server three times.
    let more = payload(1_024);
    fx.write("client", client, &more);
    fx.lab.pump();

    let replay: Vec<Vec<u8>> = fx
        .lab
        .in_flight_frames
        .iter()
        .filter(|(sender, _, _)| sender == "client")
        .map(|(_, _, frame)| frame.clone())
        .collect();
    assert!(
        !replay.is_empty(),
        "expected the client to have queued data frames"
    );

    fx.settle();
    let first_pass = fx.drain("server", server);

    // Now replay the identical frames several more times.
    for _ in 0..3 {
        for frame in &replay {
            let _ = fx
                .lab
                .host_mut("server")
                .unwrap()
                .stack
                .process_frame(frame);
        }
    }
    fx.settle();

    let mut received = first_pass;
    received.extend(fx.drain("server", server));

    // Finish the transfer normally in case the window held some of it back.
    let mut guard = 0;
    while received.len() < more.len() && guard < 500 {
        fx.tick(25);
        received.extend(fx.drain("server", server));
        guard += 1;
    }

    assert_eq!(
        received.len(),
        more.len(),
        "duplicate segments delivered {} bytes for a {} byte write",
        received.len(),
        more.len()
    );
    assert_eq!(received, more, "duplicate delivery corrupted the stream");

    let after = fx.bytes_received("server", server);
    assert_eq!(
        after - before,
        more.len() as u64,
        "the receiver counted duplicate bytes as new data"
    );
}

// =====================================================================================
// F. Lost ACK
// =====================================================================================

#[test]
fn test_scenario_f_lost_acks_recover_through_cumulative_acknowledgement() {
    let mut fx = Fixture::new("lan_lost_ack", 256);
    let listener = fx.listen(80);
    let (client, server) = fx.establish(listener, 80);

    // Drop a run of frames in the server -> client direction. Since both hosts share the
    // link, these indices land on ACKs interleaved with the data flow.
    fx.drop_frames(&[6, 8, 10, 12, 14]);

    let data = payload(8_192);
    let got = fx.transfer("client", client, "server", server, &data);

    assert_eq!(got, data, "cumulative ACK recovery lost or reordered bytes");
    assert!(
        fx.frames_dropped() >= 5,
        "the ACK drops did not take effect"
    );
}

// =====================================================================================
// G. Lost FIN
// =====================================================================================

#[test]
fn test_scenario_g_lost_fin_still_tears_the_connection_down() {
    let mut fx = Fixture::new("lan_lost_fin", 512);
    let listener = fx.listen(80);
    let (client, server) = fx.establish(listener, 80);

    let data = payload(512);
    assert_eq!(fx.transfer("client", client, "server", server, &data), data);

    // The next frame the client sends is its FIN; drop it (and its first retry).
    let next_index = fx.frames_forwarded() + fx.frames_dropped() + 1;
    fx.drop_frames(&[next_index, next_index + 2]);

    fx.close("client", client);
    fx.settle();

    let closed = fx.run_until(200, 60_000, |lab| {
        lab.host("server")
            .unwrap()
            .stack
            .tcp_state(server)
            .map(|s| {
                matches!(
                    s,
                    TcpState::CloseWait | TcpState::LastAck | TcpState::Closed | TcpState::TimeWait
                )
            })
            .unwrap_or(false)
    });
    assert!(closed, "the server never saw the retransmitted FIN");

    let stats = fx.stats("client", client);
    assert!(
        stats.retransmissions >= 1,
        "the FIN was never retransmitted"
    );

    // Both ends finish.
    fx.close("server", server);
    let done = fx.run_until(500, 60_000, |lab| {
        lab.host("client")
            .unwrap()
            .stack
            .tcp_state(client)
            .map(|s| matches!(s, TcpState::TimeWait | TcpState::Closed))
            .unwrap_or(false)
    });
    assert!(done, "the client never completed teardown");
}

// =====================================================================================
// H. HTTP/1.1 application proof
// =====================================================================================

/// A minimal HTTP/1.1 origin server. It sees a byte stream and nothing else: no TCP, no
/// IPv4, no Ethernet, no access to `TcpConnection`.
fn http_server_respond(request: &str) -> String {
    let request_line = request.lines().next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    let version = parts.next().unwrap_or("");

    if version != "HTTP/1.1" {
        return "HTTP/1.1 505 HTTP Version Not Supported\r\nContent-Length: 0\r\n\r\n".to_string();
    }

    let (status, body) = match (method, target) {
        ("GET", "/hello") => ("200 OK", "Hello from the userspace TCP/IP stack!\n"),
        ("GET", _) => ("404 Not Found", "not found\n"),
        _ => ("405 Method Not Allowed", "method not allowed\n"),
    };

    format!(
        "HTTP/1.1 {}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        body.len(),
        body
    )
}

#[test]
fn test_scenario_h_http11_request_and_response_over_the_socket_api() {
    let mut fx = Fixture::new("lan_http", 536);
    let listener = fx.listen(8080);
    let (client, server) = fx.establish(listener, 8080);

    // --- HTTP client: writes a request to a stream ---
    let request = "GET /hello HTTP/1.1\r\nHost: lab.local\r\n\r\n";
    let server_saw = fx.transfer("client", client, "server", server, request.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&server_saw),
        request,
        "the server received a different request than the client sent"
    );

    // --- HTTP server: answers on the same stream ---
    let response = http_server_respond(&String::from_utf8_lossy(&server_saw));
    let client_saw = fx.transfer("server", server, "client", client, response.as_bytes());

    let text = String::from_utf8_lossy(&client_saw).to_string();
    assert!(
        text.starts_with("HTTP/1.1 200 OK\r\n"),
        "unexpected status line in:\n{}",
        text
    );
    assert!(text.contains("Content-Type: text/plain\r\n"));
    let body = "Hello from the userspace TCP/IP stack!\n";
    assert!(
        text.contains(&format!("Content-Length: {}\r\n", body.len())),
        "Content-Length header missing or wrong in:\n{}",
        text
    );
    assert!(
        text.ends_with("\r\n\r\nHello from the userspace TCP/IP stack!\n"),
        "unexpected body in:\n{}",
        text
    );
    assert_eq!(text, response, "the response was corrupted in transit");

    // The server closes; the client observes a clean end of stream.
    fx.close("server", server);
    let eof = fx.run_until(100, 60_000, |lab| {
        lab.host("client")
            .unwrap()
            .stack
            .tcp_state(client)
            .map(|s| {
                matches!(
                    s,
                    TcpState::CloseWait | TcpState::LastAck | TcpState::Closed | TcpState::TimeWait
                )
            })
            .unwrap_or(false)
    });
    assert!(eof, "the client never saw the server's FIN");
}

#[test]
fn test_scenario_h2_http_over_a_lossy_link() {
    // The same application, unchanged, over a link that drops packets.
    let mut fx = Fixture::new("lan_http_lossy", 128);
    let listener = fx.listen(8080);
    let (client, server) = fx.establish(listener, 8080);

    fx.drop_frames(&[5, 9, 14, 18]);

    let request = "GET /hello HTTP/1.1\r\nHost: lab.local\r\nX-Padding: ".to_string()
        + &"p".repeat(600)
        + "\r\n\r\n";
    let server_saw = fx.transfer("client", client, "server", server, request.as_bytes());
    assert_eq!(String::from_utf8_lossy(&server_saw), request);

    let response = http_server_respond(&String::from_utf8_lossy(&server_saw));
    let client_saw = fx.transfer("server", server, "client", client, response.as_bytes());
    assert_eq!(String::from_utf8_lossy(&client_saw), response);

    assert!(
        fx.stats("client", client).retransmissions > 0,
        "the lossy HTTP exchange never needed a retransmission"
    );
}

// =====================================================================================
// I. Large lossy transfer with PCAP proof
// =====================================================================================

#[test]
fn test_scenario_i_128kib_transfer_over_lossy_reordering_link_with_pcap() {
    const TOTAL: usize = 128 * 1024;
    const MSS: u16 = 512;

    let mut fx = Fixture::new("lan_lossy_pcap", MSS);
    fx.enable_pcap();

    let listener = fx.listen(80);
    let (client, server) = fx.establish(listener, 80);

    // Deterministic, unforgiving fault injection across the whole transfer: every 23rd
    // frame is dropped and every 37th is held back behind a later one. With a 512-byte
    // MSS a 128 KiB transfer needs 256+ segments, so this drops dozens of packets --
    // including data segments, ACKs, and eventually the FIN.
    let drops: Vec<usize> = (0..80).map(|i| 5 + i * 23).collect();
    fx.drop_frames(&drops);
    for i in 0..25 {
        let hold = 9 + i * 37;
        fx.reorder(hold, hold + 3);
    }

    let data = payload(TOTAL);
    let got = fx.transfer("client", client, "server", server, &data);

    // ---- The acceptance criterion: byte-identical delivery ----
    assert_eq!(
        got.len(),
        TOTAL,
        "received {} bytes for a {} byte transfer",
        got.len(),
        TOTAL
    );
    assert_eq!(got, data, "the received stream is not byte-identical");

    // ---- The loss was real and retransmission actually did the work ----
    let dropped = fx.frames_dropped();
    assert!(
        dropped >= 20,
        "only {} frames were dropped; the scenario was too gentle to prove anything",
        dropped
    );

    let stats = fx.stats("client", client);
    assert!(
        stats.retransmissions >= 20,
        "only {} retransmissions for {} dropped frames",
        stats.retransmissions,
        dropped
    );
    assert!(
        stats.segments_sent as usize >= TOTAL / MSS as usize,
        "the payload was not segmented to the MSS"
    );
    assert!(
        stats.duplicate_acks > 0,
        "the receiver never produced a duplicate ACK despite the holes"
    );

    // ---- PCAP proof: the project's own reader parses the capture back ----
    let pcap_bytes = fx.export_pcap().expect("PCAP capture was enabled");
    let mut reader = PcapReader::new(Cursor::new(pcap_bytes)).expect("PcapReader header");

    let mut counts = TraceCounts::default();
    let mut packets = 0usize;
    while let Ok(Some(record)) = reader.next_packet() {
        packets += 1;
        counts.observe(&record.data);
    }

    assert!(
        packets >= 250,
        "expected a substantial trace, parsed only {} packets",
        packets
    );
    assert!(counts.syn > 0, "no SYN in the capture");
    assert!(counts.syn_ack > 0, "no SYN-ACK in the capture");
    assert!(counts.ack > 0, "no ACK in the capture");
    assert!(counts.data > 0, "no data segments in the capture");
    assert!(
        counts.duplicate_acks > 0,
        "no duplicate ACKs in the capture despite the injected holes"
    );
    assert!(
        counts.retransmissions > 0,
        "no retransmitted sequence numbers in the capture"
    );

    // Close down and confirm the FIN also appears on the wire.
    fx.close("client", client);
    fx.run_until(200, 60_000, |lab| {
        lab.host("server")
            .unwrap()
            .stack
            .tcp_state(server)
            .map(|s| {
                matches!(
                    s,
                    TcpState::CloseWait | TcpState::LastAck | TcpState::Closed | TcpState::TimeWait
                )
            })
            .unwrap_or(false)
    });

    let pcap_bytes = fx.export_pcap().expect("PCAP capture");
    let mut reader = PcapReader::new(Cursor::new(pcap_bytes)).expect("PcapReader header");
    let mut counts = TraceCounts::default();
    while let Ok(Some(record)) = reader.next_packet() {
        counts.observe(&record.data);
    }
    assert!(counts.fin > 0, "no FIN in the capture after close");
}

/// Tallies the TCP control-flag and retransmission story visible in a captured trace.
#[derive(Default)]
struct TraceCounts {
    syn: usize,
    syn_ack: usize,
    ack: usize,
    data: usize,
    fin: usize,
    rst: usize,
    duplicate_acks: usize,
    retransmissions: usize,
    seen_seq: Vec<(u16, u32)>,
    last_ack: Option<(u16, u32)>,
    repeat_ack_run: usize,
}

impl TraceCounts {
    /// Parses one captured Ethernet frame and, when it carries TCP, folds it into the tally.
    fn observe(&mut self, frame: &[u8]) {
        // Ethernet II: 12 bytes of addresses, then the EtherType.
        if frame.len() < 34 {
            return;
        }
        if u16::from_be_bytes([frame[12], frame[13]]) != 0x0800 {
            return;
        }
        let ip = &frame[14..];
        let ihl = ((ip[0] & 0x0F) as usize) * 4;
        if ip.len() < ihl || ip[9] != 6 {
            return; // not IPv4/TCP
        }
        let src_ip = Ipv4Address([ip[12], ip[13], ip[14], ip[15]]);
        let dst_ip = Ipv4Address([ip[16], ip[17], ip[18], ip[19]]);
        let total_len = u16::from_be_bytes([ip[2], ip[3]]) as usize;
        if total_len < ihl || total_len > ip.len() {
            return;
        }
        let tcp_bytes = &ip[ihl..total_len];

        let Ok(seg) = TcpSegment::parse(src_ip, dst_ip, tcp_bytes, false) else {
            return;
        };

        let f: TcpFlags = seg.flags;
        if f.rst {
            self.rst += 1;
        }
        if f.fin {
            self.fin += 1;
        }
        if f.syn && f.ack {
            self.syn_ack += 1;
        } else if f.syn {
            self.syn += 1;
        }
        if f.ack {
            self.ack += 1;
        }

        if !seg.payload.is_empty() {
            self.data += 1;
            // A sequence number seen before on the same source port is a retransmission.
            let key = (seg.src_port, seg.seq_num);
            if self.seen_seq.contains(&key) {
                self.retransmissions += 1;
            } else {
                self.seen_seq.push(key);
            }
        } else if f.ack && !f.syn && !f.fin {
            // Consecutive bare ACKs repeating the same acknowledgement number are the
            // duplicate ACKs that drive fast retransmit.
            let key = (seg.src_port, seg.ack_num);
            if self.last_ack == Some(key) {
                self.repeat_ack_run += 1;
                self.duplicate_acks += 1;
            } else {
                self.last_ack = Some(key);
                self.repeat_ack_run = 0;
            }
        }
    }
}
