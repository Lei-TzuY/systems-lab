use toy_tcpip::preemption::{PreemptionEngine, SmdType};

#[test]
fn test_preemption_smd_conversions() {
    assert_eq!(SmdType::from_u8(0xD5), Some(SmdType::SmdE));
    assert_eq!(SmdType::from_u8(0xE6), Some(SmdType::SmdS0));
    assert_eq!(SmdType::from_u8(0x52), Some(SmdType::SmdC1));
    assert_eq!(SmdType::from_u8(0xFF), None);
}

#[test]
fn test_preemption_interleaving_and_reassembly() {
    let mut engine = PreemptionEngine::new();
    let bulk_payload = vec![0x42; 256];
    let express_payload = vec![0x99; 32];

    let (frag0, express, frag1) = engine.interleave_express(&bulk_payload, &express_payload, 100);
    assert_eq!(frag0.payload.len(), 100);
    assert_eq!(express, express_payload);
    assert_eq!(frag1.payload.len(), 156);

    let reconstructed = PreemptionEngine::reassemble_fragments(&[frag0, frag1]).unwrap();
    assert_eq!(reconstructed, bulk_payload);
    assert_eq!(engine.express_frames_count, 1);
    assert_eq!(engine.preempted_frames_count, 1);
}
