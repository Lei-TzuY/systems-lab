//! IEEE 802.1Qch Cyclic Queuing and Forwarding (CQF / TSN - Time-Sensitive Networking).
//!
//! Provides deterministic zero-jitter packet transmission and bounded latency
//! using synchronized alternating double-buffer ping-pong queuing cycles.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CqfPacket {
    pub id: u32,
    pub priority: u8,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct CqfBuffer {
    pub packets: Vec<CqfPacket>,
}

/// CQF Ping-Pong Dual Buffer Queue
#[derive(Debug, Clone)]
pub struct CqfEngine {
    pub cycle_duration_us: u32,
    pub current_cycle_index: u64,
    pub buffer_even: CqfBuffer,
    pub buffer_odd: CqfBuffer,
    pub transmitted_packets_count: u32,
}

impl Default for CqfEngine {
    fn default() -> Self {
        Self::new(125) // Default 125µs TSN cycle
    }
}

impl CqfEngine {
    pub fn new(cycle_duration_us: u32) -> Self {
        CqfEngine {
            cycle_duration_us,
            current_cycle_index: 0,
            buffer_even: CqfBuffer::default(),
            buffer_odd: CqfBuffer::default(),
            transmitted_packets_count: 0,
        }
    }

    /// Ingress: Enqueues an incoming frame into the active receiving cycle buffer
    pub fn enqueue(&mut self, id: u32, priority: u8, payload: Vec<u8>) {
        let pkt = CqfPacket { id, priority, payload };
        if self.current_cycle_index % 2 == 0 {
            // In even cycle: Receive into buffer_even
            self.buffer_even.packets.push(pkt);
        } else {
            // In odd cycle: Receive into buffer_odd
            self.buffer_odd.packets.push(pkt);
        }
    }

    /// Cycle Tick: Advances to the next cycle and transmits all frames collected in the previous cycle
    pub fn advance_cycle(&mut self) -> Vec<CqfPacket> {
        self.current_cycle_index = self.current_cycle_index.wrapping_add(1);

        // If new cycle is odd, we transmit frames accumulated in buffer_even during the previous even cycle
        let tx_packets = if self.current_cycle_index % 2 != 0 {
            std::mem::take(&mut self.buffer_even.packets)
        } else {
            // If new cycle is even, we transmit frames accumulated in buffer_odd during the previous odd cycle
            std::mem::take(&mut self.buffer_odd.packets)
        };

        self.transmitted_packets_count += tx_packets.len() as u32;
        tx_packets
    }

    /// Calculates deterministic min/max latency bounds in microseconds
    pub fn latency_bounds_us(&self) -> (u32, u32) {
        (self.cycle_duration_us, 2 * self.cycle_duration_us)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cqf_ping_pong_cycle_switching() {
        let mut cqf = CqfEngine::new(250); // 250µs cycle
        assert_eq!(cqf.latency_bounds_us(), (250, 500));

        // Enqueue during Cycle 0 (Even)
        cqf.enqueue(101, 7, b"Mission Critical Packet 1".to_vec());
        cqf.enqueue(102, 7, b"Mission Critical Packet 2".to_vec());

        // Advance to Cycle 1 (Odd) -> Drains Even buffer
        let tx_cycle_1 = cqf.advance_cycle();
        assert_eq!(tx_cycle_1.len(), 2);
        assert_eq!(tx_cycle_1[0].id, 101);
        assert_eq!(tx_cycle_1[1].id, 102);

        // Enqueue during Cycle 1 (Odd)
        cqf.enqueue(201, 6, b"Control Loop Packet 3".to_vec());

        // Advance to Cycle 2 (Even) -> Drains Odd buffer
        let tx_cycle_2 = cqf.advance_cycle();
        assert_eq!(tx_cycle_2.len(), 1);
        assert_eq!(tx_cycle_2[0].id, 201);
    }
}
