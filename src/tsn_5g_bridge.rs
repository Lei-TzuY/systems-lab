//! 3GPP TS 23.501 Section 5.27 / TS 24.519 / TS 29.574 / IEEE 802.1AS 5G-TSN Virtual Bridge Engine.
//!
//! Implements Time-Sensitive Communication (TSC) and deterministic networking over 5G System (5GS):
//! - 5GS operates as an IEEE 802.1Q Virtual Ethernet Bridge with virtual bridge ID and bridge ports.
//! - Network-side TSN Translator (NW-TT): Collocated with UPF, translates TSN frames to/from 5GS.
//! - Device-side TSN Translator (DS-TT): Collocated with UE, translates TSN frames to/from 5GS.
//! - Time-Sensitive Communication and Time Synchronization Function (TSCTF - TS 29.574):
//!   - Bridge delay computation: Reports per-port-pair minimum and maximum delays ($D_{min}, D_{max}$) to CNC.
//!   - CNC Stream Reservation mapping (IEEE 802.1Qcc): Maps TSpec to 5G QoS (Delay-Critical 5QI, GFBR, PDB).
//!   - TSCAI (Time-Sensitive Communication Assistance Information): Generates flow direction, periodicity,
//!     Burst Arrival Time (BAT), and survival time for gNodeB RAN scheduling.
//! - IEEE 802.1AS PTP Residence Time Calculation:
//!   - Ingress TT captures $T_{in}$, Egress TT captures $T_{out}$.
//!   - Calculates 5GS residence time $\Delta T_{res} = T_{out} - T_{in}$.
//!   - Adds port delays and updates PTP `correctionField` scaled by 16 bits.
//! - Deterministic De-Jittering Buffer (Hold-and-Forward):
//!   - Eliminates radio transmission jitter by holding early packets until scheduled cycle boundaries.

use std::collections::{HashMap, VecDeque};

use crate::ethernet::MacAddress;
use crate::ptp::PtpHeader;
use crate::tsn_cnc::{StreamId, TrafficSpecification, UserToNetworkRequirements};

// ---------------------------------------------------------------------------
// 5G-TSN Bridge Enums & Identifiers (TS 23.501 §5.27 / IEEE 802.1Q)
// ---------------------------------------------------------------------------

/// 64-bit IEEE 802.1Q Virtual Bridge Identifier.
/// Typically composed of a 16-bit Bridge Priority and a 48-bit Bridge MAC Address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TsnBridgeId(pub [u8; 8]);

impl TsnBridgeId {
    pub fn new(priority: u16, bridge_mac: MacAddress) -> Self {
        let mut bytes = [0u8; 8];
        bytes[0..2].copy_from_slice(&priority.to_be_bytes());
        bytes[2..8].copy_from_slice(&bridge_mac.0);
        TsnBridgeId(bytes)
    }

    pub fn to_string(&self) -> String {
        format!(
            "{:02x}{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7]
        )
    }
}

/// TSN Port Type in 5GS Virtual Bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsnPortType {
    /// Network-side TSN Translator (collocated with UPF).
    NwTt,
    /// Device-side TSN Translator (collocated with UE).
    DsTt,
}

/// Operational state of a TSN bridge port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsnPortState {
    Disabled,
    Blocking,
    Listening,
    Learning,
    Forwarding,
}

/// Configuration profile for a TSN Bridge Port (TS 23.501 §5.27.1.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsnPortConfig {
    pub port_number: u32,
    pub port_type: TsnPortType,
    pub mac_address: MacAddress,
    pub link_speed_mbps: u64,
    /// Physical/internal transmission propagation delay in nanoseconds.
    pub tx_propagation_delay_ns: u64,
    /// Time synchronization granularity in nanoseconds (e.g. 8ns or 10ns).
    pub sync_granularity_ns: u32,
    pub state: TsnPortState,
}

impl TsnPortConfig {
    pub fn new(
        port_number: u32,
        port_type: TsnPortType,
        mac_address: MacAddress,
        link_speed_mbps: u64,
        tx_propagation_delay_ns: u64,
        sync_granularity_ns: u32,
    ) -> Self {
        TsnPortConfig {
            port_number,
            port_type,
            mac_address,
            link_speed_mbps,
            tx_propagation_delay_ns,
            sync_granularity_ns,
            state: TsnPortState::Forwarding,
        }
    }
}

