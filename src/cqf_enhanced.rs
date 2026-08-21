//! IEEE 802.1Qch Enhanced Cyclic Queuing and Forwarding (CQF / TSN Ping-Pong Dual Buffer).
//!
//! Implements strict cycle-synchronized double-buffering where frames received in cycle `i`
//! are scheduled for transmission strictly in cycle `i+1`, guaranteeing zero jitter.

/// CQF Ping-Pong Buffer Phase
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CqfPhase {
    Even, // Queue 0 Rx, Queue 1 Tx
    Odd,  // Queue 1 Rx, Queue 0 Tx
}

/// CQF Timed Frame
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CqfBufferedFrame {
    pub frame_id: u32,
    pub ingress_time_us: u64,
    pub size_bytes: usize,
    pub payload: Vec<u8>,
}

/// IEEE 802.1Qch Ping-Pong Dual-Queue Engine
#[derive(Debug, Clone)]
pub struct CqfDualBufferEngine {
    pub cycle_duration_us: u64,
    pub queue_capacity_bytes: usize,
    pub current_cycle_index: u64,
    pub phase: CqfPhase,
    pub queue_even: Vec<CqfBufferedFrame>,
    pub queue_odd: Vec<CqfBufferedFrame>,
    pub total_enqueued: usize,
    pub total_drained: usize,
    pub total_dropped: usize,
}

impl CqfDualBufferEngine {
    pub fn new(cycle_duration_us: u64, queue_capacity_bytes: usize) -> Self {
        CqfDualBufferEngine {
            cycle_duration_us,
            queue_capacity_bytes,
            current_cycle_index: 0,
            phase: CqfPhase::Even,
            queue_even: Vec::new(),
            queue_odd: Vec::new(),
            total_enqueued: 0,
            total_drained: 0,
            total_dropped: 0,
        }
    }

    /// Advances simulation time to current timestamp, switching ping-pong phase if cycle boundary crossed
    pub fn update_time(&mut self, now_us: u64) -> bool {
        let new_cycle = now_us / self.cycle_duration_us;
        if new_cycle != self.current_cycle_index {
            self.current_cycle_index = new_cycle;
            self.phase = if new_cycle % 2 == 0 {
                CqfPhase::Even
            } else {
                CqfPhase::Odd
            };
            true // Cycle switched
        } else {
            false
        }
    }

    /// Ingress: Enqueues a packet into the current receiving queue
    pub fn enqueue_frame(
        &mut self,
        frame_id: u32,
        ingress_time_us: u64,
        payload: Vec<u8>,
    ) -> bool {
        self.update_time(ingress_time_us);
        let size_bytes = payload.len();

        let rx_queue = match self.phase {
            CqfPhase::Even => &mut self.queue_even,
            CqfPhase::Odd => &mut self.queue_odd,
        };

        let current_bytes: usize = rx_queue.iter().map(|f| f.size_bytes).sum();
        if current_bytes + size_bytes > self.queue_capacity_bytes {
            self.total_dropped += 1;
            return false;
        }

        rx_queue.push(CqfBufferedFrame {
            frame_id,
            ingress_time_us,
            size_bytes,
            payload,
        });
        self.total_enqueued += 1;
        true
    }

    /// Egress: Drains and transmits all ready packets from the transmitting queue
    pub fn drain_transmitting_queue(&mut self, now_us: u64) -> Vec<CqfBufferedFrame> {
        self.update_time(now_us);
        let tx_queue = match self.phase {
            CqfPhase::Even => &mut self.queue_odd, // Even phase transmits Odd queue
            CqfPhase::Odd => &mut self.queue_even, // Odd phase transmits Even queue
        };

        let frames = std::mem::take(tx_queue);
        self.total_drained += frames.len();
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cqf_ping_pong_cycle_switching_and_drain() {
        let mut engine = CqfDualBufferEngine::new(1000, 10000); // 1000µs cycle

        // Cycle 0 (t=100µs, Phase: Even): Enqueue into Even queue
        assert!(engine.enqueue_frame(1, 100, vec![0x11; 500]));
        assert_eq!(engine.queue_even.len(), 1);
        assert_eq!(engine.queue_odd.len(), 0);

        // During Cycle 0, Tx queue is Odd (empty)
        let drained_c0 = engine.drain_transmitting_queue(500);
        assert_eq!(drained_c0.len(), 0);

        // Advance to Cycle 1 (t=1200µs, Phase: Odd):
        // 1. Enqueue frame 2 into Odd queue
        assert!(engine.enqueue_frame(2, 1200, vec![0x22; 800]));
        assert_eq!(engine.queue_odd.len(), 1);

        // 2. Transmit from Even queue (contains frame 1 from cycle 0)
        let drained_c1 = engine.drain_transmitting_queue(1500);
        assert_eq!(drained_c1.len(), 1);
        assert_eq!(drained_c1[0].frame_id, 1);
        assert_eq!(engine.queue_even.len(), 0);
    }
}
