//! Layer 4: Transmission Control Protocol (TCP - RFC 793, RFC 9293).
//!
//! Connection-oriented, reliable transport protocol with TCP Option parsing,
//! out-of-order segment reassembly, Congestion Control (RFC 5681), and full finite-state machine.

use crate::checksum::{compute_ipv4_transport_checksum, verify_ipv4_transport_checksum};
use crate::congestion::{CongestionControl, RttEstimator};
use crate::ipv4::Ipv4Address;
use std::collections::{BTreeMap, HashMap};
use std::fmt;

pub const TCP_MIN_HEADER_LEN: usize = 20;

// Flag bitmasks
pub const TCP_FLAG_FIN: u8 = 0x01;
pub const TCP_FLAG_SYN: u8 = 0x02;
pub const TCP_FLAG_RST: u8 = 0x04;
pub const TCP_FLAG_PSH: u8 = 0x08;
pub const TCP_FLAG_ACK: u8 = 0x10;
pub const TCP_FLAG_URG: u8 = 0x20;

// TCP Option Kinds (RFC 793, RFC 7323)
pub const TCP_OPT_EOL: u8 = 0;
pub const TCP_OPT_NOP: u8 = 1;
pub const TCP_OPT_MSS: u8 = 2;
pub const TCP_OPT_WSCALE: u8 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TcpOption {
    EndOfOptions,
    Nop,
    Mss(u16),
    WindowScale(u8),
    Unknown { kind: u8, data: Vec<u8> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TcpFlags {
    pub fin: bool,
    pub syn: bool,
    pub rst: bool,
    pub psh: bool,
    pub ack: bool,
    pub urg: bool,
}

impl TcpFlags {
    pub fn from_u8(val: u8) -> Self {
        TcpFlags {
            fin: (val & TCP_FLAG_FIN) != 0,
            syn: (val & TCP_FLAG_SYN) != 0,
            rst: (val & TCP_FLAG_RST) != 0,
            psh: (val & TCP_FLAG_PSH) != 0,
            ack: (val & TCP_FLAG_ACK) != 0,
            urg: (val & TCP_FLAG_URG) != 0,
        }
    }

    pub fn to_u8(&self) -> u8 {
        let mut val = 0u8;
        if self.fin {
            val |= TCP_FLAG_FIN;
        }
        if self.syn {
            val |= TCP_FLAG_SYN;
        }
        if self.rst {
            val |= TCP_FLAG_RST;
        }
        if self.psh {
            val |= TCP_FLAG_PSH;
        }
        if self.ack {
            val |= TCP_FLAG_ACK;
        }
        if self.urg {
            val |= TCP_FLAG_URG;
        }
        val
    }

    pub fn syn_ack() -> Self {
        TcpFlags {
            syn: true,
            ack: true,
            ..Default::default()
        }
    }

    pub fn ack() -> Self {
        TcpFlags {
            ack: true,
            ..Default::default()
        }
    }

    pub fn syn() -> Self {
        TcpFlags {
            syn: true,
            ..Default::default()
        }
    }

    pub fn fin_ack() -> Self {
        TcpFlags {
            fin: true,
            ack: true,
            ..Default::default()
        }
    }

    pub fn rst() -> Self {
        TcpFlags {
            rst: true,
            ..Default::default()
        }
    }
}

impl fmt::Display for TcpFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut flags = Vec::new();
        if self.syn {
            flags.push("SYN");
        }
        if self.ack {
            flags.push("ACK");
        }
        if self.fin {
            flags.push("FIN");
        }
        if self.rst {
            flags.push("RST");
        }
        if self.psh {
            flags.push("PSH");
        }
        if self.urg {
            flags.push("URG");
        }
        if flags.is_empty() {
            write!(f, "[NONE]")
        } else {
            write!(f, "[{}]", flags.join("|"))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcpSegment<'a> {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub data_offset: u8, // in 32-bit words (min 5 = 20 bytes)
    pub flags: TcpFlags,
    pub window_size: u16,
    pub checksum: u16,
    pub urgent_ptr: u16,
    pub options: Vec<TcpOption>,
    pub payload: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TcpError {
    SegmentTooShort(usize),
    InvalidDataOffset(u8),
    DataOffsetExceedsLength {
        offset_bytes: usize,
        available: usize,
    },
    InvalidChecksum {
        found: u16,
    },
}

impl fmt::Display for TcpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TcpError::SegmentTooShort(len) => {
                write!(f, "TCP segment too short ({} bytes, min 20)", len)
            }
            TcpError::InvalidDataOffset(d) => write!(f, "Invalid TCP data offset: {} (min 5)", d),
            TcpError::DataOffsetExceedsLength {
                offset_bytes,
                available,
            } => {
                write!(
                    f,
                    "TCP header offset {} exceeds segment length {}",
                    offset_bytes, available
                )
            }
            TcpError::InvalidChecksum { found } => {
                write!(
                    f,
                    "TCP checksum mismatch with checksum field 0x{:04x}",
                    found
                )
            }
        }
    }
}

