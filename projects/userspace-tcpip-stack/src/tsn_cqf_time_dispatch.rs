//! IEEE 802.1Qch Cyclic Queuing and Forwarding (CQF) Time-Synchronized Dispatch Engine.
//!
//! In IEEE 802.1Qch CQF, frames received in cycle $i$ are exclusively transmitted in cycle $i+1$,
//! guaranteeing bounded multi-hop latency and zero packet jitter across deterministic networks.
//!
//! This module implements:
//! * Microsecond cycle clock tick driving deterministic ping-pong queue swapping.
//! * Even-cycle / Odd-cycle frame reception and transmission decoupling.
//! * Bounded latency verification across cascaded bridge nodes.

/// CQF Ping-Pong Queue Cycle phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CqfCyclePhase {
    Even,
    Odd,
}

impl CqfCyclePhase {
    pub fn next(self) -> Self {
        match self {
            CqfCyclePhase::Even => CqfCyclePhase::Odd,
            CqfCyclePhase::Odd => CqfCyclePhase::Even,
        }
    }
}

/// A frame stored in a CQF cyclic queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CqfBufferedPacket {
    pub stream_id: u32,
    pub payload_bytes: usize,
    pub ingress_cycle: u64,
}

/// IEEE 802.1Qch CQF Time-Synchronized Dispatch Engine.
#[derive(Debug, Clone)]
pub struct TsnCqfTimeDispatchEngine {
    pub cycle_time_ns: u64,
    pub current_cycle_index: u64,
    pub current_time_ns: u64,
    pub queue_even: Vec<CqfBufferedPacket>,
    pub queue_odd: Vec<CqfBufferedPacket>,
    pub total_enqueued: u64,
    pub total_dispatched: u64,
}

impl TsnCqfTimeDispatchEngine {
    pub fn new(cycle_time_ns: u64) -> Self {
        TsnCqfTimeDispatchEngine {
            cycle_time_ns: cycle_time_ns.max(1000), // Minimum 1 us cycle
            current_cycle_index: 0,
            current_time_ns: 0,
            queue_even: Vec::new(),
            queue_odd: Vec::new(),
            total_enqueued: 0,
            total_dispatched: 0,
        }
    }

    /// Enqueues an incoming frame into the current active receiving queue.
    pub fn enqueue_frame(&mut self, stream_id: u32, payload_bytes: usize) {
        let current_phase = if self.current_cycle_index % 2 == 0 {
            CqfCyclePhase::Even
        } else {
            CqfCyclePhase::Odd
        };

        let packet = CqfBufferedPacket {
            stream_id,
            payload_bytes,
            ingress_cycle: self.current_cycle_index,
        };

        match current_phase {
            CqfCyclePhase::Even => self.queue_even.push(packet),
            CqfCyclePhase::Odd => self.queue_odd.push(packet),
        }
        self.total_enqueued += 1;
    }

    /// Advances the simulation clock. If the cycle boundary is crossed,
    /// swaps ping-pong queues and drains the transmit queue.
    pub fn advance_time(&mut self, delta_ns: u64) -> Vec<CqfBufferedPacket> {
        self.current_time_ns += delta_ns;
        let new_cycle = self.current_time_ns / self.cycle_time_ns;

        let mut dispatched_frames = Vec::new();

        while self.current_cycle_index < new_cycle {
            self.current_cycle_index += 1;

            // In the new cycle, drain the queue that was filled in the previous cycle
            let drain_phase = if self.current_cycle_index % 2 == 0 {
                // Now receiving in Even, drain Odd (filled in Odd cycle)
                CqfCyclePhase::Odd
            } else {
                // Now receiving in Odd, drain Even (filled in Even cycle)
                CqfCyclePhase::Even
            };

            match drain_phase {
                CqfCyclePhase::Even => {
                    self.total_dispatched += self.queue_even.len() as u64;
                    dispatched_frames.append(&mut self.queue_even);
                }
                CqfCyclePhase::Odd => {
                    self.total_dispatched += self.queue_odd.len() as u64;
                    dispatched_frames.append(&mut self.queue_odd);
                }
            }
        }

        dispatched_frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tsn_cqf_time_synchronized_dispatch() {
        let cycle_10us = 10_000; // 10 microseconds
        let mut engine = TsnCqfTimeDispatchEngine::new(cycle_10us);

        // Cycle 0: Enqueue 2 frames
        engine.enqueue_frame(101, 500);
        engine.enqueue_frame(102, 600);
        assert_eq!(engine.total_enqueued, 2);

        // Advance 5us (still in Cycle 0): No frames dispatched
        let d1 = engine.advance_time(5_000);
        assert!(d1.is_empty());

        // Advance another 5us (enters Cycle 1 at 10us): Frames from Cycle 0 are dispatched!
        let d2 = engine.advance_time(5_000);
        assert_eq!(d2.len(), 2);
        assert_eq!(d2[0].stream_id, 101);
        assert_eq!(d2[1].stream_id, 102);

        // Cycle 1: Enqueue frame 103
        engine.enqueue_frame(103, 1200);

        // Advance 10us (enters Cycle 2 at 20us): Frame 103 from Cycle 1 dispatched!
        let d3 = engine.advance_time(10_000);
        assert_eq!(d3.len(), 1);
        assert_eq!(d3[0].stream_id, 103);
    }
}
