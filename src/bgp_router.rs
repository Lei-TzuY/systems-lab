//! Packet-driven BGP-4 control plane (RFC 4271).
//!
//! This is a real BGP speaker, not a model of one. Every message it exchanges travels
//! over this repository's own reliable TCP runtime on port 179:
//!
//! ```text
//! BgpRouter -> TcpListener / TcpStream :179 -> SocketRuntime -> IPv4 / ARP / Ethernet
//! ```
//!
//! and every route it selects is installed into the same `RoutingTable` the IPv4
//! forwarding path consults:
//!
//! ```text
//! UPDATE -> Adj-RIB-In -> best path -> Loc-RIB -> RoutingTable -> IPv4 forwarding
//! ```
//!
//! The speaker never shuttles messages between peer objects in memory, never sleeps,
//! never spawns a thread, and never reads a wall clock. `poll` is driven with a
//! simulated timestamp, which is what makes the whole control plane reproducible.

use crate::bgp::{
    BGP_DEFAULT_LOCAL_PREF, BGP_ERR_CEASE, BGP_ERR_FSM, BGP_ERR_HOLD_TIMER_EXPIRED,
    BGP_ERR_UPDATE_MESSAGE, BGP_MIN_HOLD_TIME, BGP_PORT, BGP_SUB_BAD_BGP_IDENTIFIER,
    BGP_SUB_BAD_PEER_AS, BGP_SUB_INVALID_NEXT_HOP, BGP_SUB_MALFORMED_AS_PATH,
    BGP_SUB_UNACCEPTABLE_HOLD_TIME, BGP_SUB_UNSUPPORTED_VERSION, BGP_VERSION, BgpFramer,
    BgpNotificationMessage, BgpOpenMessage, BgpParseError, BgpPathAttributes, BgpPdu,
    BgpUpdateMessage, Ipv4Prefix,
};
use crate::bgp_rib::{
    AdjRibIn, AdjRibOut, AdvertisedRoute, BgpPath, LocRib, PathSource, PolicyOutcome, RoutePolicy,
    select_best,
};
use crate::ipv4::Ipv4Address;
use crate::router::{RouteSource, RoutingTable};
use crate::socket::{SocketError, SocketRuntime, TcpListenerHandle, TcpStreamHandle};
use crate::tcp::{SocketAddrV4, TcpState};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Default ConnectRetryTime, in simulated milliseconds (RFC 4271 suggests 120 s;
/// the lab runs a shorter one so convergence tests stay brisk in logical time).
pub const DEFAULT_CONNECT_RETRY_MS: u64 = 5_000;
/// Hold time proposed in OPEN, in seconds.
pub const DEFAULT_HOLD_TIME: u16 = 90;
/// Hold time applied while waiting for the peer's OPEN, before one is negotiated
/// (RFC 4271 section 8.2.2 "large value", 4 minutes).
pub const INITIAL_HOLD_MS: u64 = 240_000;
/// Bytes drained from the socket per read call.
const READ_CHUNK: usize = 2_048;
/// Upper bound on retained control-plane log lines.
pub const MAX_EVENT_LOG: usize = 512;
/// Default per-peer prefix limit. A neighbour that advertises more than this has its
/// session closed rather than being allowed to exhaust memory (RFC 4486 subcode 1).
pub const DEFAULT_MAX_PREFIXES: usize = 4_096;
/// NOTIFICATION subcode for "Maximum Number of Prefixes Reached" (RFC 4486).
pub const BGP_SUB_MAX_PREFIXES: u8 = 1;

/// BGP finite state machine states (RFC 4271 section 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BgpState {
    Idle,
    Connect,
    Active,
    OpenSent,
    OpenConfirm,
    Established,
}

impl BgpState {
    pub fn as_str(&self) -> &'static str {
        match self {
            BgpState::Idle => "Idle",
            BgpState::Connect => "Connect",
            BgpState::Active => "Active",
            BgpState::OpenSent => "OpenSent",
            BgpState::OpenConfirm => "OpenConfirm",
            BgpState::Established => "Established",
        }
    }
}

impl fmt::Display for BgpState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether this speaker opens the TCP connection to the peer or waits for it.
///
/// Configuring one end of every session passive is standard operational practice and
/// removes connection-collision ambiguity, which keeps the simulation deterministic.
/// An inbound connection that arrives for a peer already past `Active` is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BgpPeerMode {
    Active,
    Passive,
}

/// Per-peer message counters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BgpPeerCounters {
    pub opens_sent: u64,
    pub opens_received: u64,
    pub updates_sent: u64,
    pub updates_received: u64,
    pub keepalives_sent: u64,
    pub keepalives_received: u64,
    pub notifications_sent: u64,
    pub notifications_received: u64,
    /// NLRI discarded because the local ASN already appeared in AS_PATH.
    pub as_loops_rejected: u64,
    /// NLRI discarded by the import policy.
    pub policy_rejected: u64,
    /// NLRI discarded because the NEXT_HOP was unusable.
    pub next_hop_rejected: u64,
    /// UPDATEs refused because the AS_PATH was not acceptable on this session.
    pub as_path_rejected: u64,
}

/// One configured BGP neighbour and the session state that belongs to it.
pub struct BgpPeer {
    pub addr: Ipv4Address,
    pub remote_as: u16,
    /// Local address used as the source of the session (the "update source").
    pub local_addr: Ipv4Address,
    pub mode: BgpPeerMode,
    pub admin_up: bool,
    pub state: BgpState,
    pub stream: Option<TcpStreamHandle>,
    framer: BgpFramer,
    connect_retry_deadline: Option<u64>,
    hold_deadline: Option<u64>,
    keepalive_deadline: Option<u64>,
    pub negotiated_hold_ms: u64,
    pub keepalive_interval_ms: u64,
    pub remote_router_id: Option<Ipv4Address>,
    pub established_since_ms: Option<u64>,
    pub import_policy: RoutePolicy,
    pub export_policy: RoutePolicy,
    /// Advertise our own session address as the NEXT_HOP instead of passing on the one
    /// we were told. Always done on eBGP sessions; optional on iBGP ones, where it is
    /// what lets a peer with no IGP resolve the next hop.
    pub next_hop_self: bool,
    /// Largest number of prefixes this neighbour may hold in the Adj-RIB-In.
    pub max_prefixes: usize,
    /// Require an eBGP UPDATE to lead with this neighbour's own ASN (RFC 4271
    /// section 6.3). On by default, as it is on modern production routers.
    pub enforce_first_as: bool,
    /// Set when a BGP message could only be written to the transport in part. The
    /// stream then carries half a message and cannot be repaired by retrying, so the
    /// session is reset instead of being allowed to desynchronise the peer's framer.
    tx_desynced: bool,
    pub counters: BgpPeerCounters,
    pub last_error: Option<String>,
    /// How many times this peer has reached ESTABLISHED.
    pub establishment_count: u32,
}

impl BgpPeer {
    fn new(addr: Ipv4Address, remote_as: u16, local_addr: Ipv4Address, mode: BgpPeerMode) -> Self {
        BgpPeer {
            addr,
            remote_as,
            local_addr,
            mode,
            admin_up: true,
            state: BgpState::Idle,
            stream: None,
            framer: BgpFramer::new(),
            connect_retry_deadline: None,
            hold_deadline: None,
            keepalive_deadline: None,
            negotiated_hold_ms: 0,
            keepalive_interval_ms: 0,
            remote_router_id: None,
            established_since_ms: None,
            import_policy: RoutePolicy::new(),
            export_policy: RoutePolicy::new(),
            next_hop_self: false,
            max_prefixes: DEFAULT_MAX_PREFIXES,
            enforce_first_as: true,
            tx_desynced: false,
            counters: BgpPeerCounters::default(),
            last_error: None,
            establishment_count: 0,
        }
    }

