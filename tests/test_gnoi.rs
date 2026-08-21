use toy_tcpip::gnoi::{GnoiHealthStatus, GnoiServer, GNOI_PORT, GNOI_VERSION};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_gnoi_system_ping_and_healthz() {
    let server = GnoiServer::new();
    let ping_results = server.execute_ping(Ipv4Address::new(10, 0, 0, 1), 4);
    assert_eq!(ping_results.len(), 4);
    assert_eq!(ping_results[3].sequence, 4);

    let health = server.check_health();
    assert!(health.iter().any(|h| h.component == "SwitchingFabric" && h.status == GnoiHealthStatus::Healthy));
}

#[test]
fn test_gnoi_os_verify_and_constants() {
    let server = GnoiServer::new();
    let (version, verified) = server.verify_os();
    assert_eq!(version, "ToyNOS-v2.5.0-LTS");
    assert!(verified);

    assert_eq!(GNOI_PORT, 9339);
    assert_eq!(GNOI_VERSION, "0.1.0");
}
