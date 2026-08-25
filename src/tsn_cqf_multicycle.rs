//! IEEE 802.1Qch-2017 Cyclic Queuing and Forwarding (CQF) Multi-Queue Engine.
//!
//! CQF (also known as the Peristaltic Shaper) provides deterministic, bounded
//! latency and zero jitter by synchronizing frame transmission and reception
//! into discrete cyclic time intervals ($T_{cycle}$).
//!
//! This module implements:
//! * 3-Queue Cyclic Buffer ($Q_0, Q_1, Q_2$) rotation engine.
//! * Queue Roles:
//!   - `Receiving`: Accepts incoming frames for the current ingress cycle.
//!   - `Gated/Queued`: Fully buffered frames awaiting transmit cycle.
//!   - `Transmitting`: Draining frames onto the physical egress port.
//! * Strict per-hop latency bounds: $D_{hop} \in [T_{cycle}, 2 \cdot T_{cycle}]$.
//! * Ingress timestamp to cycle slot mapping with phase offset $\phi$.
//! * Buffer capacity accounting and congestion drop prevention.

/// CQF Queue Lifecycle Phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CqfQueueRole {
    /// Queue is actively receiving frames arriving in the current ingress cycle.
    Receiving,
    /// Queue is closed/gated and holding frames awaiting transmit cycle.
    Gated,
    /// Queue is open and transmitting frames out the egress interface.
    Transmitting,
}

/// Represents an enqueued CQF frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CqfFrame {
    pub stream_id: u32,
    pub priority: u8,
    pub payload_len: usize,
    pub ingress_timestamp_ns: u64,
    pub payload: Vec<u8>,
}

/// A single cyclic queue instance within the multi-queue shaper.
#[derive(Debug, Clone)]
pub struct CqfQueue {
    pub id: usize,
    pub role: CqfQueueRole,
    pub frames: Vec<CqfFrame>,
    pub max_capacity_bytes: usize,
    pub current_bytes: usize,
    pub total_enqueued: u64,
    pub total_transmitted: u64,
    pub total_dropped: u64,
}

impl CqfQueue {
    pub fn new(id: usize, max_capacity_bytes: usize, initial_role: CqfQueueRole) -> Self {
        CqfQueue {
            id,
            role: initial_role,
            frames: Vec::new(),
            max_capacity_bytes,
            current_bytes: 0,
            total_enqueued: 0,
            total_transmitted: 0,
            total_dropped: 0,
        }
    }

    /// Attempts to enqueue a frame if queue is receiving and under capacity.
    pub fn enqueue(&mut self, frame: CqfFrame) -> Result<(), &'static str> {
        if self.role != CqfQueueRole::Receiving {
            self.total_dropped += 1;
            return Err("Queue is not in Receiving phase");
        }
        if self.current_bytes + frame.payload_len > self.max_capacity_bytes {
            self.total_dropped += 1;
            return Err("CQF Queue buffer capacity overflow");
        }
        self.current_bytes += frame.payload_len;
        self.total_enqueued += 1;
        self.frames.push(frame);
        Ok(())
    }

    /// Drains all frames if queue is in transmitting phase.
    pub fn drain_transmitting(&mut self) -> Vec<CqfFrame> {
        if self.role != CqfQueueRole::Transmitting {
            return Vec::new();
        }
        let drained = std::mem::take(&mut self.frames);
        self.total_transmitted += drained.len() as u64;
        self.current_bytes = 0;
        drained
    }
}

/// IEEE 802.1Qch Multi-Cycle CQF Shaper Engine with 3 rotating queues.
#[derive(Debug, Clone)]
pub struct CqfMultiCycleEngine {
    /// Cycle time duration in nanoseconds ($T_{cycle}$, e.g. 125,000 ns = 125 µs).
    pub cycle_time_ns: u64,
    /// Phase offset $\phi$ in nanoseconds relative to global network epoch.
    pub phase_offset_ns: u64,
    /// 3 cyclic queues.
    pub queues: [CqfQueue; 3],
    /// Current cycle index $C = \lfloor \frac{t - \phi}{T_{cycle}} \rfloor$.
    pub current_cycle: u64,
    /// Statistics.
    pub frames_forwarded: u64,
    pub frames_dropped: u64,
}

impl CqfMultiCycleEngine {
    /// Creates a new 3-queue CQF engine.
    pub fn new(cycle_time_ns: u64, phase_offset_ns: u64, max_queue_capacity_bytes: usize) -> Self {
        let q0 = CqfQueue::new(0, max_queue_capacity_bytes, CqfQueueRole::Receiving);
        let q1 = CqfQueue::new(1, max_queue_capacity_bytes, CqfQueueRole::Transmitting);
        let q2 = CqfQueue::new(2, max_queue_capacity_bytes, CqfQueueRole::Gated);

        CqfMultiCycleEngine {
            cycle_time_ns,
            phase_offset_ns,
            queues: [q0, q1, q2],
            current_cycle: 0,
            frames_forwarded: 0,
            frames_dropped: 0,
        }
    }

    /// Computes which cycle index corresponds to the given timestamp.
    pub fn cycle_index(&self, timestamp_ns: u64) -> u64 {
        if timestamp_ns < self.phase_offset_ns {
            0
        } else {
            (timestamp_ns - self.phase_offset_ns) / self.cycle_time_ns
        }
    }