impl std::error::Error for TcpError {}

impl<'a> TcpSegment<'a> {
    pub fn parse(
        src_ip: Ipv4Address,
        dst_ip: Ipv4Address,
        data: &'a [u8],
        check_checksum: bool,
    ) -> Result<Self, TcpError> {
        if data.len() < TCP_MIN_HEADER_LEN {
            return Err(TcpError::SegmentTooShort(data.len()));
        }

        let src_port = u16::from_be_bytes([data[0], data[1]]);
        let dst_port = u16::from_be_bytes([data[2], data[3]]);
        let seq_num = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let ack_num = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

        let data_offset = data[12] >> 4;
        if data_offset < 5 {
            return Err(TcpError::InvalidDataOffset(data_offset));
        }

        let offset_bytes = (data_offset as usize) * 4;
        if offset_bytes > data.len() {
            return Err(TcpError::DataOffsetExceedsLength {
                offset_bytes,
                available: data.len(),
            });
        }

        let flags_raw = data[13];
        let flags = TcpFlags::from_u8(flags_raw);
        let window_size = u16::from_be_bytes([data[14], data[15]]);
        let checksum = u16::from_be_bytes([data[16], data[17]]);
        let urgent_ptr = u16::from_be_bytes([data[18], data[19]]);

        if check_checksum && !verify_ipv4_transport_checksum(src_ip.0, dst_ip.0, 6, data) {
            return Err(TcpError::InvalidChecksum { found: checksum });
        }

        // Parse TCP Options (between byte 20 and offset_bytes)
        let mut options = Vec::new();
        let mut opt_offset = TCP_MIN_HEADER_LEN;
        while opt_offset < offset_bytes {
            let kind = data[opt_offset];
            if kind == TCP_OPT_EOL {
                options.push(TcpOption::EndOfOptions);
                break;
            }
            if kind == TCP_OPT_NOP {
                options.push(TcpOption::Nop);
                opt_offset += 1;
                continue;
            }

            if opt_offset + 1 >= offset_bytes {
                break;
            }
            let len = data[opt_offset + 1] as usize;
            if len < 2 || opt_offset + len > offset_bytes {
                break;
            }

            match kind {
                TCP_OPT_MSS if len == 4 => {
                    let mss = u16::from_be_bytes([data[opt_offset + 2], data[opt_offset + 3]]);
                    options.push(TcpOption::Mss(mss));
                }
                TCP_OPT_WSCALE if len == 3 => {
                    options.push(TcpOption::WindowScale(data[opt_offset + 2]));
                }
                other => {
                    let opt_data = data[opt_offset + 2..opt_offset + len].to_vec();
                    options.push(TcpOption::Unknown {
                        kind: other,
                        data: opt_data,
                    });
                }
            }
            opt_offset += len;
        }

        let payload = &data[offset_bytes..];

        Ok(TcpSegment {
            src_port,
            dst_port,
            seq_num,
            ack_num,
            data_offset,
            flags,
            window_size,
            checksum,
            urgent_ptr,
            options,
            payload,
        })
    }

    pub fn serialize(
        src_ip: Ipv4Address,
        dst_ip: Ipv4Address,
        src_port: u16,
        dst_port: u16,
        seq_num: u32,
        ack_num: u32,
        flags: TcpFlags,
        window_size: u16,
        payload: &[u8],
    ) -> Vec<u8> {
        Self::serialize_with_options(
            src_ip,
            dst_ip,
            src_port,
            dst_port,
            seq_num,
            ack_num,
            flags,
            window_size,
            &[],
            payload,
        )
    }

