// =============================================================================
// 3GPP TS 38.415 / ITU-T Y.1731 5G GTP-U In-Band Packet Loss Measurement (LMM/LMR)
// =============================================================================
//
// Reliable SLA auditing and SLA assurance for 5G User Plane traffic requires
// continuous packet loss measurement. ITU-T Y.1731 / 3GPP TS 38.415 specify
// dual-ended in-band counter exchange where endpoints compare forward and
// backward transmit/receive frame counts:
//
//   • Forward Loss (Far-End Loss) : ΔTx_fwd - ΔRx_fwd
//   • Backward Loss (Near-End Loss): ΔTx_rev - ΔRx_rev
//   • Packet Loss Ratio (PLR)     : Loss / Total_Tx
//
// Features:
//   1. Loss Measurement Message (LMM) and Reply (LMR) packet construction.
//   2. Dual-ended monotonic frame counter tracking.
//   3. Near-End and Far-End packet loss computation with fixed-point loss ratio.
//   4. Per-QFI / Session telemetry aggregation.
//
// Pure safe Rust, zero external crates.

/// Dual-ended Packet Loss Measurement sample result.
#[derive(Debug, Clone)]
pub struct PlmMeasurementResult {
    pub session_id: u32,
    pub qfi: u8,
    pub forward_tx_count: u64,
    pub forward_rx_count: u64,
    pub backward_tx_count: u64,
    pub backward_rx_count: u64,
    pub far_end_loss_frames: u64,
    pub near_end_loss_frames: u64,
    /// Loss ratio in basis points (1 bp = 0.01%, 10000 bp = 100.00%).
    pub far_end_loss_ratio_bp: u32,
    pub near_end_loss_ratio_bp: u32,
}

/// In-Band Loss Measurement Message (LMM/LMR).
#[derive(Debug, Clone)]
pub struct GtpuLossMessage {
    pub session_id: u32,
    pub qfi: u8,
    pub is_reply: bool,
    /// Transmit Frame Count Forward (at origin sender).
    pub tx_fc_f: u64,
    /// Receive Frame Count Forward (at remote reflector).
    pub rx_fc_f: u64,
    /// Transmit Frame Count Backward (at remote reflector).
    pub tx_fc_b: u64,
    /// Timestamp in microseconds.
    pub timestamp_us: u64,
}

/// Per-session/QFI loss tracking state.
#[derive(Debug, Clone, Default)]
pub struct SessionLossCounters {
    pub tx_packets_forward: u64,
    pub rx_packets_forward: u64,
    pub tx_packets_backward: u64,
    pub rx_packets_backward: u64,
    pub last_far_end_loss: u64,
    pub last_near_end_loss: u64,
    pub total_measurements: u64,
}

/// 5G GTP-U In-Band Packet Loss Measurement (LMM/LMR) Engine.
pub struct GtpuLossTelemetryEngine {
    pub session_counters: Vec<(u32, u8, SessionLossCounters)>, // (session_id, qfi, counters)
    pub total_lmm_generated: u64,
    pub total_lmr_processed: u64,
}

impl GtpuLossTelemetryEngine {
    pub fn new() -> Self {
        Self {
            session_counters: Vec::new(),
            total_lmm_generated: 0,
            total_lmr_processed: 0,
        }
    }

    /// Record a transmitted user-plane data packet.
    pub fn record_tx_packet(&mut self, session_id: u32, qfi: u8) {
        let entry = self.get_or_create_counters(session_id, qfi);
        entry.tx_packets_forward = entry.tx_packets_forward.saturating_add(1);
    }

    /// Record a received user-plane data packet.
    pub fn record_rx_packet(&mut self, session_id: u32, qfi: u8) {
        let entry = self.get_or_create_counters(session_id, qfi);
        entry.rx_packets_forward = entry.rx_packets_forward.saturating_add(1);
    }

    /// Generate an outgoing Loss Measurement Message (LMM) query.
    pub fn create_lmm(&mut self, session_id: u32, qfi: u8, now_us: u64) -> GtpuLossMessage {
        self.total_lmm_generated += 1;
        let entry = self.get_or_create_counters(session_id, qfi);
        GtpuLossMessage {
            session_id,
            qfi,
            is_reply: false,
            tx_fc_f: entry.tx_packets_forward,
            rx_fc_f: 0,
            tx_fc_b: 0,
            timestamp_us: now_us,
        }
    }