    /// Ingests an incoming frame based on current timestamp.
    pub fn ingest_frame(
        &mut self,
        stream_id: u32,
        priority: u8,
        payload: Vec<u8>,
        current_time_ns: u64,
    ) -> Result<(), &'static str> {
        self.advance_time(current_time_ns);

        // Active receiving queue is determined by (current_cycle % 3)
        let rx_idx = (self.current_cycle % 3) as usize;
        let frame = CqfFrame {
            stream_id,
            priority,
            payload_len: payload.len(),
            ingress_timestamp_ns: current_time_ns,
            payload,
        };

        match self.queues[rx_idx].enqueue(frame) {
            Ok(_) => Ok(()),
            Err(e) => {
                self.frames_dropped += 1;
                Err(e)
            }
        }
    }

    /// Advances the engine time, rotating queue roles when a cycle boundary is crossed.
    pub fn advance_time(&mut self, current_time_ns: u64) -> Vec<CqfFrame> {
        let target_cycle = self.cycle_index(current_time_ns);
        if target_cycle > self.current_cycle {
            let cycles_advanced = target_cycle - self.current_cycle;
            self.current_cycle = target_cycle;

            // Rotate queue roles for 3-queue CQF:
            // Receiving    = Q[C % 3]
            // Gated        = Q[(C + 2) % 3]  (holding frames from Cycle C - 1)
            // Transmitting = Q[(C + 1) % 3]  (draining frames from Cycle C - 2)
            let rx_q = (target_cycle % 3) as usize;
            let gt_q = ((target_cycle + 2) % 3) as usize;
            let tx_q = ((target_cycle + 1) % 3) as usize;

            self.queues[rx_q].role = CqfQueueRole::Receiving;
            self.queues[gt_q].role = CqfQueueRole::Gated;
            self.queues[tx_q].role = CqfQueueRole::Transmitting;

            // Drain transmitting queue if cycles advanced by 1 or more
            let mut transmitted = Vec::new();
            if cycles_advanced >= 1 {
                let drained = self.queues[tx_q].drain_transmitting();
                self.frames_forwarded += drained.len() as u64;
                transmitted.extend(drained);
            }
            transmitted
        } else {
            Vec::new()
        }
    }

    /// Explicitly triggers the transmission drain for the currently transmitting queue.
    pub fn transmit_current_cycle(&mut self) -> Vec<CqfFrame> {
        let tx_q = ((self.current_cycle + 1) % 3) as usize;
        let drained = self.queues[tx_q].drain_transmitting();
        self.frames_forwarded += drained.len() as u64;
        drained
    }

    /// Calculates theoretical latency bounds for this hop [min_ns, max_ns].
    pub fn hop_latency_bounds(&self) -> (u64, u64) {
        (self.cycle_time_ns, 2 * self.cycle_time_ns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cqf_cycle_index_calculation() {
        let engine = CqfMultiCycleEngine::new(125_000, 0, 65536); // 125µs cycle
        assert_eq!(engine.cycle_index(0), 0);
        assert_eq!(engine.cycle_index(124_999), 0);
        assert_eq!(engine.cycle_index(125_000), 1);
        assert_eq!(engine.cycle_index(250_000), 2);
    }

    #[test]
    fn test_cqf_3queue_rotation_and_drain() {
        let mut engine = CqfMultiCycleEngine::new(100_000, 0, 65536); // 100µs cycle

        // Cycle 0: Ingest frame into Receiving queue Q0
        let res = engine.ingest_frame(1, 7, vec![0x11; 64], 10_000);
        assert!(res.is_ok());
        assert_eq!(engine.queues[0].frames.len(), 1);

        // Advance to Cycle 1 (at 105,000 ns):
        // Q0 becomes Gated (holding Cycle 0 frames)
        // Q1 becomes Receiving
        // Q2 becomes Transmitting
        let tx1 = engine.advance_time(105_000);
        assert_eq!(tx1.len(), 0); // Q2 had no frames
        assert_eq!(engine.queues[0].role, CqfQueueRole::Gated);
        assert_eq!(engine.queues[1].role, CqfQueueRole::Receiving);
        assert_eq!(engine.queues[2].role, CqfQueueRole::Transmitting);

        // Ingest frame in Cycle 1 -> enters Q1
        engine.ingest_frame(1, 7, vec![0x22; 64], 150_000).unwrap();
        assert_eq!(engine.queues[1].frames.len(), 1);

        // Advance to Cycle 2 (at 205,000 ns):
        // Q0 becomes Transmitting -> drains Cycle 0 frame!
        let tx2 = engine.advance_time(205_000);
        assert_eq!(tx2.len(), 1);
        assert_eq!(tx2[0].payload[0], 0x11);
        assert_eq!(engine.frames_forwarded, 1);
    }

    #[test]
    fn test_cqf_capacity_overflow_drop() {
        let mut engine = CqfMultiCycleEngine::new(100_000, 0, 100); // 100B max capacity
        assert!(engine.ingest_frame(1, 7, vec![0xAA; 80], 5_000).is_ok());
        // Second frame overflows 100B limit
        assert!(engine.ingest_frame(1, 7, vec![0xBB; 40], 6_000).is_err());
        assert_eq!(engine.frames_dropped, 1);
    }

    #[test]
    fn test_cqf_hop_latency_bounds() {
        let engine = CqfMultiCycleEngine::new(125_000, 0, 65536);
        let (min_lat, max_lat) = engine.hop_latency_bounds();
        assert_eq!(min_lat, 125_000);
        assert_eq!(max_lat, 250_000);
    }
}