/// Minimum and Maximum Bridge Delay across a port pair (TS 23.501 §5.27.1.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortPairDelay {
    pub ingress_port: u32,
    pub egress_port: u32,
    pub traffic_class: u8, // Priority / PCP (0..7)
    pub min_bridge_delay_ns: u64,
    pub max_bridge_delay_ns: u64,
}

// ---------------------------------------------------------------------------
// TSCAI & 5G QoS Modeling (TS 23.501 §5.27.2 / TS 29.574)
// ---------------------------------------------------------------------------

/// TSC Traffic Flow Direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TscTrafficDirection {
    Downlink,
    Uplink,
}

/// Time-Sensitive Communication Assistance Information (TSCAI - TS 23.501 §5.27.2.3).
/// Passed to gNodeB / RAN for deterministic radio scheduling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tscai {
    pub direction: TscTrafficDirection,
    /// Burst repetition periodicity in nanoseconds (e.g. 1_000_000 ns = 1ms).
    pub periodicity_ns: u64,
    /// Reference arrival time of the first packet of a burst in 5GS clock domain (nanoseconds).
    pub burst_arrival_time_ns: u64,
    /// Maximum burst size in bytes.
    pub burst_size_bytes: u32,
    /// Maximum application survival time in microseconds upon frame loss.
    pub survival_time_us: u32,
}

/// 5G QoS Profile assigned to a TSN Stream.
#[derive(Debug, Clone, PartialEq)]
pub struct TsnQosProfile {
    pub qfi: u8,
    pub five_qi: u16, // Delay-Critical GBR 5QI, e.g. 82, 83, 84, 85, 86
    pub gfbr_bps: u64,
    pub mfbr_bps: u64,
    pub pdb_ms: u32,        // Packet Delay Budget in milliseconds
    pub per_loss_rate: f64, // Packet Error Rate
    pub tscai: Option<Tscai>,
}

/// Binding between a TSN Stream (IEEE 802.1Qcc) and 5GS user plane.
#[derive(Debug, Clone, PartialEq)]
pub struct Tsn5gStreamBinding {
    pub stream_id: StreamId,
    pub vlan_id: u16,
    pub pcp: u8,
    pub ingress_port: u32,
    pub egress_port: u32,
    pub pdu_session_id: u8,
    pub qos_profile: TsnQosProfile,
}

// ---------------------------------------------------------------------------
// IEEE 802.1AS PTP Synchronization Support (TS 24.519 / IEEE 1588)
// ---------------------------------------------------------------------------

/// PTP Residence Time and Correction Field computation report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PtpResidenceTimeReport {
    pub ptp_msg_type: u8,
    pub ingress_port: u32,
    pub egress_port: u32,
    pub ingress_timestamp_ns: u64,
    pub egress_timestamp_ns: u64,
    pub residence_time_ns: u64,
    pub ingress_port_delay_ns: u64,
    pub egress_port_delay_ns: u64,
    pub total_correction_ns: u64,
    pub original_correction_field: i64,
    pub updated_correction_field: i64,
}

/// Frame holding entry in the De-Jittering Buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeJitterBufferEntry {
    pub frame_id: u64,
    pub stream_id: StreamId,
    pub ingress_5g_timestamp_ns: u64,
    pub scheduled_release_ns: u64,
    pub payload: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Network-Side TSN Translator (NW-TT)
// ---------------------------------------------------------------------------

/// Network-side TSN Translator (NW-TT - TS 23.501 §5.27.1.2) collocated with UPF.
pub struct NwTtEngine {
    pub upf_id: String,
    pub ports: HashMap<u32, TsnPortConfig>,
    pub ingress_timestamps: HashMap<u64, u64>, // frame_id -> ingress timestamp ns
    pub de_jitter_buffer: VecDeque<DeJitterBufferEntry>,
}

impl NwTtEngine {
    pub fn new(upf_id: &str) -> Self {
        NwTtEngine {
            upf_id: upf_id.to_string(),
            ports: HashMap::new(),
            ingress_timestamps: HashMap::new(),
            de_jitter_buffer: VecDeque::new(),
        }
    }