    pub fn serialize_with_options(
        src_ip: Ipv4Address,
        dst_ip: Ipv4Address,
        src_port: u16,
        dst_port: u16,
        seq_num: u32,
        ack_num: u32,
        flags: TcpFlags,
        window_size: u16,
        options: &[TcpOption],
        payload: &[u8],
    ) -> Vec<u8> {
        let mut opt_bytes = Vec::new();
        for opt in options {
            match opt {
                TcpOption::EndOfOptions => opt_bytes.push(TCP_OPT_EOL),
                TcpOption::Nop => opt_bytes.push(TCP_OPT_NOP),
                TcpOption::Mss(mss) => {
                    opt_bytes.push(TCP_OPT_MSS);
                    opt_bytes.push(4);
                    opt_bytes.extend_from_slice(&mss.to_be_bytes());
                }
                TcpOption::WindowScale(scale) => {
                    opt_bytes.push(TCP_OPT_WSCALE);
                    opt_bytes.push(3);
                    opt_bytes.push(*scale);
                }
                TcpOption::Unknown { kind, data } => {
                    opt_bytes.push(*kind);
                    opt_bytes.push((data.len() + 2) as u8);
                    opt_bytes.extend_from_slice(data);
                }
            }
        }

        // Pad options to multiple of 4 bytes
        while opt_bytes.len() % 4 != 0 {
            opt_bytes.push(TCP_OPT_NOP);
        }

        let header_len = TCP_MIN_HEADER_LEN + opt_bytes.len();
        let data_offset = (header_len / 4) as u8;
        let total_len = header_len + payload.len();
        let mut buf = Vec::with_capacity(total_len);

        buf.extend_from_slice(&src_port.to_be_bytes());
        buf.extend_from_slice(&dst_port.to_be_bytes());
        buf.extend_from_slice(&seq_num.to_be_bytes());
        buf.extend_from_slice(&ack_num.to_be_bytes());
        buf.push((data_offset << 4) & 0xF0);
        buf.push(flags.to_u8());
        buf.extend_from_slice(&window_size.to_be_bytes());
        buf.extend_from_slice(&[0x00, 0x00]); // Checksum placeholder
        buf.extend_from_slice(&0u16.to_be_bytes()); // Urgent pointer
        buf.extend_from_slice(&opt_bytes);
        buf.extend_from_slice(payload);

        let csum = compute_ipv4_transport_checksum(src_ip.0, dst_ip.0, 6, &buf);
        buf[16..18].copy_from_slice(&csum.to_be_bytes());

        buf
    }
}

/// TCP Connection States (RFC 793)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

impl fmt::Display for TcpState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TcpState::Closed => write!(f, "CLOSED"),
            TcpState::Listen => write!(f, "LISTEN"),
            TcpState::SynSent => write!(f, "SYN_SENT"),
            TcpState::SynReceived => write!(f, "SYN_RECEIVED"),
            TcpState::Established => write!(f, "ESTABLISHED"),
            TcpState::FinWait1 => write!(f, "FIN_WAIT_1"),
            TcpState::FinWait2 => write!(f, "FIN_WAIT_2"),
            TcpState::CloseWait => write!(f, "CLOSE_WAIT"),
            TcpState::Closing => write!(f, "CLOSING"),
            TcpState::LastAck => write!(f, "LAST_ACK"),
            TcpState::TimeWait => write!(f, "TIME_WAIT"),
        }
    }
}

/// 4-tuple Socket Key: (Local IP, Local Port, Remote IP, Remote Port)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SocketAddrV4 {
    pub ip: Ipv4Address,
    pub port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TcpConnectionKey {
    pub local: SocketAddrV4,
    pub remote: SocketAddrV4,
}

/// Manages a single TCP connection state machine, out-of-order reassembly queue, and Congestion Control
#[derive(Debug, Clone)]
pub struct TcpConnection {
    pub local: SocketAddrV4,
    pub remote: SocketAddrV4,
    pub state: TcpState,
    pub snd_una: u32,
    pub snd_nxt: u32,
    pub snd_wnd: u16,
    pub rcv_nxt: u32,
    pub rcv_wnd: u16,
    pub peer_mss: u16,
    pub rx_buffer: Vec<u8>,
    pub ooo_queue: BTreeMap<u32, Vec<u8>>, // Out-of-order segment buffer: SeqNum -> Payload
    pub congestion: CongestionControl,
    pub rtt: RttEstimator,
}

impl TcpConnection {
    pub fn new_server(local: SocketAddrV4, remote: SocketAddrV4, isn: u32) -> Self {
        let mss = 1460;
        TcpConnection {
            local,
            remote,
            state: TcpState::Listen,
            snd_una: isn,
            snd_nxt: isn,
            snd_wnd: 65535,
            rcv_nxt: 0,
            rcv_wnd: 65535,
            peer_mss: mss,
            rx_buffer: Vec::new(),
            ooo_queue: BTreeMap::new(),
            congestion: CongestionControl::new(mss as u32),
            rtt: RttEstimator::new(),
        }
    }

