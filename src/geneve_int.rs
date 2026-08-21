//! Geneve In-Band Network Telemetry (INT-over-Geneve - RFC 8926 / P4 INT Architecture).
//!
//! Embeds per-hop switch telemetry (Switch ID, Ingress/Egress Ports, Hop Latency, Queue Depth)
//! directly within Geneve options for real-time datacenter performance monitoring.

use crate::geneve::{GeneveOption, GenevePacket};
use crate::geneve_opts::GeneveOptionTlv;

pub const GENEVE_OPT_CLASS_INT: u16 = 0x0105;
pub const GENEVE_OPT_TYPE_INT_HOP: u8 = 0x01;
pub const INT_HOP_METADATA_LEN: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntHopTelemetry {
    pub switch_id: u32,
    pub ingress_port: u16,
    pub egress_port: u16,
    pub hop_latency_ns: u32,
    pub queue_depth_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneveIntPacket {
    pub vni: u32,
    pub protocol_type: u16,
    pub telemetry_hops: Vec<IntHopTelemetry>,
    pub payload: Vec<u8>,
}

impl GeneveIntPacket {
    pub fn build(
        vni: u32,
        protocol_type: u16,
        telemetry_hops: Vec<IntHopTelemetry>,
        payload: &[u8],
    ) -> Self {
        GeneveIntPacket {
            vni,
            protocol_type,
            telemetry_hops,
            payload: payload.to_vec(),
        }
    }

    pub fn add_hop_telemetry(&mut self, hop: IntHopTelemetry) {
        self.telemetry_hops.push(hop);
    }

    pub fn calculate_total_latency_ns(&self) -> u32 {
        self.telemetry_hops.iter().map(|h| h.hop_latency_ns).sum()
    }

    pub fn max_queue_depth_bytes(&self) -> u32 {
        self.telemetry_hops.iter().map(|h| h.queue_depth_bytes).max().unwrap_or(0)
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut options = Vec::new();

        for hop in &self.telemetry_hops {
            let mut data = Vec::with_capacity(INT_HOP_METADATA_LEN);
            data.extend_from_slice(&hop.switch_id.to_be_bytes());
            data.extend_from_slice(&hop.ingress_port.to_be_bytes());
            data.extend_from_slice(&hop.egress_port.to_be_bytes());
            data.extend_from_slice(&hop.hop_latency_ns.to_be_bytes());
            data.extend_from_slice(&hop.queue_depth_bytes.to_be_bytes());

            let opt_tlv = GeneveOptionTlv::new(
                GENEVE_OPT_CLASS_INT,
                GENEVE_OPT_TYPE_INT_HOP,
                false,
                &data,
            );
            options.push(GeneveOption {
                class: opt_tlv.class,
                opt_type: opt_tlv.type_code,
                critical: opt_tlv.critical,
                data: opt_tlv.data,
            });
        }

        let geneve_pkt = GenevePacket {
            version: 0,
            oam: true, // OAM bit set for telemetry packets
            critical: false,
            protocol_type: self.protocol_type,
            vni: self.vni,
            options,
            payload: self.payload.clone(),
        };

        geneve_pkt.serialize()
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        let geneve = GenevePacket::parse(data).ok()?;
        let mut telemetry_hops = Vec::new();

        for opt in &geneve.options {
            if opt.class == GENEVE_OPT_CLASS_INT && opt.opt_type == GENEVE_OPT_TYPE_INT_HOP && opt.data.len() >= INT_HOP_METADATA_LEN {
                let switch_id = u32::from_be_bytes([opt.data[0], opt.data[1], opt.data[2], opt.data[3]]);
                let ingress_port = u16::from_be_bytes([opt.data[4], opt.data[5]]);
                let egress_port = u16::from_be_bytes([opt.data[6], opt.data[7]]);
                let hop_latency_ns = u32::from_be_bytes([opt.data[8], opt.data[9], opt.data[10], opt.data[11]]);
                let queue_depth_bytes = u32::from_be_bytes([opt.data[12], opt.data[13], opt.data[14], opt.data[15]]);

                telemetry_hops.push(IntHopTelemetry {
                    switch_id,
                    ingress_port,
                    egress_port,
                    hop_latency_ns,
                    queue_depth_bytes,
                });
            }
        }

        Some(GeneveIntPacket {
            vni: geneve.vni,
            protocol_type: geneve.protocol_type,
            telemetry_hops,
            payload: geneve.payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geneve_int_hop_recording_and_metrics() {
        let mut int_pkt = GeneveIntPacket::build(1001, 0x0800, Vec::new(), b"HTTP Payload Data");

        // Leaf 1
        int_pkt.add_hop_telemetry(IntHopTelemetry {
            switch_id: 101,
            ingress_port: 1,
            egress_port: 48,
            hop_latency_ns: 450,
            queue_depth_bytes: 2048,
        });

        // Spine 1
        int_pkt.add_hop_telemetry(IntHopTelemetry {
            switch_id: 201,
            ingress_port: 12,
            egress_port: 14,
            hop_latency_ns: 320,
            queue_depth_bytes: 8192,
        });

        // Leaf 2
        int_pkt.add_hop_telemetry(IntHopTelemetry {
            switch_id: 102,
            ingress_port: 48,
            egress_port: 4,
            hop_latency_ns: 410,
            queue_depth_bytes: 1024,
        });

        assert_eq!(int_pkt.calculate_total_latency_ns(), 1180);
        assert_eq!(int_pkt.max_queue_depth_bytes(), 8192);

        let raw = int_pkt.serialize();
        let parsed = GeneveIntPacket::parse(&raw).unwrap();

        assert_eq!(parsed.vni, 1001);
        assert_eq!(parsed.telemetry_hops.len(), 3);
        assert_eq!(parsed.telemetry_hops[1].switch_id, 201);
        assert_eq!(parsed.calculate_total_latency_ns(), 1180);
    }
}
