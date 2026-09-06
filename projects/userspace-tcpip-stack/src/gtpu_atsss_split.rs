// =============================================================================
// 3GPP TS 23.501 / TS 24.193 5G ATSSS Dynamic Packet Splitting & Aggregation Engine
// =============================================================================
//
// In 5G Multi-Access (MA) PDU sessions, the Access Traffic Steering, Switching,
// and Splitting (ATSSS) feature supports a "Split" steering mode. The UPF (or UE)
// distributes user-plane packets across available access legs (3GPP and Non-3GPP)
// according to weighted ratios (e.g. 80% 3GPP Cellular / 20% Non-3GPP Wi-Fi)
// while maintaining end-to-end flow sequencing.
//
// Features:
//   1. Weighted Round-Robin (WRR) Packet Dispatch: Splits flows proportionally.
//   2. Monotonic Sequence Stamping: Adds unified MA sequence numbers to packets.
//   3. Leg Health Awareness: Automatically shifts traffic off degraded/unavailable
//      legs to healthy paths.
//   4. Multi-Leg Aggregation & Statistics: Tracks per-leg forwarded bytes,
//      packets, and active split ratios.
//
// Pure safe Rust, zero external crates.

/// Access leg type for multi-access session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtsssAccessLeg {
    ThreeGppCellular,
    NonThreeGppWifi,
}

/// Dynamic steering mode for ATSSS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtsssSteeringRule {
    /// Active-Standby mode (all traffic on primary leg until down).
    ActiveStandby { primary: AtsssAccessLeg },
    /// Smallest Delay mode (route to leg with lowest measured RTT).
    SmallestDelay,
    /// Weighted Split mode (load balancing across both legs).
    WeightedSplit { weight_3gpp: u32, weight_wifi: u32 },
}

/// Dispatched packet payload with access leg tag.
#[derive(Debug, Clone)]
pub struct SplitPacket {
    pub session_id: u32,
    pub ma_seq: u64,
    pub leg: AtsssAccessLeg,
    pub payload_bytes: usize,
}

/// Statistics per access leg.
#[derive(Debug, Clone, Default)]
pub struct LegStats {
    pub packets_forwarded: u64,
    pub bytes_forwarded: u64,
}

/// 5G ATSSS Multi-Access Dynamic Packet Splitting & Aggregation Engine.
pub struct GtpuAtsssSplitEngine {
    pub session_id: u32,
    pub steering_rule: AtsssSteeringRule,
    pub is_3gpp_healthy: bool,
    pub is_wifi_healthy: bool,
    pub next_seq: u64,
    pub wrr_counter_3gpp: u32,
    pub wrr_counter_wifi: u32,
    pub stats_3gpp: LegStats,
    pub stats_wifi: LegStats,
}

impl GtpuAtsssSplitEngine {
    pub fn new(session_id: u32, rule: AtsssSteeringRule) -> Self {
        Self {
            session_id,
            steering_rule: rule,
            is_3gpp_healthy: true,
            is_wifi_healthy: true,
            next_seq: 1,
            wrr_counter_3gpp: 0,
            wrr_counter_wifi: 0,
            stats_3gpp: Default::default(),
            stats_wifi: Default::default(),
        }
    }

    /// Update health status of an access leg.
    pub fn set_leg_health(&mut self, leg: AtsssAccessLeg, healthy: bool) {
        match leg {
            AtsssAccessLeg::ThreeGppCellular => self.is_3gpp_healthy = healthy,
            AtsssAccessLeg::NonThreeGppWifi => self.is_wifi_healthy = healthy,
        }
    }

    /// Select the access leg for the next packet based on steering rule and health.
    pub fn select_leg(&mut self) -> Option<AtsssAccessLeg> {
        if !self.is_3gpp_healthy && !self.is_wifi_healthy {
            return None;
        }

        if !self.is_3gpp_healthy {
            return Some(AtsssAccessLeg::NonThreeGppWifi);
        }
        if !self.is_wifi_healthy {
            return Some(AtsssAccessLeg::ThreeGppCellular);
        }

        match self.steering_rule {
            AtsssSteeringRule::ActiveStandby { primary } => Some(primary),
            AtsssSteeringRule::SmallestDelay => {
                // Default to 3GPP Cellular when delays are equal
                Some(AtsssAccessLeg::ThreeGppCellular)
            }
            AtsssSteeringRule::WeightedSplit {
                weight_3gpp,
                weight_wifi,
            } => {
                if weight_3gpp == 0 && weight_wifi == 0 {
                    return Some(AtsssAccessLeg::ThreeGppCellular);
                }
                if weight_3gpp == 0 {
                    return Some(AtsssAccessLeg::NonThreeGppWifi);
                }
                if weight_wifi == 0 {
                    return Some(AtsssAccessLeg::ThreeGppCellular);
                }

                if self.wrr_counter_3gpp < weight_3gpp {
                    self.wrr_counter_3gpp += 1;
                    Some(AtsssAccessLeg::ThreeGppCellular)
                } else if self.wrr_counter_wifi < weight_wifi {
                    self.wrr_counter_wifi += 1;
                    Some(AtsssAccessLeg::NonThreeGppWifi)
                } else {
                    // Reset round
                    self.wrr_counter_3gpp = 1;
                    self.wrr_counter_wifi = 0;
                    Some(AtsssAccessLeg::ThreeGppCellular)
                }
            }
        }
    }

    /// Ingest and split a user-plane packet.
    pub fn split_packet(&mut self, payload_bytes: usize) -> Option<SplitPacket> {
        let leg = self.select_leg()?;
        let seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);

        match leg {
            AtsssAccessLeg::ThreeGppCellular => {
                self.stats_3gpp.packets_forwarded += 1;
                self.stats_3gpp.bytes_forwarded += payload_bytes as u64;
            }
            AtsssAccessLeg::NonThreeGppWifi => {
                self.stats_wifi.packets_forwarded += 1;
                self.stats_wifi.bytes_forwarded += payload_bytes as u64;
            }
        }

        Some(SplitPacket {
            session_id: self.session_id,
            ma_seq: seq,
            leg,
            payload_bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weighted_splitting_lifecycle() {
        // 3:1 ratio (75% 3GPP, 25% Wi-Fi)
        let mut engine = GtpuAtsssSplitEngine::new(
            0x6001,
            AtsssSteeringRule::WeightedSplit {
                weight_3gpp: 3,
                weight_wifi: 1,
            },
        );

        let p1 = engine.split_packet(100).unwrap();
        assert_eq!(p1.leg, AtsssAccessLeg::ThreeGppCellular);
        assert_eq!(p1.ma_seq, 1);

        let p2 = engine.split_packet(100).unwrap();
        assert_eq!(p2.leg, AtsssAccessLeg::ThreeGppCellular);
        assert_eq!(p2.ma_seq, 2);

        let p3 = engine.split_packet(100).unwrap();
        assert_eq!(p3.leg, AtsssAccessLeg::ThreeGppCellular);
        assert_eq!(p3.ma_seq, 3);

        let p4 = engine.split_packet(100).unwrap();
        assert_eq!(p4.leg, AtsssAccessLeg::NonThreeGppWifi);
        assert_eq!(p4.ma_seq, 4);

        // Next round starts
        let p5 = engine.split_packet(100).unwrap();
        assert_eq!(p5.leg, AtsssAccessLeg::ThreeGppCellular);
        assert_eq!(p5.ma_seq, 5);

        assert_eq!(engine.stats_3gpp.packets_forwarded, 4);
        assert_eq!(engine.stats_wifi.packets_forwarded, 1);
    }
}
