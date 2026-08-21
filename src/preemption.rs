//! IEEE 802.1Qbu / IEEE 802.3br Frame Preemption & Interspersed Express Traffic (FPE / IET - TSN).
//!
//! Allows ultra-low-latency Express frames (eMAC) to interrupt and preempt Best-Effort
//! or bulk frames (pMAC) mid-transmission with mPacket fragmentation and reassembly.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmdType {
    SmdE = 0xD5,       // Standard SFD / Express Frame
    SmdS0 = 0xE6,      // Start of Preempted Fragment 0
    SmdS1 = 0x4C,      // Start of Preempted Fragment 1
    SmdS2 = 0x7F,      // Start of Preempted Fragment 2
    SmdS3 = 0xB3,      // Start of Preempted Fragment 3
    SmdC0 = 0x61,      // Continuation Fragment 0
    SmdC1 = 0x52,      // Continuation Fragment 1
    SmdC2 = 0x9E,      // Continuation Fragment 2
    SmdC3 = 0x2A,      // Continuation Fragment 3
    SmdVerify = 0x07,  // Preemption link verification
    SmdRespond = 0x19, // Preemption link response
}

impl SmdType {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0xD5 => Some(SmdType::SmdE),
            0xE6 => Some(SmdType::SmdS0),
            0x4C => Some(SmdType::SmdS1),
            0x7F => Some(SmdType::SmdS2),
            0xB3 => Some(SmdType::SmdS3),
            0x61 => Some(SmdType::SmdC0),
            0x52 => Some(SmdType::SmdC1),
            0x9E => Some(SmdType::SmdC2),
            0x2A => Some(SmdType::SmdC3),
            0x07 => Some(SmdType::SmdVerify),
            0x19 => Some(SmdType::SmdRespond),
            _ => None,
        }
    }
}

/// mPacket Frame Fragment
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MPacketFragment {
    pub smd: SmdType,
    pub frag_num: u8, // Monotonic fragment counter (0..3)
    pub payload: Vec<u8>,
    pub is_last: bool,
}

/// IEEE 802.1Qbu / 802.3br Frame Preemption Engine
#[derive(Debug, Clone, Default)]
pub struct PreemptionEngine {
    pub express_frames_count: u64,
    pub preempted_frames_count: u64,
    pub total_fragments_count: u64,
}

impl PreemptionEngine {
    pub fn new() -> Self {
        PreemptionEngine {
            express_frames_count: 0,
            preempted_frames_count: 0,
            total_fragments_count: 0,
        }
    }

    /// Splits a preemptible bulk frame into mPacket fragments with SMD markers
    pub fn fragment_frame(&mut self, payload: &[u8], chunk_size: usize) -> Vec<MPacketFragment> {
        let mut fragments = Vec::new();
        let chunks: Vec<&[u8]> = payload.chunks(chunk_size.max(64)).collect();
        let total_chunks = chunks.len();

        for (idx, chunk) in chunks.into_iter().enumerate() {
            let smd = if idx == 0 {
                match idx % 4 {
                    0 => SmdType::SmdS0,
                    1 => SmdType::SmdS1,
                    2 => SmdType::SmdS2,
                    _ => SmdType::SmdS3,
                }
            } else {
                match idx % 4 {
                    0 => SmdType::SmdC0,
                    1 => SmdType::SmdC1,
                    2 => SmdType::SmdC2,
                    _ => SmdType::SmdC3,
                }
            };

            fragments.push(MPacketFragment {
                smd,
                frag_num: (idx % 4) as u8,
                payload: chunk.to_vec(),
                is_last: idx == total_chunks - 1,
            });
            self.total_fragments_count += 1;
        }

        self.preempted_frames_count += 1;
        fragments
    }

    /// Reassembles mPacket fragments back into the original complete frame
    pub fn reassemble_fragments(fragments: &[MPacketFragment]) -> Result<Vec<u8>, &'static str> {
        if fragments.is_empty() {
            return Err("Empty fragments list");
        }

        let mut output = Vec::new();
        for (idx, frag) in fragments.iter().enumerate() {
            if idx == 0 {
                match frag.smd {
                    SmdType::SmdS0 | SmdType::SmdS1 | SmdType::SmdS2 | SmdType::SmdS3 => {}
                    _ => return Err("First fragment must have SmdS start marker"),
                }
            } else {
                match frag.smd {
                    SmdType::SmdC0 | SmdType::SmdC1 | SmdType::SmdC2 | SmdType::SmdC3 => {}
                    _ => return Err("Subsequent fragments must have SmdC continuation marker"),
                }
            }
            output.extend_from_slice(&frag.payload);
        }

        Ok(output)
    }

    /// Simulates preemption: Interleaves an express frame right in between two bulk fragments
    pub fn interleave_express(
        &mut self,
        preemptible_data: &[u8],
        express_data: &[u8],
        split_offset: usize,
    ) -> (MPacketFragment, Vec<u8>, MPacketFragment) {
        let (first_half, second_half) =
            preemptible_data.split_at(split_offset.min(preemptible_data.len()));

        let frag_start = MPacketFragment {
            smd: SmdType::SmdS0,
            frag_num: 0,
            payload: first_half.to_vec(),
            is_last: false,
        };

        let express_pkt = express_data.to_vec();
        self.express_frames_count += 1;

        let frag_cont = MPacketFragment {
            smd: SmdType::SmdC1,
            frag_num: 1,
            payload: second_half.to_vec(),
            is_last: true,
        };

        self.preempted_frames_count += 1;
        self.total_fragments_count += 2;

        (frag_start, express_pkt, frag_cont)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preemption_fragmentation_and_reassembly() {
        let mut engine = PreemptionEngine::new();
        let bulk_data = vec![0xAB; 200]; // 200 bytes bulk payload

        let frags = engine.fragment_frame(&bulk_data, 64);
        assert_eq!(frags.len(), 4); // 64 + 64 + 64 + 8
        assert_eq!(frags[0].smd, SmdType::SmdS0);
        assert_eq!(frags[1].smd, SmdType::SmdC1);
        assert_eq!(frags[2].smd, SmdType::SmdC2);
        assert_eq!(frags[3].smd, SmdType::SmdC3);
        assert!(frags[3].is_last);

        let reassembled = PreemptionEngine::reassemble_fragments(&frags).unwrap();
        assert_eq!(reassembled, bulk_data);
    }

    #[test]
    fn test_express_frame_interleaving() {
        let mut engine = PreemptionEngine::new();
        let bulk_data = b"Preemptible Bulk Background File Transfer".to_vec();
        let express_data = b"EMERGENCY ROBOT BRAKE COMMAND".to_vec();

        let (frag0, express, frag1) = engine.interleave_express(&bulk_data, &express_data, 15);
        assert_eq!(frag0.payload.len(), 15);
        assert_eq!(express, express_data);
        assert_eq!(frag1.payload.len(), bulk_data.len() - 15);

        let reassembled = PreemptionEngine::reassemble_fragments(&[frag0, frag1]).unwrap();
        assert_eq!(reassembled, bulk_data);
    }
}
