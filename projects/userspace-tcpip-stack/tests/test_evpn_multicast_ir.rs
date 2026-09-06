use toy_tcpip::evpn_multicast_ir::{EvpnSelectiveIrEngine, MulticastChannel};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_evpn_multicast_ir_selective_leaf_replication() {
    let mut engine = EvpnSelectiveIrEngine::new();

    // 4 Leaf VTEPs in VNI 500
    engine.set_inclusive_vteps(
        500,
        vec![
            Ipv4Address::new(172, 16, 0, 1),
            Ipv4Address::new(172, 16, 0, 2),
            Ipv4Address::new(172, 16, 0, 3),
            Ipv4Address::new(172, 16, 0, 4),
        ],
    );

    let src = Ipv4Address::new(10, 50, 1, 10);
    let grp = Ipv4Address::new(239, 10, 10, 10);
    let chan = MulticastChannel::new_ssm(500, src, grp);

    // Only Leaf 3 joins
    engine.add_smet_receiver(chan, Ipv4Address::new(172, 16, 0, 3));

    let (targets, is_sel) = engine.resolve_replication_targets(500, src, grp);
    assert!(is_sel);
    assert_eq!(targets, vec![Ipv4Address::new(172, 16, 0, 3)]);
    assert_eq!(engine.total_pruned_packets_saved, 3); // 4 - 1 = 3 pruned!

    // Leaf 3 leaves
    assert!(engine.remove_smet_receiver(&chan, &Ipv4Address::new(172, 16, 0, 3)));

    // After leave, falls back to IMET list
    let (targets_after, is_sel_after) = engine.resolve_replication_targets(500, src, grp);
    assert!(!is_sel_after);
    assert_eq!(targets_after.len(), 4);
}