    pub fn add_port(&mut self, port_config: TsnPortConfig) {
        self.ports.insert(port_config.port_number, port_config);
    }

    pub fn record_ingress_timestamp(&mut self, frame_id: u64, timestamp_ns: u64) {
        self.ingress_timestamps.insert(frame_id, timestamp_ns);
    }

    pub fn remove_ingress_timestamp(&mut self, frame_id: u64) -> Option<u64> {
        self.ingress_timestamps.remove(&frame_id)
    }
}

// ---------------------------------------------------------------------------
// Device-Side TSN Translator (DS-TT)
// ---------------------------------------------------------------------------

/// Device-side TSN Translator (DS-TT - TS 23.501 §5.27.1.2) collocated with UE.
pub struct DsTtEngine {
    pub ue_id: String,
    pub port: TsnPortConfig,
    pub ingress_timestamps: HashMap<u64, u64>,
    pub de_jitter_buffer: VecDeque<DeJitterBufferEntry>,
}

impl DsTtEngine {
    pub fn new(ue_id: &str, port: TsnPortConfig) -> Self {
        DsTtEngine {
            ue_id: ue_id.to_string(),
            port,
            ingress_timestamps: HashMap::new(),
            de_jitter_buffer: VecDeque::new(),
        }
    }

    pub fn record_ingress_timestamp(&mut self, frame_id: u64, timestamp_ns: u64) {
        self.ingress_timestamps.insert(frame_id, timestamp_ns);
    }

    pub fn remove_ingress_timestamp(&mut self, frame_id: u64) -> Option<u64> {
        self.ingress_timestamps.remove(&frame_id)
    }
}

// ---------------------------------------------------------------------------
// 5G-TSN Bridge Engine (TSCTF & Virtual Bridge Orchestration)
// ---------------------------------------------------------------------------

/// Top-Level 5G-TSN Virtual Bridge Engine (TSCTF, NW-TT, DS-TT).
pub struct Tsn5gBridgeEngine {
    pub tsctf_id: String,
    pub bridge_id: TsnBridgeId,
    pub nw_tts: HashMap<String, NwTtEngine>,
    pub ds_tts: HashMap<String, DsTtEngine>,
    /// Port pair delays: (ingress_port, egress_port) -> PortPairDelay
    pub port_pair_delays: HashMap<(u32, u32), PortPairDelay>,
    /// TSN Stream bindings to 5G QoS and PDU Sessions
    pub stream_bindings: HashMap<StreamId, Tsn5gStreamBinding>,
    pub next_qfi: u8,
    pub next_frame_id: u64,
}

impl Tsn5gBridgeEngine {
    /// Create a new 5G-TSN Virtual Bridge Engine instance.
    pub fn new(tsctf_id: &str, bridge_id: TsnBridgeId) -> Self {
        Tsn5gBridgeEngine {
            tsctf_id: tsctf_id.to_string(),
            bridge_id,
            nw_tts: HashMap::new(),
            ds_tts: HashMap::new(),
            port_pair_delays: HashMap::new(),
            stream_bindings: HashMap::new(),
            next_qfi: 10,
            next_frame_id: 1,
        }
    }

    /// Register an NW-TT instance (collocated with a UPF).
    pub fn register_nw_tt(&mut self, nw_tt: NwTtEngine) {
        self.nw_tts.insert(nw_tt.upf_id.clone(), nw_tt);
    }

    /// Register a DS-TT instance (collocated with a UE).
    pub fn register_ds_tt(&mut self, ds_tt: DsTtEngine) {
        self.ds_tts.insert(ds_tt.ue_id.clone(), ds_tt);
    }

    /// Configure bidirectional or directional port-pair delay bounds across the 5GS.
    pub fn configure_port_pair_delay(
        &mut self,
        ingress_port: u32,
        egress_port: u32,
        traffic_class: u8,
        min_bridge_delay_ns: u64,
        max_bridge_delay_ns: u64,
    ) {
        let delay = PortPairDelay {
            ingress_port,
            egress_port,
            traffic_class,
            min_bridge_delay_ns,
            max_bridge_delay_ns,
        };
        self.port_pair_delays
            .insert((ingress_port, egress_port), delay);
    }