    pub fn is_established(&self) -> bool {
        self.state == BgpState::Established
    }

    /// Simulated milliseconds since the session came up.
    pub fn uptime_ms(&self, now_ms: u64) -> Option<u64> {
        self.established_since_ms.map(|t| now_ms.saturating_sub(t))
    }

    /// Milliseconds left before the HoldTimer fires.
    pub fn hold_remaining_ms(&self, now_ms: u64) -> Option<u64> {
        self.hold_deadline.map(|d| d.saturating_sub(now_ms))
    }

    /// Milliseconds left before the next KEEPALIVE is due.
    pub fn keepalive_remaining_ms(&self, now_ms: u64) -> Option<u64> {
        self.keepalive_deadline.map(|d| d.saturating_sub(now_ms))
    }

    /// Bytes currently held in the stream reassembly buffer.
    pub fn buffered_bytes(&self) -> usize {
        self.framer.buffered()
    }
}

/// A control-plane log line, retained for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BgpEvent {
    pub time_ms: u64,
    pub peer: Ipv4Address,
    pub text: String,
}

impl fmt::Display for BgpEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:>8}ms] {} {}", self.time_ms, self.peer, self.text)
    }
}

/// Snapshot of one peer for `show bgp summary` style output.
#[derive(Debug, Clone)]
pub struct BgpPeerSummary {
    pub addr: Ipv4Address,
    pub remote_as: u16,
    pub local_addr: Ipv4Address,
    pub state: BgpState,
    pub router_id: Option<Ipv4Address>,
    pub uptime_ms: Option<u64>,
    pub hold_ms: u64,
    pub hold_remaining_ms: Option<u64>,
    pub keepalive_interval_ms: u64,
    pub keepalive_remaining_ms: Option<u64>,
    pub prefixes_received: usize,
    pub prefixes_advertised: usize,
    pub counters: BgpPeerCounters,
    pub last_error: Option<String>,
    pub establishment_count: u32,
}

/// Why a session is being torn down.
enum Teardown {
    /// The transport went away; no NOTIFICATION can be delivered.
    Transport(String),
    /// A protocol violation; tell the peer before closing.
    Protocol(BgpNotificationMessage, String),
    /// The peer told us it is going away.
    PeerNotification(String),
}

/// A BGP-4 speaker: one local AS, one BGP identifier, a set of peers, three RIBs,
/// and the decision process that connects them to the forwarding table.
pub struct BgpRouter {
    pub local_as: u16,
    pub router_id: Ipv4Address,
    /// Hold time proposed in our OPEN, in seconds.
    pub hold_time: u16,
    pub connect_retry_ms: u64,
    peers: Vec<BgpPeer>,
    listener: Option<TcpListenerHandle>,
    pub adj_rib_in: AdjRibIn,
    pub loc_rib: LocRib,
    pub adj_rib_out: AdjRibOut,
    originated: BTreeMap<Ipv4Prefix, Ipv4Address>,
    /// Prefixes this speaker currently has installed in the FIB.
    installed: BTreeSet<Ipv4Prefix>,
    /// Best paths whose NEXT_HOP could not be resolved to an egress interface.
    unresolved: BTreeSet<Ipv4Prefix>,
    events: Vec<BgpEvent>,
    /// Set whenever the Adj-RIB-In or the originated set changes, so the decision
    /// process runs once per poll instead of once per UPDATE.
    dirty: bool,
    pub decision_runs: u64,
}

impl BgpRouter {
    pub fn new(local_as: u16, router_id: Ipv4Address) -> Self {
        BgpRouter {
            local_as,
            router_id,
            hold_time: DEFAULT_HOLD_TIME,
            connect_retry_ms: DEFAULT_CONNECT_RETRY_MS,
            peers: Vec::new(),
            listener: None,
            adj_rib_in: AdjRibIn::new(),
            loc_rib: LocRib::new(),
            adj_rib_out: AdjRibOut::new(),
            originated: BTreeMap::new(),
            installed: BTreeSet::new(),
            unresolved: BTreeSet::new(),
            events: Vec::new(),
            dirty: true,
            decision_runs: 0,
        }
    }

    /// Sets the hold time proposed in OPEN. Values of 1 or 2 seconds are illegal
    /// (RFC 4271 section 4.2) and are raised to the minimum.
    pub fn set_hold_time(&mut self, seconds: u16) {
        self.hold_time = if seconds == 0 {
            0
        } else {
            seconds.max(BGP_MIN_HOLD_TIME)
        };
    }

    pub fn set_connect_retry_ms(&mut self, ms: u64) {
        self.connect_retry_ms = ms.max(1);
    }

    /// Configures a neighbour. Peers are kept sorted by address so every iteration
    /// over them, and therefore every message ordering, is deterministic.
    pub fn add_peer(
        &mut self,
        addr: Ipv4Address,
        remote_as: u16,
        local_addr: Ipv4Address,
        mode: BgpPeerMode,
    ) {
        if self.peers.iter().any(|p| p.addr == addr) {
            return;
        }
        self.peers
            .push(BgpPeer::new(addr, remote_as, local_addr, mode));
        self.peers.sort_by_key(|p| p.addr);
    }

    pub fn peers(&self) -> &[BgpPeer] {
        &self.peers
    }

    pub fn peer(&self, addr: Ipv4Address) -> Option<&BgpPeer> {
        self.peers.iter().find(|p| p.addr == addr)
    }

    pub fn peer_mut(&mut self, addr: Ipv4Address) -> Option<&mut BgpPeer> {
        self.peers.iter_mut().find(|p| p.addr == addr)
    }

    pub fn peer_state(&self, addr: Ipv4Address) -> Option<BgpState> {
        self.peer(addr).map(|p| p.state)
    }

    pub fn established_peer_count(&self) -> usize {
        self.peers.iter().filter(|p| p.is_established()).count()
    }

    /// Sets the import policy applied to routes received from `addr`.
    pub fn set_import_policy(&mut self, addr: Ipv4Address, policy: RoutePolicy) {
        if let Some(p) = self.peer_mut(addr) {
            p.import_policy = policy;
        }
    }

    /// Sets the export policy applied to routes advertised to `addr`.
    pub fn set_export_policy(&mut self, addr: Ipv4Address, policy: RoutePolicy) {
        if let Some(p) = self.peer_mut(addr) {
            p.export_policy = policy;
        }
    }

    /// Turns next-hop-self on or off for `addr`. eBGP sessions always rewrite the
    /// NEXT_HOP regardless; this is what makes an iBGP peer usable without an IGP.
    pub fn set_next_hop_self(&mut self, addr: Ipv4Address, on: bool) {
        if let Some(p) = self.peer_mut(addr) {
            p.next_hop_self = on;
        }
    }

    /// Caps how many prefixes `addr` may install in the Adj-RIB-In.
    pub fn set_max_prefixes(&mut self, addr: Ipv4Address, limit: usize) {
        if let Some(p) = self.peer_mut(addr) {
            p.max_prefixes = limit;
        }
    }

    /// Turns the eBGP leading-AS check on or off for `addr`. Turning it off still
    /// leaves an empty AS_PATH from an external peer refused, because that is not a
    /// policy preference: a zero-length path would beat every real route.
    pub fn set_enforce_first_as(&mut self, addr: Ipv4Address, on: bool) {
        if let Some(p) = self.peer_mut(addr) {
            p.enforce_first_as = on;
        }
    }

    /// Originates a prefix into BGP, the equivalent of a `network` statement.
    /// `next_hop` is the address advertised to iBGP peers; eBGP advertisements use
    /// the session's own local address instead.
    pub fn originate(&mut self, prefix: Ipv4Prefix, next_hop: Ipv4Address) {
        self.originated.insert(prefix, next_hop);
        self.dirty = true;
    }