    pub fn new_client(local: SocketAddrV4, remote: SocketAddrV4, isn: u32) -> Self {
        let mss = 1460;
        TcpConnection {
            local,
            remote,
            state: TcpState::Closed,
            snd_una: isn,
            snd_nxt: isn,
            snd_wnd: 65535,
            rcv_nxt: 0,
            rcv_wnd: 65535,
            peer_mss: mss,
            rx_buffer: Vec::new(),
            ooo_queue: BTreeMap::new(),
            congestion: CongestionControl::new(mss as u32),
            rtt: RttEstimator::new(),
        }
    }

    /// Client initiates active connection opening (sends SYN)
    pub fn initiate_syn(&mut self) -> Vec<u8> {
        self.state = TcpState::SynSent;
        let syn_seq = self.snd_nxt;
        self.snd_nxt = self.snd_nxt.wrapping_add(1);

        let options = vec![TcpOption::Mss(1460)];
        TcpSegment::serialize_with_options(
            self.local.ip,
            self.remote.ip,
            self.local.port,
            self.remote.port,
            syn_seq,
            0,
            TcpFlags::syn(),
            self.rcv_wnd,
            &options,
            &[],
        )
    }

    /// Client or Server sends application data
    pub fn send_data(&mut self, payload: &[u8]) -> Option<Vec<u8>> {
        if self.state != TcpState::Established {
            return None;
        }
        let seq = self.snd_nxt;
        self.snd_nxt = self.snd_nxt.wrapping_add(payload.len() as u32);

        let mut flags = TcpFlags::ack();
        flags.psh = true;

        Some(TcpSegment::serialize(
            self.local.ip,
            self.remote.ip,
            self.local.port,
            self.remote.port,
            seq,
            self.rcv_nxt,
            flags,
            self.rcv_wnd,
            payload,
        ))
    }

    /// Initiates active connection teardown (sends FIN)
    pub fn initiate_close(&mut self) -> Option<Vec<u8>> {
        match self.state {
            TcpState::Established => {
                self.state = TcpState::FinWait1;
                let fin_seq = self.snd_nxt;
                self.snd_nxt = self.snd_nxt.wrapping_add(1);

                Some(TcpSegment::serialize(
                    self.local.ip,
                    self.remote.ip,
                    self.local.port,
                    self.remote.port,
                    fin_seq,
                    self.rcv_nxt,
                    TcpFlags::fin_ack(),
                    self.rcv_wnd,
                    &[],
                ))
            }
            TcpState::CloseWait => {
                self.state = TcpState::LastAck;
                let fin_seq = self.snd_nxt;
                self.snd_nxt = self.snd_nxt.wrapping_add(1);

                Some(TcpSegment::serialize(
                    self.local.ip,
                    self.remote.ip,
                    self.local.port,
                    self.remote.port,
                    fin_seq,
                    self.rcv_nxt,
                    TcpFlags::fin_ack(),
                    self.rcv_wnd,
                    &[],
                ))
            }
            _ => None,
        }
    }

