//! In-Band Flow Analytics & Telemetry (IFA 2.0 / RFC 9197).
//!
//! Implements In-situ Flow Analytics (IFA 2.0) packet framing, hop-by-hop metadata insertion
//! (Node ID, Ingress/Egress Interface, Queue Depth, and Transit Latency), and egress flow
//! telemetry analytics extraction for real-time congestion and microburst detection.

/// IFA 2.0 Protocol Version.
pub const IFA_VERSION_2: u8 = 0x02;

/// Telemetry Request Bitflags in IFA Header.
pub const IFA_REQ_NODE_ID: u8 = 0x01;
pub const IFA_REQ_PORTS: u8 = 0x02;
pub const IFA_REQ_LATENCY: u8 = 0x04;
pub const IFA_REQ_QUEUE_DEPTH: u8 = 0x08;
pub const IFA_REQ_TIMESTAMPS: u8 = 0x10;
pub const IFA_REQ_DROP_REASON: u8 = 0x20;
pub const IFA_REQ_BUFFER_OCCUPANCY: u8 = 0x40;

/// IFA 2.0 Base Header (RFC 9197).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfaHeader {
    pub version: u8,
    pub hop_limit: u8,
    pub current_hop_count: u8,
    pub request_vector: u8,
}

impl IfaHeader {
    pub fn new(hop_limit: u8, request_vector: u8) -> Self {
        IfaHeader {
            version: IFA_VERSION_2,
            hop_limit,
            current_hop_count: 0,
            request_vector,
        }
    }

    pub fn serialize(&self) -> [u8; 4] {
        [
            (self.version << 4) & 0xF0,
            self.hop_limit,
            self.current_hop_count,
            self.request_vector,
        ]
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        let version = (data[0] >> 4) & 0x0F;
        if version != IFA_VERSION_2 {
            return None;
        }
        Some(IfaHeader {
            version,
            hop_limit: data[1],
            current_hop_count: data[2],
            request_vector: data[3],
        })
    }
}

/// Hop-by-Hop Telemetry Record inserted by transit routers (16 octets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfaHopRecord {
    pub node_id: u32,
    pub ingress_port: u16,
    pub egress_port: u16,
    pub hop_latency_ns: u32,
    pub queue_depth_bytes: u32,
}

impl IfaHopRecord {
    pub fn new(
        node_id: u32,
        ingress_port: u16,
        egress_port: u16,
        hop_latency_ns: u32,
        queue_depth_bytes: u32,
    ) -> Self {
        IfaHopRecord {
            node_id,
            ingress_port,
            egress_port,
            hop_latency_ns,
            queue_depth_bytes,
        }
    }

    pub fn serialize(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..4].copy_from_slice(&self.node_id.to_be_bytes());
        buf[4..6].copy_from_slice(&self.ingress_port.to_be_bytes());
        buf[6..8].copy_from_slice(&self.egress_port.to_be_bytes());
        buf[8..12].copy_from_slice(&self.hop_latency_ns.to_be_bytes());
        buf[12..16].copy_from_slice(&self.queue_depth_bytes.to_be_bytes());
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 16 {
            return None;
        }
        let node_id = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let ingress_port = u16::from_be_bytes([data[4], data[5]]);
        let egress_port = u16::from_be_bytes([data[6], data[7]]);
        let hop_latency_ns = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let queue_depth_bytes = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);

        Some(IfaHopRecord {
            node_id,
            ingress_port,
            egress_port,
            hop_latency_ns,
            queue_depth_bytes,
        })
    }
}

/// IFA 2.0 In-Band Telemetry Packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfaPacket {
    pub header: IfaHeader,
    pub records: Vec<IfaHopRecord>,
    pub payload: Vec<u8>,
}

impl IfaPacket {
    pub fn new(header: IfaHeader, payload: Vec<u8>) -> Self {
        IfaPacket {
            header,
            records: Vec::new(),
            payload,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + self.records.len() * 16 + self.payload.len());
        buf.extend_from_slice(&self.header.serialize());
        for rec in &self.records {
            buf.extend_from_slice(&rec.serialize());
        }
        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        let header = IfaHeader::parse(&data[0..4])?;
        let hop_count = header.current_hop_count as usize;
        let records_len = hop_count * 16;
        if data.len() < 4 + records_len {
            return None;
        }

        let mut records = Vec::with_capacity(hop_count);
        let mut offset = 4;
        for _ in 0..hop_count {
            let rec = IfaHopRecord::parse(&data[offset..offset + 16])?;
            records.push(rec);
            offset += 16;
        }

        let payload = data[offset..].to_vec();

        Some(IfaPacket {
            header,
            records,
            payload,
        })
    }
}

