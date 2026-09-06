//! TCP Sequence Number Arithmetic (RFC 1982 - Serial Number Arithmetic).
//!
//! Provides wraparound-safe sequence comparisons and arithmetic across modulo 2^32.

/// Returns true if sequence number `s1` is strictly before `s2` (s1 < s2).
#[inline]
pub fn seq_lt(s1: u32, s2: u32) -> bool {
    ((s1.wrapping_sub(s2)) as i32) < 0
}

/// Returns true if sequence number `s1` is before or equal to `s2` (s1 <= s2).
#[inline]
pub fn seq_le(s1: u32, s2: u32) -> bool {
    ((s1.wrapping_sub(s2)) as i32) <= 0
}

/// Returns true if sequence number `s1` is strictly after `s2` (s1 > s2).
#[inline]
pub fn seq_gt(s1: u32, s2: u32) -> bool {
    ((s1.wrapping_sub(s2)) as i32) > 0
}

/// Returns true if sequence number `s1` is after or equal to `s2` (s1 >= s2).
#[inline]
pub fn seq_ge(s1: u32, s2: u32) -> bool {
    ((s1.wrapping_sub(s2)) as i32) >= 0
}

/// Returns the forward distance from `s1` to `s2` in sequence space modulo 2^32.
#[inline]
pub fn seq_diff(s2: u32, s1: u32) -> u32 {
    s2.wrapping_sub(s1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequence_number_comparisons() {
        assert!(seq_lt(100, 200));
        assert!(seq_le(100, 100));
        assert!(seq_gt(200, 100));
        assert!(seq_ge(100, 100));

        // Wraparound boundary tests around 0xFFFF_FFFF
        let high = 0xFFFF_FFFE;
        let low = 0x0000_0005;

        assert!(seq_lt(high, low));
        assert!(seq_gt(low, high));
        assert!(!seq_gt(high, low));
        assert!(!seq_lt(low, high));

        assert_eq!(seq_diff(low, high), 7);
    }
}
