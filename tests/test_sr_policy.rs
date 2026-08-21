use toy_tcpip::ipv6::Ipv6Address;
use toy_tcpip::sr_policy::{
    SrCandidatePath, SrPolicy, SrPolicyDatabase, SrProtocolOrigin, SrSegmentList,
    BGP_EXT_COMMUNITY_COLOR, SR_POLICY_TUNNEL_TYPE,
};

#[test]
fn test_sr_policy_traffic_steering_and_preference() {
    let mut db = SrPolicyDatabase::new();
    let endpoint = Ipv6Address::new([0x2001, 0x0db8, 0, 0, 0, 0, 0, 0x0001]);

    let mut policy = SrPolicy::new(200, endpoint, "SR-Policy-BulkData");

    policy.add_candidate_path(SrCandidatePath {
        preference: 50,
        protocol_origin: SrProtocolOrigin::Cli,
        segment_lists: vec![SrSegmentList {
            weight: 10,
            segments: vec![Ipv6Address::new([0xfc00, 0, 0, 1, 0, 0, 0, 1])],
        }],
    });

    policy.add_candidate_path(SrCandidatePath {
        preference: 150,
        protocol_origin: SrProtocolOrigin::Pcep,
        segment_lists: vec![SrSegmentList {
            weight: 10,
            segments: vec![
                Ipv6Address::new([0xfc00, 0, 0, 2, 0, 0, 0, 1]),
                Ipv6Address::new([0xfc00, 0, 0, 4, 0, 0, 0, 1]),
            ],
        }],
    });

    db.insert_policy(policy);

    let steered = db.steer_traffic(200, endpoint).unwrap();
    assert_eq!(steered.segments.len(), 2);
    assert_eq!(steered.segments[0], Ipv6Address::new([0xfc00, 0, 0, 2, 0, 0, 0, 1]));
}

#[test]
fn test_sr_policy_constants() {
    assert_eq!(BGP_EXT_COMMUNITY_COLOR, 0x030B);
    assert_eq!(SR_POLICY_TUNNEL_TYPE, 15);
}
