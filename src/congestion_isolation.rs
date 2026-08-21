//! IEEE 802.1Qcz Congestion Isolation (CI / Data Center TSN & RoCEv2 PFC Mitigation).
//!
//! Implements flow-level congestion tracking, ECN/PFC victim flow mitigation,
//! and automated isolation queue assignment to prevent Head-of-Line (HoL) blocking.

use crate::ipv4::Ipv4Address;

/// Congestion Isolation Flow State
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowIsolationState {
    Normal,
    Monitoring,
    Isolated,
    Restoring,
}

/// 5-Tuple Flow Key for Congestion Tracking
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CongestionFlowKey {
    pub src_ip: Ipv4Address,
    pub dst_ip: Ipv4Address,
    pub protocol: u8,
    pub src_port: u16,
    pub dst_port: u16,
}

/// Flow Congestion Entry
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowCongestionEntry {
    pub key: CongestionFlowKey,
    pub state: FlowIsolationState,
    pub ecn_ce_count: u32,
    pub last_seen_us: u64,
    pub assigned_queue_id: u8, // 0 = Standard Queue, 1 = Isolated Queue
}

/// IEEE 802.1Qcz Congestion Isolation Engine
#[derive(Debug, Clone)]
pub struct CongestionIsolationEngine {
    pub ecn_threshold_ce_marks: u32,
    pub isolation_queue_id: u8,
    pub standard_queue_id: u8,
    pub flows: Vec<FlowCongestionEntry>,
    pub total_isolated_flows: usize,
    pub total_cnp_sent: usize,
}

impl CongestionIsolationEngine {
    pub fn new(ecn_threshold_ce_marks: u32) -> Self {
        CongestionIsolationEngine {
            ecn_threshold_ce_marks,
            isolation_queue_id: 1,
            standard_queue_id: 0,
            flows: Vec::new(),
            total_isolated_flows: 0,
            total_cnp_sent: 0,
        }
    }

    /// Processes an incoming packet with ECN bits (0..3) at given timestamp
    pub fn process_packet(
        &mut self,
        key: CongestionFlowKey,
        ecn_bits: u8,
        timestamp_us: u64,
    ) -> u8 {
        let is_ce = ecn_bits == 0x03; // ECN Congestion Encountered (CE)

        let entry = if let Some(pos) = self.flows.iter().position(|f| f.key == key) {
            &mut self.flows[pos]
        } else {
            self.flows.push(FlowCongestionEntry {
                key: key.clone(),
                state: FlowIsolationState::Normal,
                ecn_ce_count: 0,
                last_seen_us: timestamp_us,
                assigned_queue_id: self.standard_queue_id,
            });
            self.flows.last_mut().unwrap()
        };

        entry.last_seen_us = timestamp_us;

        if is_ce {
            entry.ecn_ce_count += 1;
            if entry.ecn_ce_count >= self.ecn_threshold_ce_marks
                && entry.state != FlowIsolationState::Isolated
            {
                entry.state = FlowIsolationState::Isolated;
                entry.assigned_queue_id = self.isolation_queue_id;
                self.total_isolated_flows += 1;
                self.total_cnp_sent += 1; // Trigger Congestion Notification Packet
            }
        }

        entry.assigned_queue_id
    }

    /// Periodic aging: restores flows to normal queue if no CE marks observed
    pub fn age_flows(&mut self, current_time_us: u64, silence_timeout_us: u64) {
        for flow in &mut self.flows {
            if flow.state == FlowIsolationState::Isolated {
                if current_time_us.saturating_sub(flow.last_seen_us) > silence_timeout_us {
                    flow.state = FlowIsolationState::Restoring;
                    flow.assigned_queue_id = self.standard_queue_id;
                    flow.ecn_ce_count = 0;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_congestion_isolation_flow_transition() {
        let mut engine = CongestionIsolationEngine::new(3); // 3 CE marks trigger isolation
        let flow_key = CongestionFlowKey {
            src_ip: Ipv4Address::new(10, 0, 0, 10),
            dst_ip: Ipv4Address::new(10, 0, 0, 20),
            protocol: 17, // UDP / RoCEv2
            src_port: 50000,
            dst_port: 4791,
        };

        // 1st CE mark
        let q1 = engine.process_packet(flow_key.clone(), 0x03, 1000);
        assert_eq!(q1, 0); // Standard queue

        // 2nd CE mark
        let q2 = engine.process_packet(flow_key.clone(), 0x03, 1050);
        assert_eq!(q2, 0); // Standard queue

        // 3rd CE mark -> triggers isolation
        let q3 = engine.process_packet(flow_key.clone(), 0x03, 1100);
        assert_eq!(q3, 1); // Isolated queue
        assert_eq!(engine.total_isolated_flows, 1);
        assert_eq!(engine.total_cnp_sent, 1);

        // Subsequent packets routed to isolated queue
        let q4 = engine.process_packet(flow_key.clone(), 0x00, 1150);
        assert_eq!(q4, 1);

        // Age flow after timeout
        engine.age_flows(5000, 2000);
        assert_eq!(engine.flows[0].assigned_queue_id, 0);
    }
}