/// In-Band Flow Analytics (IFA 2.0) Processing Engine.
#[derive(Debug, Clone, Default)]
pub struct IfaTelemetryEngine {
    pub local_node_id: u32,
    pub probes_encapsulated: usize,
    pub hops_inserted: usize,
    pub packets_collected: usize,
}

impl IfaTelemetryEngine {
    pub fn new(local_node_id: u32) -> Self {
        IfaTelemetryEngine {
            local_node_id,
            probes_encapsulated: 0,
            hops_inserted: 0,
            packets_collected: 0,
        }
    }

    /// Ingress Encapsulation: Generates a new IFA packet with telemetry request vector.
    pub fn ingress_encapsulate(
        &mut self,
        payload: &[u8],
        hop_limit: u8,
        req_vector: u8,
    ) -> IfaPacket {
        self.probes_encapsulated += 1;
        let header = IfaHeader::new(hop_limit, req_vector);
        IfaPacket::new(header, payload.to_vec())
    }

    /// Transit Processing: Inserts this node's telemetry record if hop limit is not exceeded.
    pub fn transit_insert_hop(
        &mut self,
        pkt: &mut IfaPacket,
        ingress_port: u16,
        egress_port: u16,
        hop_latency_ns: u32,
        queue_depth_bytes: u32,
    ) -> bool {
        if pkt.header.current_hop_count >= pkt.header.hop_limit {
            return false;
        }

        let rec = IfaHopRecord::new(
            self.local_node_id,
            ingress_port,
            egress_port,
            hop_latency_ns,
            queue_depth_bytes,
        );
        pkt.records.push(rec);
        pkt.header.current_hop_count += 1;
        self.hops_inserted += 1;
        true
    }

    /// Egress Extraction: Collects telemetry records from an arriving IFA packet.
    pub fn egress_collect(&mut self, pkt: &IfaPacket) -> Vec<IfaHopRecord> {
        self.packets_collected += 1;
        pkt.records.clone()
    }
}

/// IFA 2.0 Packet Drop Reason Code (RFC 9359 / RFC 9197).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum IfaDropReason {
    #[default]
    None = 0x00,
    Congestion = 0x01,
    MtuExceeded = 0x02,
    TtlExpired = 0x03,
    BufferOverflow = 0x04,
    AclDeny = 0x05,
    RouteLookupFailed = 0x06,
    Unknown = 0xFF,
}

impl IfaDropReason {
    pub fn from_u8(val: u8) -> Self {
        match val {
            0x00 => IfaDropReason::None,
            0x01 => IfaDropReason::Congestion,
            0x02 => IfaDropReason::MtuExceeded,
            0x03 => IfaDropReason::TtlExpired,
            0x04 => IfaDropReason::BufferOverflow,
            0x05 => IfaDropReason::AclDeny,
            0x06 => IfaDropReason::RouteLookupFailed,
            _ => IfaDropReason::Unknown,
        }
    }
}

/// Extended Hop-by-Hop Telemetry Record (32 octets) with high-resolution timestamps,
/// buffer occupancy, and drop causality (RFC 9197 / RFC 9359).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfaExtendedHopRecord {
    pub node_id: u32,
    pub ingress_port: u16,
    pub egress_port: u16,
    pub ingress_timestamp_ns: u64,
    pub egress_timestamp_ns: u64,
    pub queue_depth_bytes: u32,
    pub buffer_occupancy_pct: u8,
    pub drop_reason: IfaDropReason,
}

impl IfaExtendedHopRecord {
    pub fn new(
        node_id: u32,
        ingress_port: u16,
        egress_port: u16,
        ingress_timestamp_ns: u64,
        egress_timestamp_ns: u64,
        queue_depth_bytes: u32,
        buffer_occupancy_pct: u8,
        drop_reason: IfaDropReason,
    ) -> Self {
        IfaExtendedHopRecord {
            node_id,
            ingress_port,
            egress_port,
            ingress_timestamp_ns,
            egress_timestamp_ns,
            queue_depth_bytes,
            buffer_occupancy_pct,
            drop_reason,
        }
    }

