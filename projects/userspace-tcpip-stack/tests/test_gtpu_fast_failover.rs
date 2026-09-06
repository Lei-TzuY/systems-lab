use toy_tcpip::gtpu_fast_failover::{ActivePath, FastFailoverSession, GtpuFastFailoverEngine};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_gtpu_submillisecond_fast_failover() {
    let mut engine = GtpuFastFailoverEngine::new();
    let session = FastFailoverSession::new(
        200,
        Ipv4Address::new(192, 168, 1, 1),
        0x11111111,
        Ipv4Address::new(192, 168, 2, 2),
        0x22222222,
        3, // 3 drops threshold
    );
    engine.add_session(session);

    let (ip, teid, path) = engine.forward_user_plane(200).unwrap();
    assert_eq!(ip, Ipv4Address::new(192, 168, 1, 1));
    assert_eq!(teid, 0x11111111);
    assert_eq!(path, ActivePath::Primary);

    let sess = engine.sessions.get_mut(&200).unwrap();
    sess.report_primary_heartbeat(false);
    sess.report_primary_heartbeat(false);
    sess.report_primary_heartbeat(false); // Threshold reached -> Failover!
    assert_eq!(sess.active_path, ActivePath::Secondary);

    let (ip_failover, teid_failover, path_failover) = engine.forward_user_plane(200).unwrap();
    assert_eq!(ip_failover, Ipv4Address::new(192, 168, 2, 2));
    assert_eq!(teid_failover, 0x22222222);
    assert_eq!(path_failover, ActivePath::Secondary);
}
