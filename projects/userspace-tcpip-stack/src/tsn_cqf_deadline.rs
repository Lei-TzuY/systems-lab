//! IEEE 802.1Qch CQF Cyclic Buffer Overrun & Deadline Expiry Protection Engine.
//!
//! In Time-Sensitive Networking (TSN) Cyclic Queuing and Forwarding (CQF / 802.1Qch),
//! frames must be enqueued within a specific time window before the cycle switches to
//! prevent gate-closure truncations or queue buffer overruns.
//!
//! This module implements:
//! * Per-cycle queue capacity limits (max bytes and max frame count).
//! * Ingress deadline policing:
//!   - Frames arriving within the safe admission window $[0, T_{\text{deadline}}]$ are queued for cycle $i$.
//!   - Frames arriving past the deadline window $[T_{\text{deadline}}, T_{\text{cycle}}]$ are either deferred
//!     to cycle $i+1$ or dropped if their hard application deadline would be violated.
//! * Bounded queue memory protection against burst oversubscription.

/// Result of a CQF deadline and buffer check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CqfAdmissionResult {
    AdmittedCurrentCycle { cycle_id: u64 },
    DeferredToNextCycle { cycle_id: u64 },
    DroppedBufferOverrun,
    DroppedDeadlineMiss,
}

/// A scheduled frame with deadline requirements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CqfScheduledFrame {
    pub stream_id: u32,
    pub payload_bytes: usize,
    pub arrival_time_ns: u64,
    pub max_allowable_delay_ns: u64,
}

/// TSN CQF Deadline and Buffer Protection Engine.
#[derive(Debug, Clone)]
pub struct TsnCqfDeadlineEngine {
    pub cycle_time_ns: u64,
    pub ingress_deadline_offset_ns: u64,
    pub max_bytes_per_cycle: usize,
    pub max_frames_per_cycle: usize,
    pub current_cycle_bytes: usize,
    pub current_cycle_frames: usize,
    pub next_cycle_bytes: usize,
    pub next_cycle_frames: usize,
    pub total_admitted: u64,
    pub total_deferred: u64,
    pub total_overrun_drops: u64,
    pub total_deadline_drops: u64,
}

impl TsnCqfDeadlineEngine {
    pub fn new(
        cycle_time_ns: u64,
        ingress_deadline_offset_ns: u64,
        max_bytes_per_cycle: usize,
        max_frames_per_cycle: usize,
    ) -> Self {
        TsnCqfDeadlineEngine {
            cycle_time_ns,
            ingress_deadline_offset_ns,
            max_bytes_per_cycle,
            max_frames_per_cycle,
            current_cycle_bytes: 0,
            current_cycle_frames: 0,
            next_cycle_bytes: 0,
            next_cycle_frames: 0,
            total_admitted: 0,
            total_deferred: 0,
            total_overrun_drops: 0,
            total_deadline_drops: 0,
        }
    }

    /// Evaluates frame admission into cyclic queues based on arrival timestamp and capacity.
    pub fn ingest_frame(&mut self, frame: &CqfScheduledFrame) -> CqfAdmissionResult {
        let current_cycle_id = frame.arrival_time_ns / self.cycle_time_ns;
        let time_in_cycle = frame.arrival_time_ns % self.cycle_time_ns;

        if time_in_cycle <= self.ingress_deadline_offset_ns {
            // Frame arrived in time for current cycle ingestion
            if self.current_cycle_bytes + frame.payload_bytes > self.max_bytes_per_cycle
                || self.current_cycle_frames + 1 > self.max_frames_per_cycle
            {
                self.total_overrun_drops += 1;
                CqfAdmissionResult::DroppedBufferOverrun
            } else {
                self.current_cycle_bytes += frame.payload_bytes;
                self.current_cycle_frames += 1;
                self.total_admitted += 1;
                CqfAdmissionResult::AdmittedCurrentCycle {
                    cycle_id: current_cycle_id,
                }
            }
        } else {
            // Past safe ingress deadline for current cycle -> Check if can defer to next cycle
            let deferred_latency = self.cycle_time_ns * 2 - time_in_cycle;
            if deferred_latency > frame.max_allowable_delay_ns {
                self.total_deadline_drops += 1;
                CqfAdmissionResult::DroppedDeadlineMiss
            } else if self.next_cycle_bytes + frame.payload_bytes > self.max_bytes_per_cycle
                || self.next_cycle_frames + 1 > self.max_frames_per_cycle
            {
                self.total_overrun_drops += 1;
                CqfAdmissionResult::DroppedBufferOverrun
            } else {
                self.next_cycle_bytes += frame.payload_bytes;
                self.next_cycle_frames += 1;
                self.total_deferred += 1;
                CqfAdmissionResult::DeferredToNextCycle {
                    cycle_id: current_cycle_id + 1,
                }
            }
        }
    }

    /// Advances the cycle clock, swapping next cycle buffer counters into current cycle.
    pub fn advance_cycle(&mut self) {
        self.current_cycle_bytes = self.next_cycle_bytes;
        self.current_cycle_frames = self.next_cycle_frames;
        self.next_cycle_bytes = 0;
        self.next_cycle_frames = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cqf_deadline_and_overrun() {
        let mut engine = TsnCqfDeadlineEngine::new(10_000, 7_000, 2000, 3);

        // 1. In-time admission
        let f1 = CqfScheduledFrame {
            stream_id: 1,
            payload_bytes: 800,
            arrival_time_ns: 3_000, // 3us <= 7us
            max_allowable_delay_ns: 30_000,
        };
        assert_eq!(
            engine.ingest_frame(&f1),
            CqfAdmissionResult::AdmittedCurrentCycle { cycle_id: 0 }
        );

        // 2. Late arrival within allowable latency -> Deferred to cycle 1
        let f2 = CqfScheduledFrame {
            stream_id: 2,
            payload_bytes: 600,
            arrival_time_ns: 8_500, // 8.5us > 7us
            max_allowable_delay_ns: 20_000,
        };
        assert_eq!(
            engine.ingest_frame(&f2),
            CqfAdmissionResult::DeferredToNextCycle { cycle_id: 1 }
        );

        // 3. Late arrival but strict latency constraint -> Deadline miss drop
        let f3 = CqfScheduledFrame {
            stream_id: 3,
            payload_bytes: 500,
            arrival_time_ns: 9_000,
            max_allowable_delay_ns: 5_000, // Cannot tolerate deferral
        };
        assert_eq!(
            engine.ingest_frame(&f3),
            CqfAdmissionResult::DroppedDeadlineMiss
        );
    }
}