    /// Stops originating a prefix. The withdrawal propagates to every peer on the
    /// next poll and the FIB entry, if any, is removed.
    pub fn withdraw_originated(&mut self, prefix: Ipv4Prefix) -> bool {
        let removed = self.originated.remove(&prefix).is_some();
        if removed {
            self.dirty = true;
        }
        removed
    }

    pub fn originated_prefixes(&self) -> Vec<Ipv4Prefix> {
        self.originated.keys().copied().collect()
    }

    /// Administratively shuts a peer down: NOTIFICATION (Cease), TCP teardown, and
    /// removal of everything learned from it.
    pub fn shutdown_peer(&mut self, addr: Ipv4Address, now_ms: u64, sockets: &mut SocketRuntime) {
        let Some(idx) = self.peers.iter().position(|p| p.addr == addr) else {
            return;
        };
        self.peers[idx].admin_up = false;
        self.teardown(
            idx,
            now_ms,
            sockets,
            Teardown::Protocol(
                BgpNotificationMessage::new(BGP_ERR_CEASE, 0),
                "administratively shut down".to_string(),
            ),
        );
        self.peers[idx].connect_retry_deadline = None;
    }

    /// Re-enables a peer that was administratively shut down.
    pub fn enable_peer(&mut self, addr: Ipv4Address) {
        if let Some(p) = self.peer_mut(addr) {
            p.admin_up = true;
            p.connect_retry_deadline = None;
        }
    }

    pub fn events(&self) -> &[BgpEvent] {
        &self.events
    }

    fn log(&mut self, now_ms: u64, peer: Ipv4Address, text: impl Into<String>) {
        self.events.push(BgpEvent {
            time_ms: now_ms,
            peer,
            text: text.into(),
        });
        if self.events.len() > MAX_EVENT_LOG {
            let excess = self.events.len() - MAX_EVENT_LOG;
            self.events.drain(..excess);
        }
    }

    // ========================================================================
    // Main pump
    // ========================================================================

    /// Advances the whole control plane one step at simulated time `now_ms`.
    ///
    /// Accepts inbound connections, services every peer's FSM and timers, decodes
    /// whatever the TCP streams delivered, reruns the decision process if anything
    /// changed, syncs the FIB, and emits any UPDATEs the peers are owed.
    pub fn poll(&mut self, now_ms: u64, sockets: &mut SocketRuntime, fib: &mut RoutingTable) {
        self.ensure_listener(now_ms, sockets);
        self.accept_inbound(now_ms, sockets);

        for idx in 0..self.peers.len() {
            self.service_peer(idx, now_ms, sockets);
        }

        if self.dirty {
            self.run_decision_process(now_ms);
            self.dirty = false;
        }

        // The FIB is reconciled every poll, not only when the RIB changed. A NEXT_HOP
        // that was unresolvable earlier can become resolvable later, and reconciling
        // unconditionally also repairs the table if anything else disturbs it. Entries
        // that already match are left alone, so a steady state costs nothing.
        self.sync_fib(now_ms, fib);

        // Advertisement runs every poll: a peer that has just reached ESTABLISHED
        // needs the full Loc-RIB even when nothing about the RIB itself changed.
        for idx in 0..self.peers.len() {
            self.advertise_to_peer(idx, now_ms, sockets);
        }
    }

    fn ensure_listener(&mut self, now_ms: u64, sockets: &mut SocketRuntime) {
        if self.listener.is_some() {
            return;
        }
        match sockets.tcp_listen_any(BGP_PORT) {
            Ok(h) => {
                self.listener = Some(h);
                self.log(
                    now_ms,
                    Ipv4Address::UNSPECIFIED,
                    format!("listening on TCP port {}", BGP_PORT),
                );
            }
            Err(e) => {
                self.log(
                    now_ms,
                    Ipv4Address::UNSPECIFIED,
                    format!("cannot listen on port {}: {}", BGP_PORT, e),
                );
            }
        }
    }

    fn accept_inbound(&mut self, now_ms: u64, sockets: &mut SocketRuntime) {
        let Some(listener) = self.listener else {
            return;
        };
        let retry = self.connect_retry_ms;
        while let Ok((stream, remote)) = sockets.tcp_accept(listener) {
            let Some(idx) = self.peers.iter().position(|p| p.addr == remote.ip) else {
                self.log(
                    now_ms,
                    remote.ip,
                    "refused inbound session from an unconfigured neighbour",
                );
                Self::abandon_stream(sockets, stream, now_ms);
                continue;
            };

            let peer = &mut self.peers[idx];
            let acceptable = peer.admin_up
                && peer.stream.is_none()
                && matches!(peer.state, BgpState::Idle | BgpState::Active);
            if !acceptable {
                // Connection collision, or the peer is administratively down. Refuse
                // the new connection rather than abandoning a session in progress.
                let reason = format!(
                    "refused inbound session while in {} (collision guard)",
                    peer.state
                );
                self.log(now_ms, remote.ip, reason);
                Self::abandon_stream(sockets, stream, now_ms);
                continue;
            }

            peer.stream = Some(stream);
            peer.framer.reset();
            peer.state = BgpState::Active;
            // Bound the wait: an accepted connection whose handshake never finishes must
            // not hold the peer in Active forever.
            peer.connect_retry_deadline = Some(now_ms + retry);
            self.log(
                now_ms,
                remote.ip,
                "accepted inbound TCP session on port 179",
            );
        }
    }

    /// Drops a connection this speaker will not use: an inbound session from an
    /// unconfigured neighbour, or one that collides with a session already in progress.
    fn abandon_stream(sockets: &mut SocketRuntime, stream: TcpStreamHandle, now_ms: u64) {
        sockets.tcp_abort(stream, now_ms);
    }

    // ========================================================================
    // Per-peer FSM
    // ========================================================================

    fn service_peer(&mut self, idx: usize, now_ms: u64, sockets: &mut SocketRuntime) {
        if !self.peers[idx].admin_up {
            if self.peers[idx].state != BgpState::Idle {
                self.teardown(
                    idx,
                    now_ms,
                    sockets,
                    Teardown::Transport("administratively down".to_string()),
                );
            }
            return;
        }

        // A half-written message means the peer's framer is about to lose sync with
        // us. Nothing can be salvaged by writing more, so reset the session.
        if self.peers[idx].tx_desynced {
            self.teardown(
                idx,
                now_ms,
                sockets,
                Teardown::Transport(
                    "a BGP message was only partially written; the stream is desynchronised"
                        .to_string(),
                ),
            );
            return;
        }

        // A dead transport ends the session from any state that owns one.
        if let Some(stream) = self.peers[idx].stream
            && !sockets.tcp_is_live(stream)
        {
            self.teardown(
                idx,
                now_ms,
                sockets,
                Teardown::Transport("TCP connection failed".to_string()),
            );
            return;
        }

        match self.peers[idx].state {
            BgpState::Idle => self.start_peer(idx, now_ms, sockets),
            BgpState::Connect | BgpState::Active => self.progress_transport(idx, now_ms, sockets),
            BgpState::OpenSent | BgpState::OpenConfirm | BgpState::Established => {
                self.run_session(idx, now_ms, sockets)
            }
        }
    }

    /// Idle -> Connect (we dial) or Idle -> Active (we wait), once the
    /// ConnectRetryTimer allows another attempt.
    fn start_peer(&mut self, idx: usize, now_ms: u64, sockets: &mut SocketRuntime) {
        let ready = self.peers[idx]
            .connect_retry_deadline
            .is_none_or(|d| now_ms >= d);
        if !ready {
            return;
        }

        match self.peers[idx].mode {
            BgpPeerMode::Passive => {
                self.peers[idx].state = BgpState::Active;
                self.peers[idx].connect_retry_deadline = Some(now_ms + self.connect_retry_ms);
                self.log(now_ms, self.peers[idx].addr, "Idle -> Active (passive)");
            }
            BgpPeerMode::Active => self.dial(idx, now_ms, sockets),
        }
    }