    /// Find port configuration by port number across NW-TTs and DS-TTs.
    pub fn get_port_config(&self, port_num: u32) -> Option<&TsnPortConfig> {
        for nw_tt in self.nw_tts.values() {
            if let Some(port) = nw_tt.ports.get(&port_num) {
                return Some(port);
            }
        }
        for ds_tt in self.ds_tts.values() {
            if ds_tt.port.port_number == port_num {
                return Some(&ds_tt.port);
            }
        }
        None
    }

    // -----------------------------------------------------------------------
    // CNC Stream Reservation & TSCAI Translation (TS 23.501 §5.27.2 / TS 29.574)
    // -----------------------------------------------------------------------

    /// Process a TSN Stream Reservation from CNC (IEEE 802.1Qcc) and map to 5G QoS Profile & TSCAI.
    pub fn process_cnc_stream_reservation(
        &mut self,
        stream_id: StreamId,
        vlan_id: u16,
        pcp: u8,
        ingress_port: u32,
        egress_port: u32,
        pdu_session_id: u8,
        direction: TscTrafficDirection,
        tspec: &TrafficSpecification,
        user_reqs: &UserToNetworkRequirements,
        base_arrival_time_ns: u64,
    ) -> Result<Tsn5gStreamBinding, &'static str> {
        // Validate ports exist
        let in_port = self
            .get_port_config(ingress_port)
            .ok_or("Ingress TSN bridge port not found")?;
        let out_port = self
            .get_port_config(egress_port)
            .ok_or("Egress TSN bridge port not found")?;

        if in_port.state != TsnPortState::Forwarding || out_port.state != TsnPortState::Forwarding {
            return Err("Bridge ports must be in Forwarding state for TSN reservation");
        }

        // Validate port pair delay capability
        let port_delay = self
            .port_pair_delays
            .get(&(ingress_port, egress_port))
            .ok_or("No configured bridge delay for specified port pair")?;

        let max_bridge_delay_us = (port_delay.max_bridge_delay_ns / 1000) as u32;
        if max_bridge_delay_us > user_reqs.max_latency_us {
            return Err("5GS virtual bridge delay exceeds listener maximum latency budget");
        }

        // Calculate Guaranteed Flow Bit Rate (GFBR)
        // GFBR = (max_frame_size * max_interval_frames * 8) / (interval_us * 1e-6)
        if tspec.interval_us == 0 {
            return Err("TrafficSpecification interval cannot be zero");
        }
        let bits_per_interval =
            (tspec.max_frame_size as u64) * (tspec.max_interval_frames as u64) * 8;
        let gfbr_bps = (bits_per_interval * 1_000_000) / (tspec.interval_us as u64);
        let mfbr_bps = (gfbr_bps as f64 * 1.2) as u64; // 20% burst headroom

        // Derive 5QI (Delay-Critical GBR 5QI selection per TS 23.501 Table 5.7.4)
        // 5QI 82 (PDB 10ms, PER 1e-4)
        // 5QI 83 (PDB 10ms, PER 1e-5)
        // 5QI 84 (PDB 30ms, PER 1e-5)
        // 5QI 85 (PDB 5ms, PER 1e-5)
        // 5QI 86 (PDB 5ms, PER 1e-4)
        let five_qi = if max_bridge_delay_us <= 5000 {
            85 // 5ms PDB, highest reliability
        } else if max_bridge_delay_us <= 10000 {
            83 // 10ms PDB, high reliability
        } else {
            84 // 30ms PDB
        };

        let pdb_ms = if five_qi == 85 { 5 } else { 10 };

        // Generate TSCAI for gNodeB radio scheduler
        let periodicity_ns = (tspec.interval_us as u64) * 1000;
        let burst_size_bytes = (tspec.max_frame_size as u32) * (tspec.max_interval_frames as u32);
        let survival_time_us = tspec.interval_us * 2; // Application can survive 2 missed cycles

        let tscai = Tscai {
            direction,
            periodicity_ns,
            burst_arrival_time_ns: base_arrival_time_ns,
            burst_size_bytes,
            survival_time_us,
        };

        let qfi = self.next_qfi;
        self.next_qfi += 1;

