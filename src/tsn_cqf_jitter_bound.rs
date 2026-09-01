// =============================================================================
// IEEE 802.1Qch CQF Multi-Hop Jitter Accumulation & Bounded Delay Predictor
// =============================================================================
//
// In deterministic Time-Sensitive Networks (TSN), critical industrial and
// automotive control loops demand mathematically bounded end-to-end latency
// and jitter. Under IEEE 802.1Qch Cyclic Queuing and Forwarding (CQF):
//
// For an N-hop path where hop `i` has:
//   • Cycle Duration `T_cycle[i]`
//   • Link Propagation Delay `d_prop[i]`
//   • Bridge Processing Window `[d_proc_min[i], d_proc_max[i]]`
//
// Theoretical Delay & Jitter Bounds:
//   • Minimum Delay = Σ ( T_cycle[i] + d_prop[i] + d_proc_min[i] )
//   • Maximum Delay = Σ ( 2 * T_cycle[i] + d_prop[i] + d_proc_max[i] )
//   • Jitter Bound  = Maximum Delay - Minimum Delay
//                   = Σ ( T_cycle[i] + (d_proc_max[i] - d_proc_min[i]) )
//
// Features:
//   1. Multi-Hop Path Topology Model: Configurable per-hop cycle, prop, and proc times.
//   2. Deterministic Delay & Jitter Evaluation: Computes min/max/jitter bounds in ns.
//   3. Stream SLA Compliance Verification: Validates path characteristics against
//      max allowable latency and jitter constraints.
//
// All timing arithmetic uses integer nanoseconds (u64). Safe Rust, zero crates.

/// Hop specification along a deterministic CQF path.
#[derive(Debug, Clone)]
pub struct CqfHopProfile {
    pub hop_id: u32,
    pub name: String,
    /// Cycle duration in nanoseconds.
    pub cycle_time_ns: u64,
    /// Link propagation delay in nanoseconds.
    pub link_prop_ns: u64,
    /// Minimum internal bridge forwarding delay in nanoseconds.
    pub bridge_proc_min_ns: u64,
    /// Maximum internal bridge forwarding delay in nanoseconds.
    pub bridge_proc_max_ns: u64,
}

/// End-to-end delay and jitter calculation results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CqfPathDelayBound {
    pub hop_count: usize,
    pub min_delay_ns: u64,
    pub max_delay_ns: u64,
    pub jitter_bound_ns: u64,
}

/// SLA compliance result for a specific stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlaComplianceResult {
    /// Path meets all latency and jitter requirements.
    Compliant,
    /// Maximum delay exceeds the stream's allowable limit.
    LatencyViolation {
        calculated_max_ns: u64,
        allowed_max_ns: u64,
    },
    /// Jitter exceeds the stream's allowable jitter limit.
    JitterViolation {
        calculated_jitter_ns: u64,
        allowed_jitter_ns: u64,
    },
    /// Both latency and jitter limits are violated.
    DualViolation {
        calculated_max_ns: u64,
        allowed_max_ns: u64,
        calculated_jitter_ns: u64,
        allowed_jitter_ns: u64,
    },
}

/// TSN CQF Multi-Hop Jitter Accumulation & Bounded Delay Predictor.
pub struct TsnCqfJitterBoundEngine {
    pub hops: Vec<CqfHopProfile>,
}

impl TsnCqfJitterBoundEngine {
    pub fn new() -> Self {
        Self { hops: Vec::new() }
    }

    /// Add a bridge hop profile to the end of the CQF path.
    pub fn add_hop(&mut self, hop: CqfHopProfile) {
        self.hops.push(hop);
    }

    /// Clear all configured hops in the path.
    pub fn clear_hops(&mut self) {
        self.hops.clear();
    }