    fn dial(&mut self, idx: usize, now_ms: u64, sockets: &mut SocketRuntime) {
        let peer_addr = self.peers[idx].addr;
        let local = SocketAddrV4 {
            ip: self.peers[idx].local_addr,
            port: 0,
        };
        let remote = SocketAddrV4 {
            ip: peer_addr,
            port: BGP_PORT,
        };
        let isn = 1_000 + (now_ms % 100_000) as u32 * 7;
        match sockets.tcp_connect_from(local, remote, isn) {
            Ok(stream) => {
                self.peers[idx].stream = Some(stream);
                self.peers[idx].framer.reset();
                self.peers[idx].state = BgpState::Connect;
                self.peers[idx].connect_retry_deadline = Some(now_ms + self.connect_retry_ms);
                self.log(now_ms, peer_addr, "Idle -> Connect (TCP SYN sent to :179)");
            }
            Err(e) => {
                self.peers[idx].state = BgpState::Active;
                self.peers[idx].connect_retry_deadline = Some(now_ms + self.connect_retry_ms);
                self.peers[idx].last_error = Some(format!("connect failed: {}", e));
                self.log(now_ms, peer_addr, format!("TCP connect failed: {}", e));
            }
        }
    }

    /// Connect / Active: waiting for the three-way handshake to complete, either the
    /// one we started or the one the peer started.
    fn progress_transport(&mut self, idx: usize, now_ms: u64, sockets: &mut SocketRuntime) {
        if let Some(stream) = self.peers[idx].stream {
            if sockets.tcp_state(stream) == Ok(TcpState::Established) {
                self.send_open(idx, now_ms, sockets);
                return;
            }
            // Handshake still in flight; the ConnectRetryTimer bounds how long we wait.
            if self.peers[idx]
                .connect_retry_deadline
                .is_some_and(|d| now_ms >= d)
            {
                self.teardown(
                    idx,
                    now_ms,
                    sockets,
                    Teardown::Transport("ConnectRetryTimer expired during handshake".to_string()),
                );
            }
            return;
        }

        // No transport yet.
        if self.peers[idx].mode == BgpPeerMode::Active
            && self.peers[idx]
                .connect_retry_deadline
                .is_none_or(|d| now_ms >= d)
        {
            self.dial(idx, now_ms, sockets);
        } else if self.peers[idx]
            .connect_retry_deadline
            .is_some_and(|d| now_ms >= d)
        {
            // Passive: just re-arm and keep waiting for the peer to call.
            self.peers[idx].connect_retry_deadline = Some(now_ms + self.connect_retry_ms);
        }
    }

    fn send_open(&mut self, idx: usize, now_ms: u64, sockets: &mut SocketRuntime) {
        let open = BgpOpenMessage::new(self.local_as, self.hold_time, self.router_id);
        if !self.send_pdu(idx, sockets, &BgpPdu::Open(open)) {
            return;
        }
        self.peers[idx].counters.opens_sent += 1;
        self.peers[idx].state = BgpState::OpenSent;
        self.peers[idx].connect_retry_deadline = None;
        self.peers[idx].hold_deadline = Some(now_ms + INITIAL_HOLD_MS);
        let addr = self.peers[idx].addr;
        self.log(now_ms, addr, "TCP established -> OPEN sent (OpenSent)");
    }

    /// OpenSent / OpenConfirm / Established: read the stream, decode complete
    /// messages, then run the hold and keepalive timers.
    fn run_session(&mut self, idx: usize, now_ms: u64, sockets: &mut SocketRuntime) {
        // 1. Drain whatever TCP has delivered into the reassembly buffer. A stream
        //    that has already ended still hands back everything delivered before the
        //    FIN, so end-of-stream is remembered rather than acted on straight away.
        let eof = match self.read_stream(idx, sockets) {
            Ok(open) => !open,
            Err(e) => {
                let note = BgpNotificationMessage::new(e.code, e.subcode);
                self.teardown(
                    idx,
                    now_ms,
                    sockets,
                    Teardown::Protocol(note, format!("framing error: {}", e)),
                );
                return;
            }
        };

        // 2. Decode and handle every complete message currently buffered. This runs
        //    even once the stream has ended: a peer that sends a final NOTIFICATION
        //    and closes in the same breath delivers both in a single read, and the
        //    NOTIFICATION is the real reason the session is going down. Reporting
        //    "peer closed the TCP connection" instead would throw that away.
        loop {
            let frame = match self.peers[idx].framer.next_frame() {
                Ok(Some(f)) => f,
                Ok(None) => break,
                Err(e) => {
                    let note = BgpNotificationMessage::new(e.code, e.subcode);
                    self.teardown(
                        idx,
                        now_ms,
                        sockets,
                        Teardown::Protocol(note, format!("framing error: {}", e)),
                    );
                    return;
                }
            };

            let pdu = match BgpPdu::parse(&frame) {
                Ok(p) => p,
                Err(e) => {
                    let note = BgpNotificationMessage::new(e.code, e.subcode);
                    self.teardown(
                        idx,
                        now_ms,
                        sockets,
                        Teardown::Protocol(note, format!("decode error: {}", e)),
                    );
                    return;
                }
            };

            if let Some(t) = self.handle_pdu(idx, now_ms, sockets, pdu) {
                self.teardown(idx, now_ms, sockets, t);
                return;
            }
        }

        // 3. The peer closed, and everything it said beforehand has now been acted on.
        if eof {
            self.teardown(
                idx,
                now_ms,
                sockets,
                Teardown::Transport("peer closed the TCP connection".to_string()),
            );
            return;
        }

        // 4. Timers.
        self.run_timers(idx, now_ms, sockets);
    }

    /// Reads everything available. Returns `Ok(false)` at end of stream.
    fn read_stream(
        &mut self,
        idx: usize,
        sockets: &mut SocketRuntime,
    ) -> Result<bool, BgpParseError> {
        let Some(stream) = self.peers[idx].stream else {
            return Ok(false);
        };
        let mut buf = [0u8; READ_CHUNK];
        loop {
            match sockets.tcp_read(stream, &mut buf) {
                Ok(0) => return Ok(false),
                Ok(n) => self.peers[idx].framer.push(&buf[..n])?,
                Err(SocketError::WouldBlock) => return Ok(true),
                Err(_) => return Ok(false),
            }
        }
    }