    /// Handles an incoming TCP segment, updates state machine and reassembly queue, and generates response.
    pub fn handle_segment(&mut self, seg: &TcpSegment<'_>) -> Option<Vec<u8>> {
        // Inspect options for MSS
        for opt in &seg.options {
            if let TcpOption::Mss(m) = opt {
                self.peer_mss = *m;
                self.congestion.mss = *m as u32;
            }
        }

        match self.state {
            TcpState::Listen => {
                if seg.flags.syn {
                    self.rcv_nxt = seg.seq_num.wrapping_add(1);
                    let my_syn_seq = self.snd_nxt;
                    self.snd_nxt = self.snd_nxt.wrapping_add(1);
                    self.state = TcpState::SynReceived;

                    // Send SYN-ACK with our MSS option (1460)
                    let options = vec![TcpOption::Mss(1460)];
                    let syn_ack = TcpSegment::serialize_with_options(
                        self.local.ip,
                        self.remote.ip,
                        self.local.port,
                        self.remote.port,
                        my_syn_seq,
                        self.rcv_nxt,
                        TcpFlags::syn_ack(),
                        self.rcv_wnd,
                        &options,
                        &[],
                    );
                    Some(syn_ack)
                } else {
                    None
                }
            }

            TcpState::SynSent => {
                if seg.flags.syn && seg.flags.ack {
                    if seg.ack_num == self.snd_nxt {
                        self.rcv_nxt = seg.seq_num.wrapping_add(1);
                        self.snd_una = seg.ack_num;
                        self.state = TcpState::Established;

                        // Send ACK to complete 3-way handshake
                        let ack = TcpSegment::serialize(
                            self.local.ip,
                            self.remote.ip,
                            self.local.port,
                            self.remote.port,
                            self.snd_nxt,
                            self.rcv_nxt,
                            TcpFlags::ack(),
                            self.rcv_wnd,
                            &[],
                        );
                        Some(ack)
                    } else {
                        None
                    }
                } else if seg.flags.syn {
                    // Simultaneous open
                    self.rcv_nxt = seg.seq_num.wrapping_add(1);
                    self.state = TcpState::SynReceived;
                    let syn_ack = TcpSegment::serialize(
                        self.local.ip,
                        self.remote.ip,
                        self.local.port,
                        self.remote.port,
                        self.snd_nxt,
                        self.rcv_nxt,
                        TcpFlags::syn_ack(),
                        self.rcv_wnd,
                        &[],
                    );
                    Some(syn_ack)
                } else {
                    None
                }
            }

            TcpState::SynReceived => {
                if seg.flags.ack && seg.ack_num == self.snd_nxt {
                    self.snd_una = seg.ack_num;
                    self.state = TcpState::Established;

                    // If ACK also carried data
                    if !seg.payload.is_empty() {
                        self.rx_buffer.extend_from_slice(seg.payload);
                        self.rcv_nxt = self.rcv_nxt.wrapping_add(seg.payload.len() as u32);
                        let ack = TcpSegment::serialize(
                            self.local.ip,
                            self.remote.ip,
                            self.local.port,
                            self.remote.port,
                            self.snd_nxt,
                            self.rcv_nxt,
                            TcpFlags::ack(),
                            self.rcv_wnd,
                            &[],
                        );
                        return Some(ack);
                    }
                    None
                } else {
                    None
                }
            }

            TcpState::Established => {
                if seg.flags.rst {
                    self.state = TcpState::Closed;
                    return None;
                }

                if seg.flags.fin {
                    self.rcv_nxt = seg.seq_num.wrapping_add(1);
                    let fin_seq = self.snd_nxt;
                    self.snd_nxt = self.snd_nxt.wrapping_add(1);
                    self.state = TcpState::LastAck;

                    // Send FIN-ACK to complete passive teardown
                    let fin_ack = TcpSegment::serialize(
                        self.local.ip,
                        self.remote.ip,
                        self.local.port,
                        self.remote.port,
                        fin_seq,
                        self.rcv_nxt,
                        TcpFlags::fin_ack(),
                        self.rcv_wnd,
                        &[],
                    );
                    return Some(fin_ack);
                }

                // Process ACKs for Congestion Control
                if seg.flags.ack {
                    if seg.ack_num > self.snd_una {
                        let bytes_acked = seg.ack_num - self.snd_una;
                        self.snd_una = seg.ack_num;
                        self.congestion.on_ack(bytes_acked);
                    } else if seg.ack_num == self.snd_una && seg.payload.is_empty() {
                        self.congestion.on_dup_ack();
                    }
                }

                if !seg.payload.is_empty() {
                    let seq = seg.seq_num;
                    if seq == self.rcv_nxt {
                        // In-order segment
                        self.rx_buffer.extend_from_slice(seg.payload);
                        self.rcv_nxt = self.rcv_nxt.wrapping_add(seg.payload.len() as u32);

                        // Check if previously buffered out-of-order segments can now be assembled
                        while let Some((&next_seq, _)) = self.ooo_queue.iter().next() {
                            if next_seq == self.rcv_nxt {
                                let payload = self.ooo_queue.remove(&next_seq).unwrap();
                                self.rcv_nxt = self.rcv_nxt.wrapping_add(payload.len() as u32);
                                self.rx_buffer.extend_from_slice(&payload);
                            } else {
                                break;
                            }
                        }
                    } else if seq > self.rcv_nxt {
                        // Out-of-order segment -> Buffer in queue
                        self.ooo_queue.insert(seq, seg.payload.to_vec());
                    }

                    // Transmit ACK reflecting current contiguous rcv_nxt
                    let ack = TcpSegment::serialize(
                        self.local.ip,
                        self.remote.ip,
                        self.local.port,
                        self.remote.port,
                        self.snd_nxt,
                        self.rcv_nxt,
                        TcpFlags::ack(),
                        self.rcv_wnd,
                        &[],
                    );
                    Some(ack)
                } else if seg.flags.ack {
                    None
                } else {
                    None
                }
            }

            TcpState::FinWait1 => {
                let acked_our_fin = seg.flags.ack && seg.ack_num == self.snd_nxt;
                if acked_our_fin {
                    self.snd_una = seg.ack_num;
                    if seg.flags.fin {
                        // Received simultaneous FIN and ACK for our FIN
                        self.rcv_nxt = seg.seq_num.wrapping_add(1);
                        self.state = TcpState::TimeWait;
                        let ack = TcpSegment::serialize(
                            self.local.ip,
                            self.remote.ip,
                            self.local.port,
                            self.remote.port,
                            self.snd_nxt,
                            self.rcv_nxt,
                            TcpFlags::ack(),
                            self.rcv_wnd,
                            &[],
                        );
                        Some(ack)
                    } else {
                        self.state = TcpState::FinWait2;
                        None
                    }
                } else if seg.flags.fin {
                    // Simultaneous close
                    self.rcv_nxt = seg.seq_num.wrapping_add(1);
                    self.state = TcpState::Closing;
                    let ack = TcpSegment::serialize(
                        self.local.ip,
                        self.remote.ip,
                        self.local.port,
                        self.remote.port,
                        self.snd_nxt,
                        self.rcv_nxt,
                        TcpFlags::ack(),
                        self.rcv_wnd,
                        &[],
                    );
                    Some(ack)
                } else {
                    None
                }
            }

            TcpState::FinWait2 => {
                if seg.flags.fin {
                    self.rcv_nxt = seg.seq_num.wrapping_add(1);
                    self.state = TcpState::TimeWait;

                    let ack = TcpSegment::serialize(
                        self.local.ip,
                        self.remote.ip,
                        self.local.port,
                        self.remote.port,
                        self.snd_nxt,
                        self.rcv_nxt,
                        TcpFlags::ack(),
                        self.rcv_wnd,
                        &[],
                    );
                    Some(ack)
                } else {
                    None
                }
            }

            TcpState::CloseWait => None,

            TcpState::Closing => {
                if seg.flags.ack && seg.ack_num == self.snd_nxt {
                    self.state = TcpState::TimeWait;
                }
                None
            }

            TcpState::LastAck => {
                if seg.flags.ack && seg.ack_num == self.snd_nxt {
                    self.state = TcpState::Closed;
                }
                None
            }

            TcpState::TimeWait => {
                // In simulated environment, TimeWait can transition to Closed or ignore redundant packets
                None
            }

            TcpState::Closed => None,
        }
    }
}

