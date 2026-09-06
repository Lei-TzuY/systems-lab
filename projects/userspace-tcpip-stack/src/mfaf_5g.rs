//! 3GPP TS 29.576 / TS 23.288 Release 17 5G Messaging Framework Adaptor Function (MFAF) Engine.
//!
//! Implements 5G SBA to Distributed Messaging Framework Adapters (Kafka, MQTT, WebSocket):
//! - Nmfaf_3caDataManagement Service (TS 29.576 Section 5.2):
//!   - Message mapping configuration and topic binding (`configure_mapping` / `delete_mapping`)
//! - Nmfaf_3daDataManagement Service (TS 29.576 Section 5.3):
//!   - Real-time event ingestion and protocol transformation (`ingest_and_format_event`)
//!   - Payload serialization (JSON, compact binary)
//!   - Batching and tumbling buffer management (`flush_pending_batches`)
//!   - Backpressure buffer control preventing memory exhaustion

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// 5G MFAF Enums & Data Structures (TS 29.576 Section 6 / TS 23.288)
// ---------------------------------------------------------------------------

/// Destination Messaging Protocol (TS 29.576 Section 6.1.6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessagingProtocol {
    Kafka,
    Mqtt,
    WebSocket,
}

impl MessagingProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            MessagingProtocol::Kafka => "KAFKA",
            MessagingProtocol::Mqtt => "MQTT",
            MessagingProtocol::WebSocket => "WEBSOCKET",
        }
    }
}

/// Target Serialization Format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerializationFormat {
    Json,
    CompactBinary,
}

/// Ingested 5G Core Telemetry Item.
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryItem {
    pub source_nf: String,
    pub event_name: String,
    pub target_id: Option<String>,
    pub value: f64,
    pub timestamp_epoch_s: u64,
}

/// Dispatched Message Batch delivered to messaging broker.
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchedBatch {
    pub mapping_id: String,
    pub protocol: MessagingProtocol,
    pub destination_topic: String,
    pub payload: Vec<u8>,
    pub record_count: usize,
}

/// Mapping Configuration and State.
#[derive(Debug, Clone)]
pub struct MessageMapping {
    pub mapping_id: String,
    pub protocol: MessagingProtocol,
    pub destination_topic: String,
    pub serialization: SerializationFormat,
    pub batch_size_limit: usize,
    pub buffer: Vec<TelemetryItem>,
}

/// MFAF Error Types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MfafError {
    MappingNotFound,
    InvalidConfiguration(&'static str),
    BufferOverflow,
}

// ---------------------------------------------------------------------------
// Top-Level 5G-MFAF Engine
// ---------------------------------------------------------------------------

/// 5G Messaging Framework Adaptor Function (MFAF).
pub struct MfafEngine {
    pub mfaf_id: String,
    pub next_mapping_counter: u64,
    pub mappings: HashMap<String, MessageMapping>,
    pub max_buffer_limit: usize,
}

impl MfafEngine {
    /// Create a new 5G-MFAF engine instance.
    pub fn new(mfaf_id: &str, max_buffer_limit: usize) -> Self {
        MfafEngine {
            mfaf_id: mfaf_id.to_string(),
            next_mapping_counter: 1,
            mappings: HashMap::new(),
            max_buffer_limit,
        }
    }

    // -----------------------------------------------------------------------
    // Nmfaf_3caDataManagement Service Operations (TS 29.576 Section 5.2)
    // -----------------------------------------------------------------------

    /// Configure a messaging adapter mapping to a destination topic.
    pub fn configure_mapping(
        &mut self,
        protocol: MessagingProtocol,
        destination_topic: &str,
        serialization: SerializationFormat,
        batch_size_limit: usize,
    ) -> Result<String, MfafError> {
        if destination_topic.is_empty() {
            return Err(MfafError::InvalidConfiguration(
                "Destination topic cannot be empty",
            ));
        }
        if batch_size_limit == 0 {
            return Err(MfafError::InvalidConfiguration(
                "Batch size limit must be greater than zero",
            ));
        }

        let mapping_id = format!("mfaf-map-{:08x}", self.next_mapping_counter);
        self.next_mapping_counter += 1;

        let mapping = MessageMapping {
            mapping_id: mapping_id.clone(),
            protocol,
            destination_topic: destination_topic.to_string(),
            serialization,
            batch_size_limit,
            buffer: Vec::new(),
        };

        self.mappings.insert(mapping_id.clone(), mapping);
        Ok(mapping_id)
    }