        let qos_profile = TsnQosProfile {
            qfi,
            five_qi,
            gfbr_bps,
            mfbr_bps,
            pdb_ms,
            per_loss_rate: 1e-5,
            tscai: Some(tscai),
        };

        let binding = Tsn5gStreamBinding {
            stream_id,
            vlan_id,
            pcp,
            ingress_port,
            egress_port,
            pdu_session_id,
            qos_profile,
        };

        self.stream_bindings.insert(stream_id, binding.clone());
        Ok(binding)
    }

    // -----------------------------------------------------------------------
    // IEEE 802.1AS PTP Residence Time Calculation & Correction Field Update
    // -----------------------------------------------------------------------

    /// Process ingress of an IEEE 802.1AS PTP message into the 5GS.
    /// Ingress TT (NW-TT or DS-TT) records the 5GS ingress timestamp T_in.
    pub fn process_ptp_ingress(
        &mut self,
        ingress_port: u32,
        frame_id: u64,
        ingress_timestamp_5g_ns: u64,
    ) -> Result<(), &'static str> {
        let port = self
            .get_port_config(ingress_port)
            .ok_or("Ingress port not found")?;

        match port.port_type {
            TsnPortType::NwTt => {
                for nw_tt in self.nw_tts.values_mut() {
                    if nw_tt.ports.contains_key(&ingress_port) {
                        nw_tt.record_ingress_timestamp(frame_id, ingress_timestamp_5g_ns);
                        return Ok(());
                    }
                }
                Err("NW-TT holding port not found")
            }
            TsnPortType::DsTt => {
                for ds_tt in self.ds_tts.values_mut() {
                    if ds_tt.port.port_number == ingress_port {
                        ds_tt.record_ingress_timestamp(frame_id, ingress_timestamp_5g_ns);
                        return Ok(());
                    }
                }
                Err("DS-TT holding port not found")
            }
        }
    }

    /// Process egress of an IEEE 802.1AS PTP message from the 5GS.
    /// Calculates residence time: Delta T = T_out - T_in.
    /// Updates PTP header `correctionField` scaled by 16 bits (IEEE 1588 / 802.1AS standard).
    pub fn process_ptp_egress(
        &mut self,
        ingress_port: u32,
        egress_port: u32,
        frame_id: u64,
        egress_timestamp_5g_ns: u64,
        ptp_header: &mut PtpHeader,
    ) -> Result<PtpResidenceTimeReport, &'static str> {
        let in_port = self
            .get_port_config(ingress_port)
            .ok_or("Ingress port not found")?;
        let in_delay = in_port.tx_propagation_delay_ns;
        let in_port_type = in_port.port_type;

        let out_port = self
            .get_port_config(egress_port)
            .ok_or("Egress port not found")?;
        let out_delay = out_port.tx_propagation_delay_ns;

        // Retrieve ingress timestamp T_in from the ingress translator
        let ingress_timestamp_ns = match in_port_type {
            TsnPortType::NwTt => {
                let mut found_ts = None;
                for nw_tt in self.nw_tts.values_mut() {
                    if let Some(ts) = nw_tt.remove_ingress_timestamp(frame_id) {
                        found_ts = Some(ts);
                        break;
                    }
                }
                found_ts.ok_or("No matching ingress timestamp found in NW-TT")?
            }
            TsnPortType::DsTt => {
                let mut found_ts = None;
                for ds_tt in self.ds_tts.values_mut() {
                    if let Some(ts) = ds_tt.remove_ingress_timestamp(frame_id) {
                        found_ts = Some(ts);
                        break;
                    }
                }
                found_ts.ok_or("No matching ingress timestamp found in DS-TT")?
            }
        };

        if egress_timestamp_5g_ns < ingress_timestamp_ns {
            return Err("Egress timestamp cannot precede ingress timestamp");
        }

        let residence_time_ns = egress_timestamp_5g_ns - ingress_timestamp_ns;
        let total_correction_ns = residence_time_ns + in_delay + out_delay;

        // IEEE 1588 / 802.1AS: correctionField is a 64-bit signed integer representing nanoseconds shifted by 16 bits.
        // correctionField += (total_correction_ns << 16)
        let original_correction_field = ptp_header.correction_field;
        let correction_delta = (total_correction_ns as i64) << 16;
        let updated_correction_field = original_correction_field.saturating_add(correction_delta);
        ptp_header.correction_field = updated_correction_field;

        Ok(PtpResidenceTimeReport {
            ptp_msg_type: ptp_header.message_type,
            ingress_port,
            egress_port,
            ingress_timestamp_ns,
            egress_timestamp_ns: egress_timestamp_5g_ns,
            residence_time_ns,
            ingress_port_delay_ns: in_delay,
            egress_port_delay_ns: out_delay,
            total_correction_ns,
            original_correction_field,
            updated_correction_field,
        })
    }

    // -----------------------------------------------------------------------
    // Deterministic De-Jittering Buffer (Hold-and-Forward)
    // -----------------------------------------------------------------------

    /// Queue an arriving packet in the egress translator's de-jittering buffer.
    /// The packet will be held until `ingress_timestamp_ns + max_bridge_delay_ns`
    /// to guarantee deterministic latency and zero jitter to the receiving TSN station.
    pub fn queue_de_jitter_frame(
        &mut self,
        stream_id: StreamId,
        egress_port: u32,
        ingress_timestamp_ns: u64,
        payload: Vec<u8>,
    ) -> Result<u64, &'static str> {
        let binding = self
            .stream_bindings
            .get(&stream_id)
            .ok_or("Stream binding not found")?;

        let port_delay = self
            .port_pair_delays
            .get(&(binding.ingress_port, egress_port))
            .ok_or("Port pair delay not configured")?;

        let scheduled_release_ns = ingress_timestamp_ns + port_delay.max_bridge_delay_ns;
        let frame_id = self.next_frame_id;
        self.next_frame_id += 1;

        let entry = DeJitterBufferEntry {
            frame_id,
            stream_id,
            ingress_5g_timestamp_ns: ingress_timestamp_ns,
            scheduled_release_ns,
            payload,
        };

        let port = self
            .get_port_config(egress_port)
            .ok_or("Egress port not found")?;

        match port.port_type {
            TsnPortType::DsTt => {
                for ds_tt in self.ds_tts.values_mut() {
                    if ds_tt.port.port_number == egress_port {
                        ds_tt.de_jitter_buffer.push_back(entry);
                        return Ok(frame_id);
                    }
                }
                Err("DS-TT holding port not found")
            }
            TsnPortType::NwTt => {
                for nw_tt in self.nw_tts.values_mut() {
                    if nw_tt.ports.contains_key(&egress_port) {
                        nw_tt.de_jitter_buffer.push_back(entry);
                        return Ok(frame_id);
                    }
                }
                Err("NW-TT holding port not found")
            }
        }
    }

    /// Flush and release all packets from the egress de-jittering buffer whose
    /// scheduled release epoch has arrived (`current_time_ns >= scheduled_release_ns`).
    pub fn flush_de_jitter_buffer(
        &mut self,
        egress_port: u32,
        current_time_ns: u64,
    ) -> Vec<DeJitterBufferEntry> {
        let mut released = Vec::new();

        if let Some(port) = self.get_port_config(egress_port) {
            match port.port_type {
                TsnPortType::DsTt => {
                    for ds_tt in self.ds_tts.values_mut() {
                        if ds_tt.port.port_number == egress_port {
                            let mut remaining = VecDeque::new();
                            while let Some(entry) = ds_tt.de_jitter_buffer.pop_front() {
                                if current_time_ns >= entry.scheduled_release_ns {
                                    released.push(entry);
                                } else {
                                    remaining.push_back(entry);
                                }
                            }
                            ds_tt.de_jitter_buffer = remaining;
                            break;
                        }
                    }
                }
                TsnPortType::NwTt => {
                    for nw_tt in self.nw_tts.values_mut() {
                        if nw_tt.ports.contains_key(&egress_port) {
                            let mut remaining = VecDeque::new();
                            while let Some(entry) = nw_tt.de_jitter_buffer.pop_front() {
                                if current_time_ns >= entry.scheduled_release_ns {
                                    released.push(entry);
                                } else {
                                    remaining.push_back(entry);
                                }
                            }
                            nw_tt.de_jitter_buffer = remaining;
                            break;
                        }
                    }
                }
            }
        }

        released
    }
}