    /// Compute theoretical minimum, maximum delay, and jitter bounds across all hops.
    pub fn compute_bounds(&self) -> CqfPathDelayBound {
        let mut min_delay_ns: u64 = 0;
        let mut max_delay_ns: u64 = 0;

        for hop in &self.hops {
            // Min delay contribution: 1 cycle + prop + proc_min
            let hop_min = hop
                .cycle_time_ns
                .saturating_add(hop.link_prop_ns)
                .saturating_add(hop.bridge_proc_min_ns);

            // Max delay contribution: 2 cycles + prop + proc_max
            let hop_max = hop
                .cycle_time_ns
                .saturating_mul(2)
                .saturating_add(hop.link_prop_ns)
                .saturating_add(hop.bridge_proc_max_ns);

            min_delay_ns = min_delay_ns.saturating_add(hop_min);
            max_delay_ns = max_delay_ns.saturating_add(hop_max);
        }

        let jitter_bound_ns = max_delay_ns.saturating_sub(min_delay_ns);

        CqfPathDelayBound {
            hop_count: self.hops.len(),
            min_delay_ns,
            max_delay_ns,
            jitter_bound_ns,
        }
    }

    /// Check if the path complies with a stream's strict QoS/SLA requirements.
    pub fn evaluate_stream_sla(
        &self,
        max_allowable_latency_ns: u64,
        max_allowable_jitter_ns: u64,
    ) -> SlaComplianceResult {
        let bounds = self.compute_bounds();

        let latency_violation = bounds.max_delay_ns > max_allowable_latency_ns;
        let jitter_violation = bounds.jitter_bound_ns > max_allowable_jitter_ns;

        match (latency_violation, jitter_violation) {
            (true, true) => SlaComplianceResult::DualViolation {
                calculated_max_ns: bounds.max_delay_ns,
                allowed_max_ns: max_allowable_latency_ns,
                calculated_jitter_ns: bounds.jitter_bound_ns,
                allowed_jitter_ns: max_allowable_jitter_ns,
            },
            (true, false) => SlaComplianceResult::LatencyViolation {
                calculated_max_ns: bounds.max_delay_ns,
                allowed_max_ns: max_allowable_latency_ns,
            },
            (false, true) => SlaComplianceResult::JitterViolation {
                calculated_jitter_ns: bounds.jitter_bound_ns,
                allowed_jitter_ns: max_allowable_jitter_ns,
            },
            (false, false) => SlaComplianceResult::Compliant,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cqf_delay_and_jitter_bounds() {
        let mut engine = TsnCqfJitterBoundEngine::new();

        // Hop 1: 100 µs cycle, 5 µs prop, 2..4 µs proc
        engine.add_hop(CqfHopProfile {
            hop_id: 1,
            name: "Switch-A".to_string(),
            cycle_time_ns: 100_000,
            link_prop_ns: 5_000,
            bridge_proc_min_ns: 2_000,
            bridge_proc_max_ns: 4_000,
        });

        // Hop 2: 100 µs cycle, 5 µs prop, 2..4 µs proc
        engine.add_hop(CqfHopProfile {
            hop_id: 2,
            name: "Switch-B".to_string(),
            cycle_time_ns: 100_000,
            link_prop_ns: 5_000,
            bridge_proc_min_ns: 2_000,
            bridge_proc_max_ns: 4_000,
        });

        let bounds = engine.compute_bounds();
        // Hop Min = 100_000 + 5_000 + 2_000 = 107_000 ns -> 2 hops = 214_000 ns (214 µs)
        // Hop Max = 200_000 + 5_000 + 4_000 = 209_000 ns -> 2 hops = 418_000 ns (418 µs)
        // Jitter = 418_000 - 214_000 = 204_000 ns (204 µs)
        assert_eq!(bounds.hop_count, 2);
        assert_eq!(bounds.min_delay_ns, 214_000);
        assert_eq!(bounds.max_delay_ns, 418_000);
        assert_eq!(bounds.jitter_bound_ns, 204_000);

        // Stream SLA: 500 µs max delay, 250 µs max jitter -> Compliant
        assert_eq!(
            engine.evaluate_stream_sla(500_000, 250_000),
            SlaComplianceResult::Compliant
        );

        // Stream SLA: 300 µs max delay -> Latency violation
        assert!(matches!(
            engine.evaluate_stream_sla(300_000, 250_000),
            SlaComplianceResult::LatencyViolation { .. }
        ));
    }
}