    /// Computes residence transit latency in nanoseconds.
    pub fn transit_latency_ns(&self) -> u64 {
        if self.egress_timestamp_ns >= self.ingress_timestamp_ns {
            self.egress_timestamp_ns - self.ingress_timestamp_ns
        } else {
            0
        }
    }

    /// Serializes the 32-octet extended hop record into network byte order.
    pub fn serialize(&self) -> [u8; 32] {
        let mut buf = [0u8; 32];
        buf[0..4].copy_from_slice(&self.node_id.to_be_bytes());
        buf[4..6].copy_from_slice(&self.ingress_port.to_be_bytes());
        buf[6..8].copy_from_slice(&self.egress_port.to_be_bytes());
        buf[8..16].copy_from_slice(&self.ingress_timestamp_ns.to_be_bytes());
        buf[16..24].copy_from_slice(&self.egress_timestamp_ns.to_be_bytes());
        buf[24..28].copy_from_slice(&self.queue_depth_bytes.to_be_bytes());
        buf[28] = self.buffer_occupancy_pct;
        buf[29] = self.drop_reason as u8;
        // buf[30..32] reserved = 0
        buf
    }

    /// Parses a 32-octet extended hop record from byte slice.
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 32 {
            return None;
        }
        let node_id = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let ingress_port = u16::from_be_bytes([data[4], data[5]]);
        let egress_port = u16::from_be_bytes([data[6], data[7]]);
        let ingress_timestamp_ns = u64::from_be_bytes([
            data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
        ]);
        let egress_timestamp_ns = u64::from_be_bytes([
            data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23],
        ]);
        let queue_depth_bytes = u32::from_be_bytes([data[24], data[25], data[26], data[27]]);
        let buffer_occupancy_pct = data[28];
        let drop_reason = IfaDropReason::from_u8(data[29]);

        Some(IfaExtendedHopRecord {
            node_id,
            ingress_port,
            egress_port,
            ingress_timestamp_ns,
            egress_timestamp_ns,
            queue_depth_bytes,
            buffer_occupancy_pct,
            drop_reason,
        })
    }
}

/// IFA 2.0 Extended Telemetry Packet carrying high-resolution 32-byte records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfaExtendedPacket {
    pub header: IfaHeader,
    pub records: Vec<IfaExtendedHopRecord>,
    pub payload: Vec<u8>,
}

impl IfaExtendedPacket {
    pub fn new(header: IfaHeader, payload: Vec<u8>) -> Self {
        IfaExtendedPacket {
            header,
            records: Vec::new(),
            payload,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + self.records.len() * 32 + self.payload.len());
        buf.extend_from_slice(&self.header.serialize());
        for rec in &self.records {
            buf.extend_from_slice(&rec.serialize());
        }
        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        let header = IfaHeader::parse(&data[0..4])?;
        let hop_count = header.current_hop_count as usize;
        let records_len = hop_count * 32;
        if data.len() < 4 + records_len {
            return None;
        }

        let mut records = Vec::with_capacity(hop_count);
        let mut offset = 4;
        for _ in 0..hop_count {
            let rec = IfaExtendedHopRecord::parse(&data[offset..offset + 32])?;
            records.push(rec);
            offset += 32;
        }

        let payload = data[offset..].to_vec();
        Some(IfaExtendedPacket {
            header,
            records,
            payload,
        })
    }
}

/// IFA 2.0 Anomaly & Microburst Alert Type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IfaAlertType {
    LatencySlaViolation,
    QueueBufferSpike,
    PacketDropDetected,
    ExcessiveHopCount,
}

/// Anomaly alert notification generated by the IFA Telemetry Engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfaAlert {
    pub node_id: u32,
    pub alert_type: IfaAlertType,
    pub observed_value: u64,
    pub threshold_value: u64,
}

/// IFA Anomaly Detector for real-time SLA verification and microburst detection.
#[derive(Debug, Clone)]
pub struct IfaAnomalyDetector {
    pub latency_threshold_ns: u64,
    pub queue_threshold_bytes: u32,
    pub buffer_occupancy_threshold_pct: u8,
    pub alerts_generated: Vec<IfaAlert>,
}

