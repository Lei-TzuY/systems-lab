//! SRv6 Micro-SID (uSID) & SRH Compression (IETF SRv6 uSID Architecture).
//!
//! Implements compressed Segment Routing over IPv6 (uSID) where multiple 16-bit
//! or 32-bit Micro-SIDs are packed into a single 128-bit IPv6 Destination Address,
//! providing ultra-efficient Shift-and-Forward routing without requiring large SRH headers.

use crate::ipv6::Ipv6Address;

pub const USID_CARRIER_BLOCK_LEN_BITS: usize = 32; // 32-bit block prefix
pub const USID_LEN_BITS: usize = 16;               // 16-bit uSID per hop
pub const USID_MAX_HOPS: usize = 6;                // (128 - 32) / 16 = 6 micro-SIDs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsidBehavior {
    EndUN,   // Micro-SID Node (Shift-and-Forward)
    EndUA,   // Micro-SID Adjacency
    EndUDT4, // Micro-SID Decap to IPv4 VRF
    EndUDT6, // Micro-SID Decap to IPv6 VRF
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsidCarrier {
    pub block_prefix: u32,       // e.g. 0xFC000001 (fc00:1::/32)
    pub micro_sids: Vec<u16>,     // list of 16-bit micro-SIDs
}

impl UsidCarrier {
    pub fn new(block_prefix: u32, micro_sids: Vec<u16>) -> Self {
        UsidCarrier {
            block_prefix,
            micro_sids,
        }
    }

    /// Packs the 32-bit block prefix and up to 6 16-bit uSIDs into a 128-bit IPv6 address
    pub fn to_ipv6(&self) -> Ipv6Address {
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&self.block_prefix.to_be_bytes());

        for (i, &sid) in self.micro_sids.iter().take(USID_MAX_HOPS).enumerate() {
            let offset = 4 + (i * 2);
            bytes[offset..offset + 2].copy_from_slice(&sid.to_be_bytes());
        }

        Ipv6Address(bytes)
    }

    /// Parses an IPv6 destination address into a 32-bit block prefix and sequence of 16-bit uSIDs
    pub fn from_ipv6(addr: &Ipv6Address) -> Self {
        let block_prefix = u32::from_be_bytes([addr.0[0], addr.0[1], addr.0[2], addr.0[3]]);
        let mut micro_sids = Vec::new();

        for i in 0..USID_MAX_HOPS {
            let offset = 4 + (i * 2);
            let sid = u16::from_be_bytes([addr.0[offset], addr.0[offset + 1]]);
            if sid == 0 {
                break;
            }
            micro_sids.push(sid);
        }

        UsidCarrier {
            block_prefix,
            micro_sids,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct UsidForwardingEngine {
    pub node_usids: std::collections::HashMap<u16, UsidBehavior>, // My local uSID table
}

impl UsidForwardingEngine {
    pub fn new() -> Self {
        UsidForwardingEngine {
            node_usids: std::collections::HashMap::new(),
        }
    }

    pub fn register_usid(&mut self, usid: u16, behavior: UsidBehavior) {
        self.node_usids.insert(usid, behavior);
    }

    /// Performs the uSID Shift-and-Forward processing on an ingress IPv6 Destination Address
    pub fn process_destination_address(&self, da: &Ipv6Address) -> Option<(Ipv6Address, UsidBehavior)> {
        let mut carrier = UsidCarrier::from_ipv6(da);
        if carrier.micro_sids.is_empty() {
            return None;
        }

        let active_usid = carrier.micro_sids.remove(0); // Pop active uSID
        let behavior = *self.node_usids.get(&active_usid)?;

        match behavior {
            UsidBehavior::EndUN | UsidBehavior::EndUA => {
                // Shift-and-Forward: Construct new IPv6 DA with remaining uSIDs shifted left
                let next_da = carrier.to_ipv6();
                Some((next_da, behavior))
            }
            UsidBehavior::EndUDT4 | UsidBehavior::EndUDT6 => {
                // Termination behavior: Decapsulate outer IPv6 header
                Some((*da, behavior))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usid_carrier_packing_and_unpacking() {
        let carrier = UsidCarrier::new(0xFC000001, vec![0x1001, 0x2002, 0x3003, 0xE001]);
        let ipv6_addr = carrier.to_ipv6();

        let parsed = UsidCarrier::from_ipv6(&ipv6_addr);
        assert_eq!(parsed.block_prefix, 0xFC000001);
        assert_eq!(parsed.micro_sids, vec![0x1001, 0x2002, 0x3003, 0xE001]);
    }

    #[test]
    fn test_usid_shift_and_forward_pipeline() {
        let mut engine = UsidForwardingEngine::new();
        engine.register_usid(0x1001, UsidBehavior::EndUN);
        engine.register_usid(0x2002, UsidBehavior::EndUN);
        engine.register_usid(0xE001, UsidBehavior::EndUDT4);

        // Ingress packet with packed uSIDs: [0x1001, 0x2002, 0xE001]
        let ingress_carrier = UsidCarrier::new(0xFC000001, vec![0x1001, 0x2002, 0xE001]);
        let ingress_da = ingress_carrier.to_ipv6();

        // Node 1 (0x1001) processes DA
        let (hop1_da, behavior1) = engine.process_destination_address(&ingress_da).unwrap();
        assert_eq!(behavior1, UsidBehavior::EndUN);
        let hop1_carrier = UsidCarrier::from_ipv6(&hop1_da);
        assert_eq!(hop1_carrier.micro_sids, vec![0x2002, 0xE001]);

        // Node 2 (0x2002) processes DA
        let (hop2_da, behavior2) = engine.process_destination_address(&hop1_da).unwrap();
        assert_eq!(behavior2, UsidBehavior::EndUN);
        let hop2_carrier = UsidCarrier::from_ipv6(&hop2_da);
        assert_eq!(hop2_carrier.micro_sids, vec![0xE001]);

        // Node 3 (0xE001) processes End.uDT4 Decap
        let (_final_da, behavior3) = engine.process_destination_address(&hop2_da).unwrap();
        assert_eq!(behavior3, UsidBehavior::EndUDT4);
    }
}
