//! Quality of Service (QoS): Token Bucket Traffic Shaper and Priority Scheduler.
//!
//! Provides the Token Bucket algorithm (RFC 2697) for egress rate limiting / policing
//! and a Strict Priority Queue (SPQ) packet scheduler.

use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PacketPriority {
    Low = 0,
    Normal = 1,
    High = 2,
}

/// Token Bucket Algorithm for Traffic Shaping and Rate Limiting (RFC 2697).
#[derive(Debug, Clone)]
pub struct TokenBucket {
    pub capacity_bytes: f64,       // Maximum token capacity (Burst size)
    pub tokens: f64,               // Current token balance (in bytes)
    pub rate_bytes_per_sec: f64,   // Token replenishment rate (Bytes/sec)
    pub last_update_ms: u64,       // Timestamp of last update
}

impl TokenBucket {
    pub fn new(capacity_bytes: usize, rate_bytes_per_sec: usize) -> Self {
        TokenBucket {
            capacity_bytes: capacity_bytes as f64,
            tokens: capacity_bytes as f64, // Start full
            rate_bytes_per_sec: rate_bytes_per_sec as f64,
            last_update_ms: 0,
        }
    }

    /// Refills tokens proportional to elapsed time.
    pub fn refill(&mut self, now_ms: u64) {
        if now_ms > self.last_update_ms {
            let elapsed_sec = (now_ms - self.last_update_ms) as f64 / 1000.0;
            let added_tokens = elapsed_sec * self.rate_bytes_per_sec;
            self.tokens = (self.tokens + added_tokens).min(self.capacity_bytes);
            self.last_update_ms = now_ms;
        }
    }

    /// Tries to consume tokens for a packet of given size.
    /// Returns true if conformant (tokens available), or false if non-conformant (rate limited).
    pub fn try_consume(&mut self, packet_size_bytes: usize, now_ms: u64) -> bool {
        self.refill(now_ms);

        let required = packet_size_bytes as f64;
        if self.tokens >= required {
            self.tokens -= required;
            true
        } else {
            false
        }
    }
}

/// Strict Priority Queue (SPQ) Traffic Scheduler
#[derive(Debug, Default)]
pub struct PriorityScheduler {
    high_queue: VecDeque<Vec<u8>>,
    normal_queue: VecDeque<Vec<u8>>,
    low_queue: VecDeque<Vec<u8>>,
}

impl PriorityScheduler {
    pub fn new() -> Self {
        PriorityScheduler {
            high_queue: VecDeque::new(),
            normal_queue: VecDeque::new(),
            low_queue: VecDeque::new(),
        }
    }

    pub fn enqueue(&mut self, priority: PacketPriority, frame: Vec<u8>) {
        match priority {
            PacketPriority::High => self.high_queue.push_back(frame),
            PacketPriority::Normal => self.normal_queue.push_back(frame),
            PacketPriority::Low => self.low_queue.push_back(frame),
        }
    }

    /// Dequeues the next packet strictly honoring High > Normal > Low priority tiers.
    pub fn dequeue(&mut self) -> Option<Vec<u8>> {
        if let Some(pkt) = self.high_queue.pop_front() {
            Some(pkt)
        } else if let Some(pkt) = self.normal_queue.pop_front() {
            Some(pkt)
        } else {
            self.low_queue.pop_front()
        }
    }

    pub fn total_queued(&self) -> usize {
        self.high_queue.len() + self.normal_queue.len() + self.low_queue.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_burst_and_refill() {
        // 1000 bytes capacity, 1000 bytes/sec rate
        let mut bucket = TokenBucket::new(1000, 1000);

        // 1. Consume 800 bytes immediately (burst) -> Success
        assert!(bucket.try_consume(800, 0));
        assert_eq!(bucket.tokens as u32, 200);

        // 2. Consume another 300 bytes at time 0 -> Fails (only 200 remaining)
        assert!(!bucket.try_consume(300, 0));

        // 3. Advance clock by 500ms -> Refills 500 bytes (total 700) -> 300 bytes now succeeds!
        assert!(bucket.try_consume(300, 500));
        assert_eq!(bucket.tokens as u32, 400);
    }

    #[test]
    fn test_strict_priority_scheduling() {
        let mut scheduler = PriorityScheduler::new();

        scheduler.enqueue(PacketPriority::Low, vec![3]);
        scheduler.enqueue(PacketPriority::Normal, vec![2]);
        scheduler.enqueue(PacketPriority::High, vec![1]);

        // Strict priority should drain High -> Normal -> Low
        assert_eq!(scheduler.dequeue(), Some(vec![1]));
        assert_eq!(scheduler.dequeue(), Some(vec![2]));
        assert_eq!(scheduler.dequeue(), Some(vec![3]));
        assert_eq!(scheduler.dequeue(), None);
    }
}