impl IfaAnomalyDetector {
    pub fn new(
        latency_threshold_ns: u64,
        queue_threshold_bytes: u32,
        buffer_occupancy_threshold_pct: u8,
    ) -> Self {
        IfaAnomalyDetector {
            latency_threshold_ns,
            queue_threshold_bytes,
            buffer_occupancy_threshold_pct,
            alerts_generated: Vec::new(),
        }
    }

    /// Inspects an extended hop record and generates alerts if thresholds are breached.
    pub fn inspect_record(&mut self, rec: &IfaExtendedHopRecord) -> Vec<IfaAlert> {
        let mut triggered = Vec::new();

        let latency = rec.transit_latency_ns();
        if latency > self.latency_threshold_ns {
            let alert = IfaAlert {
                node_id: rec.node_id,
                alert_type: IfaAlertType::LatencySlaViolation,
                observed_value: latency,
                threshold_value: self.latency_threshold_ns,
            };
            triggered.push(alert.clone());
            self.alerts_generated.push(alert);
        }

        if rec.queue_depth_bytes > self.queue_threshold_bytes
            || rec.buffer_occupancy_pct > self.buffer_occupancy_threshold_pct
        {
            let alert = IfaAlert {
                node_id: rec.node_id,
                alert_type: IfaAlertType::QueueBufferSpike,
                observed_value: rec.queue_depth_bytes as u64,
                threshold_value: self.queue_threshold_bytes as u64,
            };
            triggered.push(alert.clone());
            self.alerts_generated.push(alert);
        }

        if rec.drop_reason != IfaDropReason::None {
            let alert = IfaAlert {
                node_id: rec.node_id,
                alert_type: IfaAlertType::PacketDropDetected,
                observed_value: rec.drop_reason as u64,
                threshold_value: 0,
            };
            triggered.push(alert.clone());
            self.alerts_generated.push(alert);
        }

        triggered
    }
}

/// Formatter that transforms IFA telemetry into IPFIX-compatible flow export payload (RFC 7011).
#[derive(Debug, Clone)]
pub struct IfaIpfixExporter {
    pub observation_domain_id: u32,
    pub sequence_number: u32,
    pub template_id: u16,
}

impl IfaIpfixExporter {
    pub fn new(observation_domain_id: u32, template_id: u16) -> Self {
        IfaIpfixExporter {
            observation_domain_id,
            sequence_number: 1,
            template_id,
        }
    }

    /// Serializes an IFA hop record into an IPFIX Data Record Set.
    ///
    /// IPFIX Message:
    /// - Header: Version (0x000A), Length, ExportTime, SeqNum, DomainID (16 bytes)
    /// - Set Header: SetID (Template ID), SetLength (4 bytes)
    /// - Data Record: NodeID (4B), IngressPort (2B), EgressPort (2B), Latency (8B), QueueDepth (4B) (20 bytes)
    pub fn export_record(&mut self, rec: &IfaExtendedHopRecord, export_time_sec: u32) -> Vec<u8> {
        let record_len = 20usize;
        let set_len = 4 + record_len;
        let total_len = 16 + set_len;

        let mut msg = Vec::with_capacity(total_len);

        // IPFIX Message Header (RFC 7011 Section 3.1)
        msg.extend_from_slice(&0x000Au16.to_be_bytes()); // Version 10
        msg.extend_from_slice(&(total_len as u16).to_be_bytes());
        msg.extend_from_slice(&export_time_sec.to_be_bytes());
        msg.extend_from_slice(&self.sequence_number.to_be_bytes());
        msg.extend_from_slice(&self.observation_domain_id.to_be_bytes());

        // IPFIX Set Header
        msg.extend_from_slice(&self.template_id.to_be_bytes());
        msg.extend_from_slice(&(set_len as u16).to_be_bytes());

        // Data Fields
        msg.extend_from_slice(&rec.node_id.to_be_bytes());
        msg.extend_from_slice(&rec.ingress_port.to_be_bytes());
        msg.extend_from_slice(&rec.egress_port.to_be_bytes());
        msg.extend_from_slice(&rec.transit_latency_ns().to_be_bytes());
        msg.extend_from_slice(&rec.queue_depth_bytes.to_be_bytes());

        self.sequence_number += 1;
        msg
    }
}
