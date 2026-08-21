//! Shared harness for the socket-runtime and TCP reliability integration tests.
//!
//! Everything here drives the real stack through the application-facing socket API:
//! `tcp_listen` / `tcp_connect` / `tcp_write` / `tcp_read` / `tcp_close` and the UDP
//! equivalents. No helper builds a TCP segment, IPv4 packet, or Ethernet frame, and no
//! helper reaches into `TcpConnection`. Simulated time is the only clock.

#![allow(dead_code)]

/// Multi-AS BGP topologies used by the control-plane integration suites.
pub mod bgp_lab;

use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::lab::VirtualLab;
use toy_tcpip::socket::{TcpListenerHandle, TcpStreamHandle};
use toy_tcpip::stack::NetStackConfig;
use toy_tcpip::tcp::{SocketAddrV4, TcpState};

pub const CLIENT_IP: Ipv4Address = Ipv4Address([10, 77, 0, 10]);
pub const SERVER_IP: Ipv4Address = Ipv4Address([10, 77, 0, 20]);
pub const CLIENT_MAC: MacAddress = MacAddress([0x02, 0x00, 0x00, 0x00, 0x77, 0x0A]);
pub const SERVER_MAC: MacAddress = MacAddress([0x02, 0x00, 0x00, 0x00, 0x77, 0x14]);

/// A two-host lab on a single link with the client and server socket runtimes ready to use.
pub struct Fixture {
    pub lab: VirtualLab,
    pub link: String,
}

impl Fixture {
    /// Builds the lab. ARP is pre-seeded so that link frame indices correspond one-to-one
    /// with TCP segments, which is what makes index-based loss injection meaningful.
    pub fn new(link: &str, mss: u16) -> Self {
        let mut lab = VirtualLab::new();
        lab.add_link(link);

        lab.add_host(
            "client",
            link,
            NetStackConfig {
                mac: CLIENT_MAC,
                ip: CLIENT_IP,
                ipv6: None,
                subnet_mask: 24,
                gateway: None,
            },
        );
        lab.add_host(
            "server",
            link,
            NetStackConfig {
                mac: SERVER_MAC,
                ip: SERVER_IP,
                ipv6: None,
                subnet_mask: 24,
                gateway: None,
            },
        );

        lab.host_mut("client")
            .unwrap()
            .stack
            .arp_table
            .insert(SERVER_IP.0, SERVER_MAC);
        lab.host_mut("server")
            .unwrap()
            .stack
            .arp_table
            .insert(CLIENT_IP.0, CLIENT_MAC);

        lab.host_mut("client").unwrap().stack.set_tcp_mss(mss);
        lab.host_mut("server").unwrap().stack.set_tcp_mss(mss);

        Fixture {
            lab,
            link: link.to_string(),
        }
    }

    /// Drops the given zero-based link frame indices.
    pub fn drop_frames(&mut self, indices: &[usize]) {
        self.lab
            .link_mut(&self.link)
            .unwrap()
            .drop_packet_indices(indices);
    }

    /// Holds frame `hold` until frame `release_after` has crossed the link, producing
    /// deterministic reordering.
    pub fn reorder(&mut self, hold: usize, release_after: usize) {
        self.lab
            .link_mut(&self.link)
            .unwrap()
            .reorder_packet_indices
            .push((hold, release_after));
    }

    pub fn enable_pcap(&mut self) {
        self.lab.enable_pcap(&self.link);
    }

    pub fn export_pcap(&mut self) -> Option<Vec<u8>> {
        self.lab.export_pcap(&self.link)
    }

    pub fn frames_dropped(&self) -> usize {
        self.lab
            .link(&self.link)
            .map(|l| l.frames_dropped)
            .unwrap_or(0)
    }

    pub fn frames_forwarded(&self) -> usize {
        self.lab
            .link(&self.link)
            .map(|l| l.frames_forwarded)
            .unwrap_or(0)
    }

    /// Starts a listener on the server.
    pub fn listen(&mut self, port: u16) -> TcpListenerHandle {
        self.lab
            .host_mut("server")
            .unwrap()
            .stack
            .tcp_listen(port)
            .expect("tcp_listen")
    }

    /// Opens a connection from the client using an ephemeral local port.
    pub fn connect(&mut self, port: u16) -> TcpStreamHandle {
        self.lab
            .host_mut("client")
            .unwrap()
            .stack
            .tcp_connect(SocketAddrV4 {
                ip: SERVER_IP,
                port,
            })
            .expect("tcp_connect")
    }

    /// Opens a connection from a chosen local port and ISN (used by wraparound tests).
    pub fn connect_from(&mut self, local_port: u16, port: u16, isn: u32) -> TcpStreamHandle {
        self.lab
            .host_mut("client")
            .unwrap()
            .stack
            .tcp_connect_from(
                local_port,
                SocketAddrV4 {
                    ip: SERVER_IP,
                    port,
                },
                isn,
            )
            .expect("tcp_connect_from")
    }

    /// Establishes a connection end to end and returns `(client_stream, server_stream)`.
    pub fn establish(
        &mut self,
        listener: TcpListenerHandle,
        port: u16,
    ) -> (TcpStreamHandle, TcpStreamHandle) {
        let client = self.connect(port);
        self.establish_existing(listener, client)
    }

