use toy_tcpip::eigrp::{EIGRP_OPCODE_HELLO, EigrpMetric, EigrpPacket, EigrpTopologyTable};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_eigrp_hello_packet_serialization() {
    let hello = EigrpPacket::build_hello(200);
    let raw = hello.serialize();

    let parsed = EigrpPacket::parse(&raw, true).unwrap();
    assert_eq!(parsed.header.opcode, EIGRP_OPCODE_HELLO);
    assert_eq!(parsed.header.as_number, 200);
    assert_eq!(parsed.header.version, 2);
}

#[test]
fn test_eigrp_dual_metric_and_successor_election() {
    let metric_fast = EigrpMetric::new(100_000, 100); // 100Mbps, 1ms
    let metric_slow = EigrpMetric::new(10_000, 1000); // 10Mbps, 10ms

    assert!(metric_fast.calculate_composite_metric() < metric_slow.calculate_composite_metric());

    let mut table = EigrpTopologyTable::new();
    let dest = Ipv4Address::new(172, 16, 1, 0);

    // Primary path via R1: Total Metric = 20000, RD = 15000
    // Secondary path via R2: Total Metric = 25000, RD = 18000 (< 20000 -> Feasible Successor)
    // Suboptimal path via R3: Total Metric = 30000, RD = 22000 (> 20000 -> Not Feasible Successor)
    let r1 = Ipv4Address::new(192, 168, 1, 1);
    let r2 = Ipv4Address::new(192, 168, 1, 2);
    let r3 = Ipv4Address::new(192, 168, 1, 3);

    table.add_candidate(dest, r1, 15000, 20000);
    table.add_candidate(dest, r2, 18000, 25000);
    table.add_candidate(dest, r3, 22000, 30000);

    let (successor, feasible_successors, fd) = table.compute_dual(dest).unwrap();
    assert_eq!(successor.neighbor, r1);
    assert_eq!(fd, 20000);

    // Only R2 meets RD (18000) < FD (20000)
    assert_eq!(feasible_successors.len(), 1);
    assert_eq!(feasible_successors[0].neighbor, r2);
}
