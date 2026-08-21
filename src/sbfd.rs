//! Seamless Bidirectional Forwarding Detection (S-BFD - RFC 7880 / RFC 7881 / RFC 7884).
//!
//! Provides stateless reflector and initiator architecture for sub-millisecond
//! Segment Routing and MPLS Traffic Engineering path verification over UDP port 7784.

use std::collections::HashSet;

pub const SBFD_REFLECTOR_PORT: u16 = 7784;
pub const SBFD_HEADER_LEN: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SbfdState {
    AdminDown = 0,
    Down = 1,
    Init = 2,
    Up = 3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbfdPacket {
    pub version: u8, // 1
    pub diag: u8,
    pub state: SbfdState,
    pub poll: bool,
    pub final_bit: bool,
    pub c_bit: bool,
    pub a_bit: bool,
    pub detect_mult: u8,
    pub length: u8,
    pub my_discriminator: u32,
    pub your_discriminator: u32,
    pub desired_min_tx_us: u32,
    pub required_min_rx_us: u32,
    pub required_min_echo_rx_us: u32,
}

impl SbfdPacket {
    pub fn build_initiator_probe(
        my_disc: u32,
        target_reflector_disc: u32,
        desired_tx_us: u32,
    ) -> Self {
        SbfdPacket {
            version: 1,
            diag: 0,
            state: SbfdState::Up,
            poll: true,
            final_bit: false,
            c_bit: false,
            a_bit: false,
            detect_mult: 3,
            length: SBFD_HEADER_LEN as u8,
            my_discriminator: my_disc,
            your_discriminator: target_reflector_disc,
            desired_min_tx_us: desired_tx_us,
            required_min_rx_us: 0,
            required_min_echo_rx_us: 0,
        }
    }

    pub fn serialize(&self) -> [u8; SBFD_HEADER_LEN] {
        let mut buf = [0u8; SBFD_HEADER_LEN];

        let state_val = match self.state {
            SbfdState::AdminDown => 0,
            SbfdState::Down => 1,
            SbfdState::Init => 2,
            SbfdState::Up => 3,
        };

        buf[0] = ((self.version & 0x07) << 5) | (self.diag & 0x1F);

        let mut flags = (state_val & 0x03) << 6;
        if self.poll { flags |= 0x20; }
        if self.final_bit { flags |= 0x10; }
        if self.c_bit { flags |= 0x08; }
        if self.a_bit { flags |= 0x04; }
        buf[1] = flags;

        buf[2] = self.detect_mult;
        buf[3] = self.length;

        buf[4..8].copy_from_slice(&self.my_discriminator.to_be_bytes());
        buf[8..12].copy_from_slice(&self.your_discriminator.to_be_bytes());
        buf[12..16].copy_from_slice(&self.desired_min_tx_us.to_be_bytes());
        buf[16..20].copy_from_slice(&self.required_min_rx_us.to_be_bytes());
        buf[20..24].copy_from_slice(&self.required_min_echo_rx_us.to_be_bytes());

        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < SBFD_HEADER_LEN {
            return None;
        }

        let version = (data[0] >> 5) & 0x07;
        if version != 1 {
            return None;
        }
        let diag = data[0] & 0x1F;

        let state = match (data[1] >> 6) & 0x03 {
            0 => SbfdState::AdminDown,
            1 => SbfdState::Down,
            2 => SbfdState::Init,
            _ => SbfdState::Up,
        };

        let poll = (data[1] & 0x20) != 0;
        let final_bit = (data[1] & 0x10) != 0;
        let c_bit = (data[1] & 0x08) != 0;
        let a_bit = (data[1] & 0x04) != 0;

        let detect_mult = data[2];
        let length = data[3];

        let my_discriminator = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let your_discriminator = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let desired_min_tx_us = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
        let required_min_rx_us = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let required_min_echo_rx_us = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);

        Some(SbfdPacket {
            version,
            diag,
            state,
            poll,
            final_bit,
            c_bit,
            a_bit,
            detect_mult,
            length,
            my_discriminator,
            your_discriminator,
            desired_min_tx_us,
            required_min_rx_us,
            required_min_echo_rx_us,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct SbfdReflector {
    pub local_discriminators: HashSet<u32>, // Allocated S-BFD Discriminators
    pub reflected_packets: u32,
}

impl SbfdReflector {
    pub fn new() -> Self {
        SbfdReflector {
            local_discriminators: HashSet::new(),
            reflected_packets: 0,
        }
    }

    pub fn register_discriminator(&mut self, disc: u32) {
        self.local_discriminators.insert(disc);
    }

    /// Processes an incoming S-BFD probe statelessly and generates reflection
    pub fn process_probe(&self, probe: &SbfdPacket) -> Option<SbfdPacket> {
        if !self.local_discriminators.contains(&probe.your_discriminator) {
            return None; // Mismatched target discriminator
        }

        Some(SbfdPacket {
            version: 1,
            diag: 0,
            state: SbfdState::Up,
            poll: false,
            final_bit: probe.poll, // Mirror Poll bit into Final bit
            c_bit: false,
            a_bit: false,
            detect_mult: probe.detect_mult,
            length: SBFD_HEADER_LEN as u8,
            my_discriminator: probe.your_discriminator,
            your_discriminator: probe.my_discriminator,
            desired_min_tx_us: 0,
            required_min_rx_us: 10_000,
            required_min_echo_rx_us: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sbfd_reflector_stateless_reply() {
        let mut reflector = SbfdReflector::new();
        reflector.register_discriminator(0x90001); // Target S-BFD Reflector Disc

        // Initiator sends probe
        let probe = SbfdPacket::build_initiator_probe(0x10001, 0x90001, 50_000);
        let raw_probe = probe.serialize();

        let parsed_probe = SbfdPacket::parse(&raw_probe).unwrap();
        let reply = reflector.process_probe(&parsed_probe).unwrap();

        assert_eq!(reply.state, SbfdState::Up);
        assert_eq!(reply.final_bit, true);
        assert_eq!(reply.my_discriminator, 0x90001);
        assert_eq!(reply.your_discriminator, 0x10001);
    }
}