    /// Completes the handshake for an already-initiated client stream and accepts it.
    pub fn establish_existing(
        &mut self,
        listener: TcpListenerHandle,
        client: TcpStreamHandle,
    ) -> (TcpStreamHandle, TcpStreamHandle) {
        assert!(
            self.run_until(25, 60_000, |lab| {
                lab.host("client")
                    .unwrap()
                    .stack
                    .tcp_state(client)
                    .map(|s| s == TcpState::Established)
                    .unwrap_or(false)
            }),
            "client never reached ESTABLISHED"
        );
        let (server, _) = self
            .lab
            .host_mut("server")
            .unwrap()
            .stack
            .tcp_accept(listener)
            .expect("accept queue empty after handshake");
        (client, server)
    }

    /// Writes `data`, honouring short writes: the send buffer is bounded, so a large write
    /// is accepted in pieces as the network drains it.
    pub fn write(&mut self, host: &str, stream: TcpStreamHandle, data: &[u8]) -> usize {
        let mut offset = 0usize;
        let mut guard = 0usize;
        while offset < data.len() && guard < 100_000 {
            match self
                .lab
                .host_mut(host)
                .unwrap()
                .stack
                .tcp_write(stream, &data[offset..])
            {
                Ok(n) => offset += n,
                Err(_) => {
                    // Buffer full: let the network drain before trying again.
                    self.lab.run_pumped(20);
                    self.lab.advance_time(25);
                }
            }
            guard += 1;
        }
        offset
    }

    pub fn close(&mut self, host: &str, stream: TcpStreamHandle) {
        self.lab
            .host_mut(host)
            .unwrap()
            .stack
            .tcp_close(stream)
            .expect("tcp_close");
    }

    /// Reads every byte currently available on a stream.
    pub fn drain(&mut self, host: &str, stream: TcpStreamHandle) -> Vec<u8> {
        let mut out = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match self
                .lab
                .host_mut(host)
                .unwrap()
                .stack
                .tcp_read(stream, &mut chunk)
            {
                Ok(0) => break,
                Ok(n) => out.extend_from_slice(&chunk[..n]),
                Err(_) => break,
            }
        }
        out
    }

    pub fn state(&self, host: &str, stream: TcpStreamHandle) -> TcpState {
        self.lab
            .host(host)
            .unwrap()
            .stack
            .tcp_state(stream)
            .expect("tcp_state")
    }

    pub fn stats(&self, host: &str, stream: TcpStreamHandle) -> toy_tcpip::tcp::TcpStats {
        self.lab
            .host(host)
            .unwrap()
            .stack
            .tcp_stats(stream)
            .expect("tcp_stats")
    }

    pub fn bytes_received(&self, host: &str, stream: TcpStreamHandle) -> u64 {
        self.lab
            .host(host)
            .unwrap()
            .stack
            .tcp_stats(stream)
            .map(|s| s.bytes_received)
            .unwrap_or(0)
    }

    pub fn run_until<F>(&mut self, tick_ms: u64, max_sim_ms: u64, predicate: F) -> bool
    where
        F: FnMut(&VirtualLab) -> bool,
    {
        self.lab.run_until(tick_ms, max_sim_ms, predicate)
    }

    /// Runs the network to quiescence at the current simulated time.
    pub fn settle(&mut self) {
        self.lab.run_pumped(50);
    }

    /// Advances simulated time and settles the network.
    pub fn tick(&mut self, ms: u64) {
        self.lab.advance_time(ms);
        self.lab.run_pumped(50);
    }

    /// Transfers `payload` from `from_host` to `to_host` and returns exactly what the
    /// receiving application read. Panics if the transfer does not complete in budget.
    pub fn transfer(
        &mut self,
        from_host: &str,
        from: TcpStreamHandle,
        to_host: &str,
        to: TcpStreamHandle,
        payload: &[u8],
    ) -> Vec<u8> {
        self.write(from_host, from, payload);

        // Drain the receiver as the transfer proceeds, the way a real application does.
        // Letting the receive buffer fill instead would (correctly) close the advertised
        // window and stall the sender, which would test the harness rather than the stack.
        let mut received = Vec::with_capacity(payload.len());
        let deadline = self.lab.current_time_ms + 600_000;

        loop {
            self.lab.run_pumped(50);
            received.extend(self.drain(to_host, to));
            if received.len() >= payload.len() {
                break;
            }
            if self.lab.current_time_ms >= deadline {
                break;
            }
            self.lab.advance_time(25);
        }
        received.extend(self.drain(to_host, to));

        assert!(
            received.len() >= payload.len(),
            "transfer of {} bytes did not complete within the simulated budget ({} received)",
            payload.len(),
            received.len()
        );
        received
    }
}

/// Deterministic pseudo-random test payload. Not cryptographic; it just needs to be a
/// non-repeating byte pattern so a misordered or duplicated segment is detectable.
pub fn payload(len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut state: u32 = 0x9E37_79B9;
    for _ in 0..len {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        out.push((state >> 24) as u8);
    }
    out
}