    /// Delete an existing messaging adapter mapping.
    pub fn delete_mapping(&mut self, mapping_id: &str) -> Result<(), MfafError> {
        self.mappings
            .remove(mapping_id)
            .map(|_| ())
            .ok_or(MfafError::MappingNotFound)
    }

    // -----------------------------------------------------------------------
    // Nmfaf_3daDataManagement Service Operations (TS 29.576 Section 5.3)
    // -----------------------------------------------------------------------

    /// Ingest a telemetry event and return a dispatched batch if batch threshold is met.
    pub fn ingest_event(
        &mut self,
        mapping_id: &str,
        source_nf: &str,
        event_name: &str,
        target_id: Option<&str>,
        value: f64,
        timestamp_epoch_s: u64,
    ) -> Result<Option<DispatchedBatch>, MfafError> {
        let mapping = self
            .mappings
            .get_mut(mapping_id)
            .ok_or(MfafError::MappingNotFound)?;

        if mapping.buffer.len() >= self.max_buffer_limit {
            return Err(MfafError::BufferOverflow);
        }

        mapping.buffer.push(TelemetryItem {
            source_nf: source_nf.to_string(),
            event_name: event_name.to_string(),
            target_id: target_id.map(|s| s.to_string()),
            value,
            timestamp_epoch_s,
        });

        if mapping.buffer.len() >= mapping.batch_size_limit {
            Ok(Some(Self::flush_internal(mapping)))
        } else {
            Ok(None)
        }
    }

    /// Flush any pending buffered events for a given mapping into a dispatched batch.
    pub fn flush_pending_batches(
        &mut self,
        mapping_id: &str,
    ) -> Result<Option<DispatchedBatch>, MfafError> {
        let mapping = self
            .mappings
            .get_mut(mapping_id)
            .ok_or(MfafError::MappingNotFound)?;

        if mapping.buffer.is_empty() {
            return Ok(None);
        }

        Ok(Some(Self::flush_internal(mapping)))
    }

    /// Internal serialization and batch construction helper in pure Rust.
    fn flush_internal(mapping: &mut MessageMapping) -> DispatchedBatch {
        let count = mapping.buffer.len();
        let items: Vec<TelemetryItem> = mapping.buffer.drain(..).collect();

        let payload = match mapping.serialization {
            SerializationFormat::Json => {
                let mut json_str = String::from("{\"events\":[");
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        json_str.push(',');
                    }
                    let target = item.target_id.as_deref().unwrap_or("null");
                    json_str.push_str(&format!(
                        "{{\"nf\":\"{}\",\"event\":\"{}\",\"target\":\"{}\",\"value\":{:.2},\"ts\":{}}}",
                        item.source_nf, item.event_name, target, item.value, item.timestamp_epoch_s
                    ));
                }
                json_str.push_str("]}");
                json_str.into_bytes()
            }
            SerializationFormat::CompactBinary => {
                // Header: [0xFF, 0x5C (5G Core Tag), count as u16]
                let mut bytes = Vec::with_capacity(4 + count * 16);
                bytes.push(0xFF);
                bytes.push(0x5C);
                bytes.extend_from_slice(&(count as u16).to_be_bytes());

                for item in &items {
                    bytes.extend_from_slice(&item.timestamp_epoch_s.to_be_bytes());
                    bytes.extend_from_slice(&item.value.to_bits().to_be_bytes());
                }
                bytes
            }
        };

        DispatchedBatch {
            mapping_id: mapping.mapping_id.clone(),
            protocol: mapping.protocol,
            destination_topic: mapping.destination_topic.clone(),
            payload,
            record_count: count,
        }
    }
}