/// TCP Connection Manager
#[derive(Default)]
pub struct TcpManager {
    pub listeners: HashMap<u16, u32>, // port -> next ISN
    pub connections: HashMap<TcpConnectionKey, TcpConnection>,
}

impl TcpManager {
    pub fn new() -> Self {
        TcpManager {
            listeners: HashMap::new(),
            connections: HashMap::new(),
        }
    }

    pub fn listen(&mut self, port: u16) {
        self.listeners.insert(port, 1000);
    }

    pub fn connect(&mut self, local: SocketAddrV4, remote: SocketAddrV4, isn: u32) -> Vec<u8> {
        let key = TcpConnectionKey { local, remote };
        let mut conn = TcpConnection::new_client(local, remote, isn);
        let syn_packet = conn.initiate_syn();
        self.connections.insert(key, conn);
        syn_packet
    }

    pub fn send_data(
        &mut self,
        local: SocketAddrV4,
        remote: SocketAddrV4,
        data: &[u8],
    ) -> Option<Vec<u8>> {
        let key = TcpConnectionKey { local, remote };
        if let Some(conn) = self.connections.get_mut(&key) {
            conn.send_data(data)
        } else {
            None
        }
    }

    pub fn close(&mut self, local: SocketAddrV4, remote: SocketAddrV4) -> Option<Vec<u8>> {
        let key = TcpConnectionKey { local, remote };
        if let Some(conn) = self.connections.get_mut(&key) {
            conn.initiate_close()
        } else {
            None
        }
    }

    pub fn get_connection(
        &self,
        local: SocketAddrV4,
        remote: SocketAddrV4,
    ) -> Option<&TcpConnection> {
        let key = TcpConnectionKey { local, remote };
        self.connections.get(&key)
    }

    pub fn get_connection_mut(
        &mut self,
        local: SocketAddrV4,
        remote: SocketAddrV4,
    ) -> Option<&mut TcpConnection> {
        let key = TcpConnectionKey { local, remote };
        self.connections.get_mut(&key)
    }

