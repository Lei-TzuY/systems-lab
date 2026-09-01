use toy_tcpip::evpn_uu_egress_filter::{
    EgressPortConfig, EgressVerdict, Esi, EvpnUuEgressFilterEngine,
};

#[test]
fn test_evpn_uu_egress_filtering_lifecycle() {
    let mut engine = EvpnUuEgressFilterEngine::new();

    let mut esi_site_a: Esi = [0u8; 10];
    esi_site_a[0] = 0x01;
    esi_site_a[9] = 0xAA;

    let mut esi_site_b: Esi = [0u8; 10];
    esi_site_b[0] = 0x01;
    esi_site_b[9] = 0xBB;

    // Port 1: Horizon 10, ESI Site A, VNIs 100, 200
    engine.configure_port(EgressPortConfig {
        port_id: 1,
        horizon: 10,
        esi: esi_site_a,
        active_vnis: vec![100, 200],
    });

    // Port 2: Horizon 20, ESI Site B, VNIs 100, 300
    engine.configure_port(EgressPortConfig {
        port_id: 2,
        horizon: 20,
        esi: esi_site_b,
        active_vnis: vec![100, 300],
    });

    // Port 3: Horizon 0 (Core/Fabric), No ESI, VNIs 100, 200, 300
    engine.configure_port(EgressPortConfig {
        port_id: 3,
        horizon: 0,
        esi: [0u8; 10],
        active_vnis: vec![100, 200, 300],
    });

    assert_eq!(engine.port_count(), 3);

    // ── Test 1: Frame from Horizon 10 (Access Port 1), VNI 100, Source ESI Site A ──
    let results1 = engine.evaluate(10, &esi_site_a, 100);
    assert_eq!(results1.len(), 3);
    // Port 1: Pruned by Horizon
    assert_eq!(results1[0].port_id, 1);
    assert_eq!(results1[0].verdict, EgressVerdict::PrunedHorizon);
    // Port 2: Forward (Horizon 20 != 10, ESI Site B != Site A, VNI 100 present)
    assert_eq!(results1[1].port_id, 2);
    assert_eq!(results1[1].verdict, EgressVerdict::Forward);
    // Port 3: Forward (Horizon 0, No ESI, VNI 100 present)
    assert_eq!(results1[2].port_id, 3);
    assert_eq!(results1[2].verdict, EgressVerdict::Forward);

    // ── Test 2: Frame from Core (Horizon 0), VNI 200, Source ESI Site B ──
    let results2 = engine.evaluate(0, &esi_site_b, 200);
    // Port 1: Forward (Horizon 10 != 0, ESI Site A != Site B, VNI 200 present)
    assert_eq!(results2[0].verdict, EgressVerdict::Forward);
    // Port 2: Pruned by ESI (Source ESI matches Port 2's ESI)
    assert_eq!(results2[1].verdict, EgressVerdict::PrunedEsi);
    // Port 3: Forward (Horizon 0 == 0 -> allowed since 0 means no split-horizon group, VNI 200 present)
    assert_eq!(results2[2].verdict, EgressVerdict::Forward);

    // ── Test 3: Frame from Core (Horizon 0), VNI 300, No ESI ──
    let results3 = engine.evaluate(0, &[0u8; 10], 300);
    // Port 1: Pruned by VNI (Port 1 does not have VNI 300)
    assert_eq!(results3[0].verdict, EgressVerdict::PrunedVni);
    // Port 2: Forward (Port 2 has VNI 300)
    assert_eq!(results3[1].verdict, EgressVerdict::Forward);
    // Port 3: Forward
    assert_eq!(results3[2].verdict, EgressVerdict::Forward);

    // ── Verify Pruning Statistics ──
    let stats = engine.stats();
    assert_eq!(stats.total_frames_evaluated, 3);
    assert_eq!(stats.total_port_decisions, 9);
    assert_eq!(stats.pruned_horizon, 1);
    assert_eq!(stats.pruned_esi, 1);
    assert_eq!(stats.pruned_vni, 1);
    assert_eq!(stats.forwarded, 6);
}