    /// Dispatches one decoded message according to the current FSM state.
    /// Returns `Some(Teardown)` when the session must end.
    fn handle_pdu(
        &mut self,
        idx: usize,
        now_ms: u64,
        sockets: &mut SocketRuntime,
        pdu: BgpPdu,
    ) -> Option<Teardown> {
        let state = self.peers[idx].state;
        let addr = self.peers[idx].addr;

        // Any message from the peer proves the session is alive.
        if state == BgpState::Established || state == BgpState::OpenConfirm {
            self.arm_hold_timer(idx, now_ms);
        }

        match (state, pdu) {
            (BgpState::OpenSent, BgpPdu::Open(open)) => {
                self.peers[idx].counters.opens_received += 1;
                if let Err(note) = self.validate_open(idx, &open) {
                    let reason = format!(
                        "rejected OPEN: code {}/{}",
                        note.error_code, note.error_subcode
                    );
                    return Some(Teardown::Protocol(note, reason));
                }
                let negotiated = self.hold_time.min(open.hold_time);
                self.peers[idx].remote_router_id = Some(open.bgp_id);
                self.peers[idx].negotiated_hold_ms = negotiated as u64 * 1_000;
                self.peers[idx].keepalive_interval_ms = if negotiated == 0 {
                    0
                } else {
                    (negotiated as u64 * 1_000) / 3
                };
                if !self.send_pdu(idx, sockets, &BgpPdu::Keepalive) {
                    return Some(Teardown::Transport(
                        "could not send KEEPALIVE after OPEN".to_string(),
                    ));
                }
                self.peers[idx].counters.keepalives_sent += 1;
                self.peers[idx].state = BgpState::OpenConfirm;
                self.arm_hold_timer(idx, now_ms);
                self.arm_keepalive_timer(idx, now_ms);
                self.log(
                    now_ms,
                    addr,
                    format!(
                        "OPEN received (AS {}, id {}, hold {}s) -> negotiated hold {}s, OpenConfirm",
                        open.my_as, open.bgp_id, open.hold_time, negotiated
                    ),
                );
                None
            }

            (BgpState::OpenConfirm, BgpPdu::Keepalive) => {
                self.peers[idx].counters.keepalives_received += 1;
                self.peers[idx].state = BgpState::Established;
                self.peers[idx].established_since_ms = Some(now_ms);
                self.peers[idx].establishment_count += 1;
                self.peers[idx].last_error = None;
                self.dirty = true;
                self.log(now_ms, addr, "KEEPALIVE received -> ESTABLISHED");
                None
            }

            (BgpState::Established, BgpPdu::Keepalive) => {
                self.peers[idx].counters.keepalives_received += 1;
                None
            }

            (BgpState::Established, BgpPdu::Update(update)) => {
                self.peers[idx].counters.updates_received += 1;
                match self.import_update(idx, now_ms, update) {
                    Ok(()) => None,
                    Err(note) => {
                        let reason = format!(
                            "rejected UPDATE: code {}/{}",
                            note.error_code, note.error_subcode
                        );
                        Some(Teardown::Protocol(note, reason))
                    }
                }
            }

            (_, BgpPdu::Notification(note)) => {
                self.peers[idx].counters.notifications_received += 1;
                Some(Teardown::PeerNotification(format!(
                    "peer sent NOTIFICATION: {}",
                    note.describe()
                )))
            }

            // Anything else is a finite state machine error (RFC 4271 section 6.5).
            (state, pdu) => {
                let reason = format!("{} is not valid in state {}", pdu.type_name(), state);
                Some(Teardown::Protocol(
                    BgpNotificationMessage::new(BGP_ERR_FSM, 0),
                    reason,
                ))
            }
        }
    }

    /// Validates a peer's OPEN against RFC 4271 section 6.2.
    fn validate_open(
        &self,
        idx: usize,
        open: &BgpOpenMessage,
    ) -> Result<(), BgpNotificationMessage> {
        if open.version != BGP_VERSION {
            let mut note = BgpNotificationMessage::new(
                crate::bgp::BGP_ERR_OPEN_MESSAGE,
                BGP_SUB_UNSUPPORTED_VERSION,
            );
            note.data = (BGP_VERSION as u16).to_be_bytes().to_vec();
            return Err(note);
        }
        if open.my_as != self.peers[idx].remote_as {
            return Err(BgpNotificationMessage::new(
                crate::bgp::BGP_ERR_OPEN_MESSAGE,
                BGP_SUB_BAD_PEER_AS,
            ));
        }
        // A BGP identifier must be a valid unicast host address and must differ from ours.
        if open.bgp_id.is_unspecified()
            || open.bgp_id.is_multicast()
            || open.bgp_id.is_broadcast()
            || open.bgp_id == self.router_id
        {
            return Err(BgpNotificationMessage::new(
                crate::bgp::BGP_ERR_OPEN_MESSAGE,
                BGP_SUB_BAD_BGP_IDENTIFIER,
            ));
        }
        if open.hold_time != 0 && open.hold_time < BGP_MIN_HOLD_TIME {
            return Err(BgpNotificationMessage::new(
                crate::bgp::BGP_ERR_OPEN_MESSAGE,
                BGP_SUB_UNACCEPTABLE_HOLD_TIME,
            ));
        }
        Ok(())
    }

    fn arm_hold_timer(&mut self, idx: usize, now_ms: u64) {
        let hold = self.peers[idx].negotiated_hold_ms;
        self.peers[idx].hold_deadline = if hold == 0 { None } else { Some(now_ms + hold) };
    }

    fn arm_keepalive_timer(&mut self, idx: usize, now_ms: u64) {
        let interval = self.peers[idx].keepalive_interval_ms;
        self.peers[idx].keepalive_deadline = if interval == 0 {
            None
        } else {
            Some(now_ms + interval)
        };
    }

    fn run_timers(&mut self, idx: usize, now_ms: u64, sockets: &mut SocketRuntime) {
        // HoldTimer: no message from the peer within the negotiated hold time.
        if self.peers[idx].hold_deadline.is_some_and(|d| now_ms >= d) {
            let note = BgpNotificationMessage::new(BGP_ERR_HOLD_TIMER_EXPIRED, 0);
            self.teardown(
                idx,
                now_ms,
                sockets,
                Teardown::Protocol(note, "HoldTimer expired".to_string()),
            );
            return;
        }

        // KeepaliveTimer: only meaningful once the session carries traffic.
        if matches!(
            self.peers[idx].state,
            BgpState::OpenConfirm | BgpState::Established
        ) && self.peers[idx]
            .keepalive_deadline
            .is_some_and(|d| now_ms >= d)
        {
            if self.send_pdu(idx, sockets, &BgpPdu::Keepalive) {
                self.peers[idx].counters.keepalives_sent += 1;
            }
            self.arm_keepalive_timer(idx, now_ms);
        }
    }

    /// Writes a message, but only if the send buffer can take all of it: a BGP
    /// message must never be split across a partial write.
    fn send_pdu(&mut self, idx: usize, sockets: &mut SocketRuntime, pdu: &BgpPdu) -> bool {
        let Some(stream) = self.peers[idx].stream else {
            return false;
        };
        let bytes = pdu.serialize();
        // Checking capacity first lets the caller retry the whole message later. The
        // alternative - writing a prefix now and the whole message again next time -
        // would put one header on the wire twice and desynchronise the peer.
        if sockets.tcp_writable(stream) < bytes.len() {
            return false;
        }
        match sockets.tcp_write(stream, &bytes) {
            Ok(n) if n == bytes.len() => true,
            Ok(_) => {
                // Unreachable while the capacity check above holds, but if it ever did
                // happen the stream would already carry half a message, and no retry
                // could repair it. Flag it so the session is reset instead.
                self.peers[idx].tx_desynced = true;
                false
            }
            Err(_) => false,
        }
    }

    /// Ends a session: tell the peer if we still can, drop the transport, purge
    /// everything learned from that peer, and go back to Idle with the
    /// ConnectRetryTimer armed.
    fn teardown(&mut self, idx: usize, now_ms: u64, sockets: &mut SocketRuntime, why: Teardown) {
        let addr = self.peers[idx].addr;
        let reason = match &why {
            Teardown::Transport(r) => r.clone(),
            Teardown::Protocol(_, r) => r.clone(),
            Teardown::PeerNotification(r) => r.clone(),
        };

        if let Teardown::Protocol(note, _) = &why
            && self.peers[idx].stream.is_some()
            && self.send_pdu(idx, sockets, &BgpPdu::Notification(note.clone()))
        {
            self.peers[idx].counters.notifications_sent += 1;
        }

        // Abort rather than close: the NOTIFICATION just queued is flushed first, then
        // the 4-tuple and the ephemeral port are reclaimed immediately. A graceful close
        // would leave a half-open connection behind on every failed reconnect attempt.
        if let Some(stream) = self.peers[idx].stream.take() {
            sockets.tcp_abort(stream, now_ms);
        }

        let purged = self.adj_rib_in.clear_peer(addr);
        self.adj_rib_out.clear_peer(addr);

        let peer = &mut self.peers[idx];
        peer.framer.reset();
        peer.state = BgpState::Idle;
        peer.hold_deadline = None;
        peer.keepalive_deadline = None;
        peer.negotiated_hold_ms = 0;
        peer.keepalive_interval_ms = 0;
        peer.remote_router_id = None;
        peer.established_since_ms = None;
        peer.tx_desynced = false;
        peer.last_error = Some(reason.clone());
        peer.connect_retry_deadline = Some(now_ms + self.connect_retry_ms);

        self.dirty = true;
        self.log(
            now_ms,
            addr,
            format!("session down ({}); purged {} learned paths", reason, purged),
        );
    }