    pub fn process_segment(
        &mut self,
        src_ip: Ipv4Address,
        dst_ip: Ipv4Address,
        seg: &TcpSegment<'_>,
    ) -> Option<Vec<u8>> {
        let key = TcpConnectionKey {
            local: SocketAddrV4 {
                ip: dst_ip,
                port: seg.dst_port,
            },
            remote: SocketAddrV4 {
                ip: src_ip,
                port: seg.src_port,
            },
        };

        if let Some(conn) = self.connections.get_mut(&key) {
            return conn.handle_segment(seg);
        }

        // Check if port is listening
        if let Some(isn) = self.listeners.get_mut(&seg.dst_port)
            && seg.flags.syn
        {
            let mut conn = TcpConnection::new_server(key.local, key.remote, *isn);
            *isn = isn.wrapping_add(1000);
            let resp = conn.handle_segment(seg);
            self.connections.insert(key, conn);
            return resp;
        }

        // Port closed -> send RST
        if !seg.flags.rst {
            let rst_seq = if seg.flags.ack { seg.ack_num } else { 0 };
            let rst_ack = seg.seq_num.wrapping_add(if seg.flags.syn || seg.flags.fin {
                1
            } else {
                seg.payload.len() as u32
            });
            let mut flags = TcpFlags::rst();
            if !seg.flags.ack {
                flags.ack = true;
            }
            return Some(TcpSegment::serialize(
                dst_ip,
                src_ip,
                seg.dst_port,
                seg.src_port,
                rst_seq,
                rst_ack,
                flags,
                0,
                &[],
            ));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcp_options_mss_parsing() {
        let src_ip = Ipv4Address::new(10, 0, 0, 1);
        let dst_ip = Ipv4Address::new(10, 0, 0, 2);
        let options = vec![TcpOption::Mss(1400), TcpOption::WindowScale(7)];

        let raw = TcpSegment::serialize_with_options(
            src_ip,
            dst_ip,
            12345,
            80,
            100,
            0,
            TcpFlags::syn(),
            65535,
            &options,
            &[],
        );

        let parsed = TcpSegment::parse(src_ip, dst_ip, &raw, true).unwrap();
        assert_eq!(parsed.options.len(), 3);
        assert_eq!(parsed.options[0], TcpOption::Mss(1400));
        assert_eq!(parsed.options[1], TcpOption::WindowScale(7));
        assert_eq!(parsed.options[2], TcpOption::Nop);
    }

    #[test]
    fn test_tcp_out_of_order_reassembly() {
        let mut conn = TcpConnection::new_server(
            SocketAddrV4 {
                ip: Ipv4Address::new(10, 0, 0, 1),
                port: 80,
            },
            SocketAddrV4 {
                ip: Ipv4Address::new(10, 0, 0, 2),
                port: 50000,
            },
            1000,
        );
        conn.state = TcpState::Established;
        conn.rcv_nxt = 100;

        // 1. Receive segment B (Seq 105..110) out of order
        let seg_b = TcpSegment {
            src_port: 50000,
            dst_port: 80,
            seq_num: 105,
            ack_num: 1000,
            data_offset: 5,
            flags: TcpFlags::ack(),
            window_size: 65535,
            checksum: 0,
            urgent_ptr: 0,
            options: vec![],
            payload: b"WORLD",
        };
        conn.handle_segment(&seg_b);
        assert_eq!(conn.rcv_nxt, 100); // Still waiting for seq 100
        assert_eq!(conn.rx_buffer.len(), 0);

        // 2. Receive segment A (Seq 100..105) in order
        let seg_a = TcpSegment {
            src_port: 50000,
            dst_port: 80,
            seq_num: 100,
            ack_num: 1000,
            data_offset: 5,
            flags: TcpFlags::ack(),
            window_size: 65535,
            checksum: 0,
            urgent_ptr: 0,
            options: vec![],
            payload: b"HELLO",
        };
        conn.handle_segment(&seg_a);

        // Both segment A and buffered segment B should now be assembled
        assert_eq!(conn.rcv_nxt, 100 + 5 + 5);
        assert_eq!(conn.rx_buffer, b"HELLOWORLD");
    }

    #[test]
    fn test_tcp_client_server_full_lifecycle() {
        let client_ip = Ipv4Address::new(192, 168, 1, 10);
        let server_ip = Ipv4Address::new(192, 168, 1, 20);
        let client_port = 45000;
        let server_port = 80;

        let mut client_mgr = TcpManager::new();
        let mut server_mgr = TcpManager::new();
        server_mgr.listen(server_port);

        let client_sock = SocketAddrV4 {
            ip: client_ip,
            port: client_port,
        };
        let server_sock = SocketAddrV4 {
            ip: server_ip,
            port: server_port,
        };

        // 1. Client sends SYN
        let syn_bytes = client_mgr.connect(client_sock, server_sock, 1000);
        let syn_seg = TcpSegment::parse(client_ip, server_ip, &syn_bytes, true).unwrap();
        assert_eq!(syn_seg.flags, TcpFlags::syn());
        assert_eq!(
            client_mgr
                .get_connection(client_sock, server_sock)
                .unwrap()
                .state,
            TcpState::SynSent
        );

        // 2. Server processes SYN, sends SYN-ACK
        let syn_ack_bytes = server_mgr
            .process_segment(client_ip, server_ip, &syn_seg)
            .unwrap();
        let syn_ack_seg = TcpSegment::parse(server_ip, client_ip, &syn_ack_bytes, true).unwrap();
        assert_eq!(syn_ack_seg.flags, TcpFlags::syn_ack());
        assert_eq!(
            server_mgr
                .get_connection(server_sock, client_sock)
                .unwrap()
                .state,
            TcpState::SynReceived
        );

        // 3. Client processes SYN-ACK, sends ACK -> ESTABLISHED
        let ack_bytes = client_mgr
            .process_segment(server_ip, client_ip, &syn_ack_seg)
            .unwrap();
        let ack_seg = TcpSegment::parse(client_ip, server_ip, &ack_bytes, true).unwrap();
        assert_eq!(ack_seg.flags, TcpFlags::ack());
        assert_eq!(
            client_mgr
                .get_connection(client_sock, server_sock)
                .unwrap()
                .state,
            TcpState::Established
        );

        // 4. Server processes ACK -> ESTABLISHED
        let server_resp = server_mgr.process_segment(client_ip, server_ip, &ack_seg);
        assert!(server_resp.is_none());
        assert_eq!(
            server_mgr
                .get_connection(server_sock, client_sock)
                .unwrap()
                .state,
            TcpState::Established
        );

        // 5. Client sends Data ("HTTP GET")
        let data_bytes = client_mgr
            .send_data(client_sock, server_sock, b"GET / HTTP/1.1\r\n\r\n")
            .unwrap();
        let data_seg = TcpSegment::parse(client_ip, server_ip, &data_bytes, true).unwrap();
        assert_eq!(data_seg.payload, b"GET / HTTP/1.1\r\n\r\n");

        // 6. Server receives data and sends ACK
        let data_ack_bytes = server_mgr
            .process_segment(client_ip, server_ip, &data_seg)
            .unwrap();
        let data_ack_seg = TcpSegment::parse(server_ip, client_ip, &data_ack_bytes, true).unwrap();
        assert_eq!(data_ack_seg.flags, TcpFlags::ack());
        assert_eq!(
            server_mgr
                .get_connection(server_sock, client_sock)
                .unwrap()
                .rx_buffer,
            b"GET / HTTP/1.1\r\n\r\n"
        );

        // 7. Client processes data ACK
        let _ = client_mgr.process_segment(server_ip, client_ip, &data_ack_seg);

        // 8. Client closes connection (FIN-ACK)
        let fin_bytes = client_mgr.close(client_sock, server_sock).unwrap();
        let fin_seg = TcpSegment::parse(client_ip, server_ip, &fin_bytes, true).unwrap();
        assert_eq!(fin_seg.flags, TcpFlags::fin_ack());
        assert_eq!(
            client_mgr
                .get_connection(client_sock, server_sock)
                .unwrap()
                .state,
            TcpState::FinWait1
        );

        // 9. Server receives FIN, sends FIN-ACK -> LAST_ACK
        let fin_ack_bytes = server_mgr
            .process_segment(client_ip, server_ip, &fin_seg)
            .unwrap();
        let fin_ack_seg = TcpSegment::parse(server_ip, client_ip, &fin_ack_bytes, true).unwrap();
        assert_eq!(fin_ack_seg.flags, TcpFlags::fin_ack());
        assert_eq!(
            server_mgr
                .get_connection(server_sock, client_sock)
                .unwrap()
                .state,
            TcpState::LastAck
        );

        // 10. Client receives FIN-ACK, sends ACK -> TIME_WAIT
        let final_ack_bytes = client_mgr
            .process_segment(server_ip, client_ip, &fin_ack_seg)
            .unwrap();
        let final_ack_seg =
            TcpSegment::parse(client_ip, server_ip, &final_ack_bytes, true).unwrap();
        assert_eq!(final_ack_seg.flags, TcpFlags::ack());
        assert_eq!(
            client_mgr
                .get_connection(client_sock, server_sock)
                .unwrap()
                .state,
            TcpState::TimeWait
        );

        // 11. Server receives final ACK -> CLOSED
        let server_closed_resp = server_mgr.process_segment(client_ip, server_ip, &final_ack_seg);
        assert!(server_closed_resp.is_none());
        assert_eq!(
            server_mgr
                .get_connection(server_sock, client_sock)
                .unwrap()
                .state,
            TcpState::Closed
        );
    }
}
