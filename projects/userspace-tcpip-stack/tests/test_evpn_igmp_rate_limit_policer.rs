use toy_tcpip::evpn_igmp_rate_limit_policer::{
    EvpnIgmpRateLimitPolicerEngine, IgmpMessageType, IgmpPolicerVerdict,
};

#[test]
fn test_evpn_igmp_rate_limit_policer_integration() {
    let mut policer = EvpnIgmpRateLimitPolicerEngine::new(5, 2);

    // Custom configuration on VNI 200, Port 10: 10 pps, burst 2, penalty threshold 3, quarantine 2s (2_000_000 us)
    policer.set_port_policy(200, 10, 10, 2, 3, 2_000_000);

    // 1. Initial 2 conforming messages
    let v1 = policer.police_message(200, 10, IgmpMessageType::V3MembershipReport, 0);
    assert_eq!(
        v1,
        IgmpPolicerVerdict::Conforming {
            vni: 200,
            port_id: 10,
            msg_type: IgmpMessageType::V3MembershipReport,
            remaining_tokens: 1,
        }
    );

    let v2 = policer.police_message(200, 10, IgmpMessageType::V3MembershipReport, 0);
    assert_eq!(
        v2,
        IgmpPolicerVerdict::Conforming {
            vni: 200,
            port_id: 10,
            msg_type: IgmpMessageType::V3MembershipReport,
            remaining_tokens: 0,
        }
    );

    // 2. 3 rapid drops triggering penalty box quarantine
    let d1 = policer.police_message(200, 10, IgmpMessageType::V3MembershipReport, 0);
    assert_eq!(
        d1,
        IgmpPolicerVerdict::RateLimitedDropped {
            vni: 200,
            port_id: 10,
            msg_type: IgmpMessageType::V3MembershipReport,
            drop_count: 1,
        }
    );

    let d2 = policer.police_message(200, 10, IgmpMessageType::V3MembershipReport, 0);
    assert_eq!(
        d2,
        IgmpPolicerVerdict::RateLimitedDropped {
            vni: 200,
            port_id: 10,
            msg_type: IgmpMessageType::V3MembershipReport,
            drop_count: 2,
        }
    );

    let d3 = policer.police_message(200, 10, IgmpMessageType::V3MembershipReport, 0);
    assert_eq!(
        d3,
        IgmpPolicerVerdict::QuarantinedInPenaltyBox {
            vni: 200,
            port_id: 10,
            msg_type: IgmpMessageType::V3MembershipReport,
            remaining_quarantine_us: 2_000_000,
        }
    );

    // 3. Early release of penalty box
    assert!(policer.release_penalty_box(200, 10));

    let v_after_release = policer.police_message(200, 10, IgmpMessageType::V3MembershipReport, 100);
    assert_eq!(
        v_after_release,
        IgmpPolicerVerdict::Conforming {
            vni: 200,
            port_id: 10,
            msg_type: IgmpMessageType::V3MembershipReport,
            remaining_tokens: 1,
        }
    );

    assert_eq!(policer.total_messages_evaluated, 6);
    assert_eq!(policer.total_conforming_messages, 3);
    assert_eq!(policer.total_rate_limited_drops, 3);
    assert_eq!(policer.total_penalty_box_triggers, 1);
}