    // ========================================================================
    // Import
    // ========================================================================

    /// Applies one received UPDATE to the Adj-RIB-In for this peer.
    ///
    /// Structural problems already caused a decode error before we got here. What is
    /// checked now is semantics: AS loops, usable NEXT_HOPs, and import policy. A route
    /// that fails those is discarded (it is not a protocol violation), whereas a NEXT_HOP
    /// that cannot be a host address is an UPDATE error and resets the session.
    fn import_update(
        &mut self,
        idx: usize,
        now_ms: u64,
        update: BgpUpdateMessage,
    ) -> Result<(), BgpNotificationMessage> {
        let addr = self.peers[idx].addr;

        for prefix in &update.withdrawn {
            if self.adj_rib_in.remove(addr, *prefix).is_some() {
                self.dirty = true;
                self.log(now_ms, addr, format!("withdrew {} from Adj-RIB-In", prefix));
            }
        }

        let Some(attrs) = update.attributes else {
            return Ok(());
        };
        if update.nlri.is_empty() {
            return Ok(());
        }

        // A NEXT_HOP that cannot be a unicast host address is an UPDATE error.
        if attrs.next_hop.is_unspecified()
            || attrs.next_hop.is_loopback()
            || attrs.next_hop.is_multicast()
            || attrs.next_hop.is_broadcast()
        {
            return Err(BgpNotificationMessage::new(
                BGP_ERR_UPDATE_MESSAGE,
                BGP_SUB_INVALID_NEXT_HOP,
            ));
        }

        let is_ebgp = self.peers[idx].remote_as != self.local_as;
        let peer_as = self.peers[idx].remote_as;

        // An UPDATE from an external peer has to say something truthful about where it
        // came from (RFC 4271 sections 6.3 and 9.1.2). Two separate rules:
        //
        //  * The AS_PATH must not be empty. This one is unconditional. A zero-length
        //    path wins step 2 of the decision process against every legitimate route,
        //    so a neighbour able to send one could take over any prefix it liked.
        //  * The path must lead with the neighbour's own ASN. This is the check
        //    vendors call "enforce-first-as"; it stops a peer disowning a path it is
        //    in fact carrying. It can be turned off per peer, the empty test cannot.
        //
        // Neither rule applies to an internal peer: an iBGP neighbour legitimately
        // passes on a path it did not originate, and a route originated inside this AS
        // carries an empty AS_PATH until it leaves.
        if is_ebgp {
            let refusal = if attrs.as_path.is_empty() {
                Some("AS_PATH is empty".to_string())
            } else if !self.peers[idx].enforce_first_as {
                None
            } else {
                match attrs.as_path.leading_as() {
                    Some(a) if a == peer_as => None,
                    Some(a) => Some(format!(
                        "AS_PATH [{}] leads with AS {}, not the neighbour's AS {}",
                        attrs.as_path, a, peer_as
                    )),
                    None => Some(format!(
                        "AS_PATH [{}] does not begin with an AS_SEQUENCE",
                        attrs.as_path
                    )),
                }
            };
            if let Some(reason) = refusal {
                self.peers[idx].counters.as_path_rejected += 1;
                self.log(now_ms, addr, format!("UPDATE refused: {}", reason));
                return Err(BgpNotificationMessage::new(
                    BGP_ERR_UPDATE_MESSAGE,
                    BGP_SUB_MALFORMED_AS_PATH,
                ));
            }
        }

        // AS loop: our own ASN already appears in the path, so the route has been
        // through this AS and must not be re-accepted (RFC 4271 section 9.1.2).
        if attrs.as_path.contains(self.local_as) {
            self.peers[idx].counters.as_loops_rejected += update.nlri.len() as u64;
            self.log(
                now_ms,
                addr,
                format!(
                    "rejected {} prefix(es): AS_PATH [{}] already contains AS {}",
                    update.nlri.len(),
                    attrs.as_path,
                    self.local_as
                ),
            );
            return Ok(());
        }

        let source = if is_ebgp {
            PathSource::Ebgp
        } else {
            PathSource::Ibgp
        };
        // A route learned over eBGP whose NEXT_HOP is our own session address would
        // point straight back at us; refuse it rather than build a forwarding loop.
        let own_addr = self.peers[idx].local_addr;
        let peer_router_id = self.peers[idx].remote_router_id.unwrap_or(addr);

        for prefix in update.nlri {
            if attrs.next_hop == own_addr {
                self.peers[idx].counters.next_hop_rejected += 1;
                continue;
            }

            let outcome = self.peers[idx].import_policy.apply(prefix);
            let (policy_lp, policy_med) = match outcome {
                PolicyOutcome::Denied => {
                    self.peers[idx].counters.policy_rejected += 1;
                    // A previously accepted path that policy now rejects must go.
                    if self.adj_rib_in.remove(addr, prefix).is_some() {
                        self.dirty = true;
                    }
                    continue;
                }
                PolicyOutcome::Permitted {
                    set_local_pref,
                    set_med,
                } => (set_local_pref, set_med),
            };

            let path = BgpPath {
                prefix,
                source,
                peer_addr: addr,
                peer_as,
                peer_router_id,
                origin: attrs.origin,
                as_path: attrs.as_path.clone(),
                next_hop: attrs.next_hop,
                med: policy_med.or(attrs.med),
                local_pref: policy_lp
                    .or(attrs.local_pref)
                    .unwrap_or(BGP_DEFAULT_LOCAL_PREF),
                atomic_aggregate: attrs.atomic_aggregate,
                received_at_ms: now_ms,
            };

            // Only a genuine change to the route reruns the decision process. An
            // identical re-advertisement still refreshes the stored path, so the
            // Adj-RIB-In timestamp tracks when this peer last spoke about it.
            let previous = self.adj_rib_in.insert(addr, path.clone());
            if previous.is_none_or(|prev| !prev.same_route_as(&path)) {
                self.dirty = true;
            }

            // A neighbour must not be able to exhaust memory by advertising forever.
            if self.adj_rib_in.prefix_count(addr) > self.peers[idx].max_prefixes {
                return Err(BgpNotificationMessage::new(
                    BGP_ERR_CEASE,
                    BGP_SUB_MAX_PREFIXES,
                ));
            }
        }

        self.log(
            now_ms,
            addr,
            format!(
                "Adj-RIB-In now holds {} prefix(es) from this peer",
                self.adj_rib_in.prefix_count(addr)
            ),
        );
        Ok(())
    }

    // ========================================================================
    // Decision process and FIB
    // ========================================================================