    /// Remote reflector handles incoming LMM and generates LMR response.
    pub fn handle_lmm_as_reflector(
        &mut self,
        lmm: &GtpuLossMessage,
        now_us: u64,
    ) -> GtpuLossMessage {
        let entry = self.get_or_create_counters(lmm.session_id, lmm.qfi);
        GtpuLossMessage {
            session_id: lmm.session_id,
            qfi: lmm.qfi,
            is_reply: true,
            tx_fc_f: lmm.tx_fc_f,
            rx_fc_f: entry.rx_packets_forward,
            tx_fc_b: entry.tx_packets_backward,
            timestamp_us: now_us,
        }
    }

    /// Origin sender processes received LMR and computes dual-ended loss.
    pub fn process_lmr(&mut self, lmr: &GtpuLossMessage) -> PlmMeasurementResult {
        self.total_lmr_processed += 1;
        let entry = self.get_or_create_counters(lmr.session_id, lmr.qfi);
        entry.total_measurements += 1;

        // Far-End Loss = Tx_fwd - Rx_fwd
        let far_end_loss = lmr.tx_fc_f.saturating_sub(lmr.rx_fc_f);
        // Near-End Loss = Tx_bwd - Rx_bwd
        let near_end_loss = lmr.tx_fc_b.saturating_sub(entry.rx_packets_backward);

        let far_end_ratio_bp = if lmr.tx_fc_f > 0 {
            ((far_end_loss as u128 * 10_000) / lmr.tx_fc_f as u128) as u32
        } else {
            0
        };

        let near_end_ratio_bp = if lmr.tx_fc_b > 0 {
            ((near_end_loss as u128 * 10_000) / lmr.tx_fc_b as u128) as u32
        } else {
            0
        };

        entry.last_far_end_loss = far_end_loss;
        entry.last_near_end_loss = near_end_loss;

        PlmMeasurementResult {
            session_id: lmr.session_id,
            qfi: lmr.qfi,
            forward_tx_count: lmr.tx_fc_f,
            forward_rx_count: lmr.rx_fc_f,
            backward_tx_count: lmr.tx_fc_b,
            backward_rx_count: entry.rx_packets_backward,
            far_end_loss_frames: far_end_loss,
            near_end_loss_frames: near_end_loss,
            far_end_loss_ratio_bp: far_end_ratio_bp.min(10000),
            near_end_loss_ratio_bp: near_end_ratio_bp.min(10000),
        }
    }

    fn get_or_create_counters(&mut self, session_id: u32, qfi: u8) -> &mut SessionLossCounters {
        if let Some(pos) = self
            .session_counters
            .iter()
            .position(|(s, q, _)| *s == session_id && *q == qfi)
        {
            &mut self.session_counters[pos].2
        } else {
            self.session_counters
                .push((session_id, qfi, SessionLossCounters::default()));
            &mut self.session_counters.last_mut().unwrap().2
        }
    }

    pub fn get_counters(&self, session_id: u32, qfi: u8) -> Option<&SessionLossCounters> {
        self.session_counters
            .iter()
            .find(|(s, q, _)| *s == session_id && *q == qfi)
            .map(|(_, _, c)| c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loss_measurement_lifecycle() {
        let mut sender = GtpuLossTelemetryEngine::new();
        let mut reflector = GtpuLossTelemetryEngine::new();

        let session_id = 0x5001;
        let qfi = 9;

        // Sender transmits 1000 packets
        for _ in 0..1000 {
            sender.record_tx_packet(session_id, qfi);
        }

        // Reflector receives 980 packets (20 lost in forward path)
        for _ in 0..980 {
            reflector.record_rx_packet(session_id, qfi);
        }

        // Sender generates LMM
        let lmm = sender.create_lmm(session_id, qfi, 100_000);
        assert_eq!(lmm.tx_fc_f, 1000);

        // Reflector responds with LMR
        let lmr = reflector.handle_lmm_as_reflector(&lmm, 105_000);
        assert_eq!(lmr.rx_fc_f, 980);

        // Sender processes LMR
        let result = sender.process_lmr(&lmr);
        assert_eq!(result.far_end_loss_frames, 20);
        assert_eq!(result.far_end_loss_ratio_bp, 200); // 200 bp = 2.00%
    }
}
