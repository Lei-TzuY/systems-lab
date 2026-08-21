//! 5G Core Network Exposure Function (NEF) Traffic Influence Service (3GPP TS 29.522 Nnef_TrafficInfluence).
//!
//! Implements Edge Computing (MEC) local breakout and dynamic UPF traffic steering rules
//! requested by external Application Functions (AF).

use crate::ipv4::Ipv4Address;

/// 5G S-NSSAI Slice Identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SliceId {
    pub sst: u8,
    pub sd: u32,
}

/// NEF Traffic Filter Description
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrafficFilter {
    pub dst_ip: Ipv4Address,
    pub dst_port: u16,
    pub protocol: u8,
}

/// NEF Traffic Influence Subscription (3GPP TS 29.522)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NefTrafficInfluenceSub {
    pub sub_id: u32,
    pub af_trans_id: String,
    pub af_service_id: String,
    pub dnn: String,
    pub snssai: SliceId,
    pub filter: TrafficFilter,
    pub target_dnai: String,         // Data Network Access Identifier (e.g., "DNAI-Edge-01")
    pub edge_server_ip: Ipv4Address, // Local MEC breakout EAS IP
}

/// Active Edge Steering Match Result
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeSteeringDecision {
    pub matched_sub_id: u32,
    pub target_dnai: String,
    pub local_breakout_ip: Ipv4Address,
}

/// 5G Core NEF Traffic Influence Engine
#[derive(Debug, Clone, Default)]
pub struct NefTrafficInfluenceEngine {
    pub subscriptions: Vec<NefTrafficInfluenceSub>,
    pub next_sub_id: u32,
    pub total_steered_packets: usize,
}

impl NefTrafficInfluenceEngine {
    pub fn new() -> Self {
        NefTrafficInfluenceEngine {
            subscriptions: Vec::new(),
            next_sub_id: 1,
            total_steered_packets: 0,
        }
    }

    /// Registers a new Traffic Influence subscription from AF
    pub fn create_subscription(
        &mut self,
        af_trans_id: &str,
        af_service_id: &str,
        dnn: &str,
        snssai: SliceId,
        filter: TrafficFilter,
        target_dnai: &str,
        edge_server_ip: Ipv4Address,
    ) -> u32 {
        let sub_id = self.next_sub_id;
        self.next_sub_id += 1;

        self.subscriptions.push(NefTrafficInfluenceSub {
            sub_id,
            af_trans_id: af_trans_id.to_string(),
            af_service_id: af_service_id.to_string(),
            dnn: dnn.to_string(),
            snssai,
            filter,
            target_dnai: target_dnai.to_string(),
            edge_server_ip,
        });

        sub_id
    }

    /// Evaluates PDU session packet against active Edge Traffic Influence rules
    pub fn evaluate_packet(
        &mut self,
        dnn: &str,
        snssai: &SliceId,
        dst_ip: Ipv4Address,
        dst_port: u16,
        protocol: u8,
    ) -> Option<EdgeSteeringDecision> {
        for sub in &self.subscriptions {
            if sub.dnn == dnn
                && &sub.snssai == snssai
                && sub.filter.dst_ip == dst_ip
                && sub.filter.dst_port == dst_port
                && sub.filter.protocol == protocol
            {
                self.total_steered_packets += 1;
                return Some(EdgeSteeringDecision {
                    matched_sub_id: sub.sub_id,
                    target_dnai: sub.target_dnai.clone(),
                    local_breakout_ip: sub.edge_server_ip,
                });
            }
        }
        None
    }

    /// Deletes an AF Traffic Influence subscription
    pub fn delete_subscription(&mut self, sub_id: u32) -> bool {
        let initial_len = self.subscriptions.len();
        self.subscriptions.retain(|s| s.sub_id != sub_id);
        self.subscriptions.len() < initial_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nef_traffic_influence_edge_breakout() {
        let mut engine = NefTrafficInfluenceEngine::new();
        let slice = SliceId { sst: 1, sd: 0x000001 };
        let filter = TrafficFilter {
            dst_ip: Ipv4Address::new(198, 51, 100, 10),
            dst_port: 8080,
            protocol: 6, // TCP
        };

        let sub_id = engine.create_subscription(
            "tx-af-001",
            "edge-vr-cloud",
            "edge.mec",
            slice.clone(),
            filter,
            "DNAI-Taipei-Edge",
            Ipv4Address::new(10, 200, 1, 5),
        );
        assert_eq!(sub_id, 1);

        // Matching packet evaluation
        let decision = engine
            .evaluate_packet(
                "edge.mec",
                &slice,
                Ipv4Address::new(198, 51, 100, 10),
                8080,
                6,
            )
            .unwrap();

        assert_eq!(decision.matched_sub_id, 1);
        assert_eq!(decision.target_dnai, "DNAI-Taipei-Edge");
        assert_eq!(decision.local_breakout_ip, Ipv4Address::new(10, 200, 1, 5));
        assert_eq!(engine.total_steered_packets, 1);

        // Non-matching evaluation
        let no_match = engine.evaluate_packet(
            "internet",
            &slice,
            Ipv4Address::new(8, 8, 8, 8),
            53,
            17,
        );
        assert!(no_match.is_none());
    }
}