    /// Recomputes the Loc-RIB from the Adj-RIB-In tables plus the originated set.
    fn run_decision_process(&mut self, now_ms: u64) {
        self.decision_runs += 1;
        let mut new_rib = LocRib::new();

        let mut prefixes = self.adj_rib_in.prefixes();
        prefixes.extend(self.originated.keys().copied());

        for prefix in prefixes {
            let learned = self.adj_rib_in.candidates(prefix);
            let local = self
                .originated
                .get(&prefix)
                .map(|nh| BgpPath::local(prefix, *nh, self.router_id));

            let mut candidates: Vec<&BgpPath> = learned;
            if let Some(ref l) = local {
                candidates.push(l);
            }
            if let Some(best) = select_best(&candidates) {
                new_rib.insert(best.clone());
            }
        }

        let before: Vec<Ipv4Prefix> = self.loc_rib.prefixes();
        let after: Vec<Ipv4Prefix> = new_rib.prefixes();
        if before != after {
            self.log(
                now_ms,
                Ipv4Address::UNSPECIFIED,
                format!(
                    "decision process: Loc-RIB {} -> {} prefix(es)",
                    before.len(),
                    after.len()
                ),
            );
        }
        self.loc_rib = new_rib;
    }

    /// Pushes the Loc-RIB into the real forwarding table, and removes whatever it no
    /// longer contains. Only BGP-sourced entries are touched, so connected and static
    /// routes are never disturbed.
    fn sync_fib(&mut self, now_ms: u64, fib: &mut RoutingTable) {
        let mut desired: BTreeMap<Ipv4Prefix, (Ipv4Address, String)> = BTreeMap::new();
        let mut unresolved = BTreeSet::new();

        for (prefix, path) in self.loc_rib.iter() {
            // A locally originated prefix is already reachable through a connected or
            // static route; installing a BGP copy of it would add nothing.
            if path.is_local() {
                continue;
            }
            match Self::resolve_next_hop(fib, path.next_hop) {
                Some((next_hop, iface)) => {
                    desired.insert(*prefix, (next_hop, iface));
                }
                None => {
                    unresolved.insert(*prefix);
                }
            }
        }

        let stale: Vec<Ipv4Prefix> = self
            .installed
            .iter()
            .filter(|p| !desired.contains_key(p))
            .copied()
            .collect();
        for prefix in stale {
            if fib.remove_route(prefix.address, prefix.length, RouteSource::Bgp) {
                self.log(
                    now_ms,
                    Ipv4Address::UNSPECIFIED,
                    format!("FIB: removed {}", prefix),
                );
            }
            self.installed.remove(&prefix);
        }

        for (prefix, (next_hop, iface)) in &desired {
            let already = fib
                .routes_from(RouteSource::Bgp)
                .into_iter()
                .find(|r| r.destination == prefix.address && r.prefix_len == prefix.length)
                .is_some_and(|r| r.gateway == Some(*next_hop) && r.interface == *iface);
            if !already {
                fib.add_route_from(
                    prefix.address,
                    prefix.length,
                    Some(*next_hop),
                    iface,
                    RouteSource::Bgp,
                );
                self.log(
                    now_ms,
                    Ipv4Address::UNSPECIFIED,
                    format!("FIB: installed {} via {} dev {}", prefix, next_hop, iface),
                );
            }
            self.installed.insert(*prefix);
        }

        for prefix in &unresolved {
            if !self.unresolved.contains(prefix) {
                self.log(
                    now_ms,
                    Ipv4Address::UNSPECIFIED,
                    format!("best path for {} has an unresolvable NEXT_HOP", prefix),
                );
            }
        }
        self.unresolved = unresolved;
    }

    /// Resolves a BGP NEXT_HOP to `(forwarding next hop, egress interface)` using only
    /// non-BGP routes, so resolution can never recurse through another BGP route.
    fn resolve_next_hop(
        fib: &RoutingTable,
        next_hop: Ipv4Address,
    ) -> Option<(Ipv4Address, String)> {
        let route = fib
            .all_routes()
            .iter()
            .find(|r| r.source != RouteSource::Bgp && r.matches(next_hop))?;
        Some((route.gateway.unwrap_or(next_hop), route.interface.clone()))
    }

    /// Prefixes whose best path could not be resolved to an egress interface.
    pub fn unresolved_prefixes(&self) -> Vec<Ipv4Prefix> {
        self.unresolved.iter().copied().collect()
    }

    /// Prefixes this speaker currently has installed in the FIB.
    pub fn installed_prefixes(&self) -> Vec<Ipv4Prefix> {
        self.installed.iter().copied().collect()
    }

    // ========================================================================
    // Export
    // ========================================================================

    /// Computes what `idx` should be hearing and sends only the differences.
    fn advertise_to_peer(&mut self, idx: usize, now_ms: u64, sockets: &mut SocketRuntime) {
        if !self.peers[idx].is_established() {
            return;
        }
        let addr = self.peers[idx].addr;
        let desired = self.compute_adj_rib_out(idx);

        // Withdrawals: everything we previously advertised that is no longer desired.
        let withdrawn: Vec<Ipv4Prefix> = self
            .adj_rib_out
            .prefixes(addr)
            .into_iter()
            .filter(|p| !desired.contains_key(p))
            .collect();

        if !withdrawn.is_empty() {
            let pdu = BgpPdu::Update(BgpUpdateMessage::withdraw(withdrawn.clone()));
            if self.send_pdu(idx, sockets, &pdu) {
                self.peers[idx].counters.updates_sent += 1;
                for p in &withdrawn {
                    self.adj_rib_out.remove(addr, p);
                }
                self.log(
                    now_ms,
                    addr,
                    format!("advertised withdrawal of {} prefix(es)", withdrawn.len()),
                );
            }
        }

        // Announcements: new or changed routes, grouped by identical attribute set so
        // one UPDATE can carry several NLRI, exactly as a real speaker packs them.
        let mut groups: BTreeMap<Vec<u8>, (AdvertisedRoute, Vec<Ipv4Prefix>)> = BTreeMap::new();
        for (prefix, route) in &desired {
            if self.adj_rib_out.get(addr, prefix) == Some(route) {
                continue;
            }
            let attrs = Self::attributes_for(route);
            let key = attrs.encode();
            groups
                .entry(key)
                .or_insert_with(|| (route.clone(), Vec::new()))
                .1
                .push(*prefix);
        }

        for (_, (route, prefixes)) in groups {
            let attrs = Self::attributes_for(&route);
            let pdu = BgpPdu::Update(BgpUpdateMessage::announce(attrs, prefixes.clone()));
            if self.send_pdu(idx, sockets, &pdu) {
                self.peers[idx].counters.updates_sent += 1;
                for p in &prefixes {
                    self.adj_rib_out.insert(addr, *p, route.clone());
                }
                self.log(
                    now_ms,
                    addr,
                    format!(
                        "advertised {} prefix(es) with AS_PATH [{}] next-hop {}",
                        prefixes.len(),
                        route.as_path,
                        route.next_hop
                    ),
                );
            }
        }
    }

    fn attributes_for(route: &AdvertisedRoute) -> BgpPathAttributes {
        BgpPathAttributes {
            origin: route.origin,
            as_path: route.as_path.clone(),
            next_hop: route.next_hop,
            med: route.med,
            local_pref: route.local_pref,
            atomic_aggregate: false,
        }
    }

    /// Builds the outbound view of the Loc-RIB for one peer, applying split horizon,
    /// the iBGP re-advertisement rule, export policy, AS_PATH prepending, next-hop
    /// selection, and outbound loop prevention.
    fn compute_adj_rib_out(&self, idx: usize) -> BTreeMap<Ipv4Prefix, AdvertisedRoute> {
        let peer = &self.peers[idx];
        let is_ebgp_session = peer.remote_as != self.local_as;
        let mut out = BTreeMap::new();

        for (prefix, best) in self.loc_rib.iter() {
            // Never advertise a route back to the peer it came from.
            if best.peer_addr == peer.addr {
                continue;
            }
            // A route learned over iBGP is not re-advertised to another iBGP peer.
            if !is_ebgp_session && best.source == PathSource::Ibgp {
                continue;
            }

            let (policy_lp, policy_med) = match peer.export_policy.apply(*prefix) {
                PolicyOutcome::Denied => continue,
                PolicyOutcome::Permitted {
                    set_local_pref,
                    set_med,
                } => (set_local_pref, set_med),
            };

            let mut as_path = best.as_path.clone();
            if is_ebgp_session {
                as_path.prepend(self.local_as);
            }
            // Do not send a route into an AS that is already on its path; the peer
            // would only reject it as a loop.
            if as_path.contains(peer.remote_as) {
                continue;
            }

            // An eBGP peer must forward through our own address on the shared subnet,
            // never through whatever we were told. An iBGP peer keeps the original
            // NEXT_HOP unless next-hop-self is configured.
            let next_hop = if is_ebgp_session || peer.next_hop_self || best.is_local() {
                peer.local_addr
            } else {
                best.next_hop
            };

            let local_pref = if is_ebgp_session {
                // LOCAL_PREF is not sent to external peers (RFC 4271 section 5.1.5).
                None
            } else {
                Some(policy_lp.unwrap_or(best.local_pref))
            };

            let med = if is_ebgp_session {
                policy_med
            } else {
                policy_med.or(best.med)
            };

            out.insert(
                *prefix,
                AdvertisedRoute {
                    origin: best.origin,
                    as_path,
                    next_hop,
                    med,
                    local_pref,
                },
            );
        }

        out
    }

    // ========================================================================
    // Diagnostics
    // ========================================================================

    pub fn summaries(&self, now_ms: u64) -> Vec<BgpPeerSummary> {
        self.peers
            .iter()
            .map(|p| BgpPeerSummary {
                addr: p.addr,
                remote_as: p.remote_as,
                local_addr: p.local_addr,
                state: p.state,
                router_id: p.remote_router_id,
                uptime_ms: p.uptime_ms(now_ms),
                hold_ms: p.negotiated_hold_ms,
                hold_remaining_ms: p.hold_remaining_ms(now_ms),
                keepalive_interval_ms: p.keepalive_interval_ms,
                keepalive_remaining_ms: p.keepalive_remaining_ms(now_ms),
                prefixes_received: self.adj_rib_in.prefix_count(p.addr),
                prefixes_advertised: self.adj_rib_out.prefix_count(p.addr),
                counters: p.counters.clone(),
                last_error: p.last_error.clone(),
                establishment_count: p.establishment_count,
            })
            .collect()
    }

    /// `show bgp summary`
    pub fn format_summary(&self, now_ms: u64) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "BGP router identifier {}, local AS number {}\n",
            self.router_id, self.local_as
        ));
        s.push_str(&format!(
            "Loc-RIB {} prefix(es), Adj-RIB-In {} path(s), {} in FIB, {} decision run(s)\n",
            self.loc_rib.len(),
            self.adj_rib_in.path_count(),
            self.installed.len(),
            self.decision_runs
        ));
        s.push_str(
            "Neighbor          AS  State        Up(ms)  Hold  PfxRcd  PfxAdv  MsgRcd  MsgSent\n",
        );
        for p in self.summaries(now_ms) {
            let msg_rcd = p.counters.opens_received
                + p.counters.updates_received
                + p.counters.keepalives_received
                + p.counters.notifications_received;
            let msg_sent = p.counters.opens_sent
                + p.counters.updates_sent
                + p.counters.keepalives_sent
                + p.counters.notifications_sent;
            s.push_str(&format!(
                "{:<15} {:>5}  {:<11} {:>7} {:>5} {:>7} {:>7} {:>7} {:>8}\n",
                p.addr.to_string(),
                p.remote_as,
                p.state.as_str(),
                p.uptime_ms.map(|u| u.to_string()).unwrap_or("-".into()),
                p.hold_ms / 1_000,
                p.prefixes_received,
                p.prefixes_advertised,
                msg_rcd,
                msg_sent
            ));
        }
        s
    }

    /// `show bgp peers`
    pub fn format_peers(&self, now_ms: u64) -> String {
        let mut s = String::new();
        for p in self.summaries(now_ms) {
            s.push_str(&format!(
                "neighbor {} remote-as {} local-address {}\n",
                p.addr, p.remote_as, p.local_addr
            ));
            s.push_str(&format!(
                "  state {}  router-id {}  established {} time(s)\n",
                p.state,
                p.router_id.map(|r| r.to_string()).unwrap_or("-".into()),
                p.establishment_count
            ));
            s.push_str(&format!(
                "  uptime {}  hold {}ms (remaining {})  keepalive {}ms (remaining {})\n",
                p.uptime_ms
                    .map(|u| format!("{}ms", u))
                    .unwrap_or("down".into()),
                p.hold_ms,
                p.hold_remaining_ms
                    .map(|v| format!("{}ms", v))
                    .unwrap_or("n/a".into()),
                p.keepalive_interval_ms,
                p.keepalive_remaining_ms
                    .map(|v| format!("{}ms", v))
                    .unwrap_or("n/a".into()),
            ));
            s.push_str(&format!(
                "  prefixes received {}  advertised {}\n",
                p.prefixes_received, p.prefixes_advertised
            ));
            s.push_str(&format!(
                "  messages open {}/{} update {}/{} keepalive {}/{} notification {}/{} (rcvd/sent)\n",
                p.counters.opens_received,
                p.counters.opens_sent,
                p.counters.updates_received,
                p.counters.updates_sent,
                p.counters.keepalives_received,
                p.counters.keepalives_sent,
                p.counters.notifications_received,
                p.counters.notifications_sent
            ));
            s.push_str(&format!(
                "  discarded: as-loop {}  policy {}  next-hop {}  as-path {}\n",
                p.counters.as_loops_rejected,
                p.counters.policy_rejected,
                p.counters.next_hop_rejected,
                p.counters.as_path_rejected
            ));
            s.push_str(&format!(
                "  last error: {}\n",
                p.last_error.unwrap_or("none".into())
            ));
        }
        if s.is_empty() {
            s.push_str("no BGP neighbors configured\n");
        }
        s
    }

    /// `show bgp routes` - the Loc-RIB, i.e. the best path per prefix.
    pub fn format_routes(&self) -> String {
        let mut s = String::from(
            "Prefix              Next Hop         LocPrf  AS Path        Origin  Source  FIB\n",
        );
        for (prefix, path) in self.loc_rib.iter() {
            s.push_str(&format!(
                "{:<19} {:<16} {:>6}  {:<14} {:<6}  {:<6}  {}\n",
                prefix.to_string(),
                path.next_hop.to_string(),
                path.local_pref,
                path.as_path.to_string(),
                path.origin.to_string(),
                path.source.as_str(),
                if self.installed.contains(prefix) {
                    "yes"
                } else if path.is_local() {
                    "local"
                } else {
                    "no"
                }
            ));
        }
        s
    }

    /// `show bgp rib` - every path in the Adj-RIB-In, best paths marked.
    pub fn format_rib(&self) -> String {
        let mut s = String::from(
            "   Prefix              Peer             AS Path        LocPrf  MED   Origin\n",
        );
        for path in self.adj_rib_in.iter_paths() {
            let best = self
                .loc_rib
                .get(&path.prefix)
                .is_some_and(|b| b.peer_addr == path.peer_addr);
            s.push_str(&format!(
                "{}  {:<19} {:<16} {:<14} {:>6}  {:<5} {}\n",
                if best { ">" } else { " " },
                path.prefix.to_string(),
                path.peer_addr.to_string(),
                path.as_path.to_string(),
                path.local_pref,
                path.med.map(|m| m.to_string()).unwrap_or("-".into()),
                path.origin
            ));
        }
        for (prefix, next_hop) in &self.originated {
            s.push_str(&format!(
                ">  {:<19} {:<16} {:<14} {:>6}  {:<5} i (originated)\n",
                prefix.to_string(),
                next_hop.to_string(),
                "-",
                BGP_DEFAULT_LOCAL_PREF,
                "-"
            ));
        }
        s
    }
}
