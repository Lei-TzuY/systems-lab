use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::tsn_cnc::{
    CentralizedNetworkConfigurator, StreamId, TrafficSpecification, TsnListener, TsnTalker,
    UserToNetworkRequirements,
};

#[test]
fn test_tsn_cnc_bandwidth_and_latency_bounds() {
    let mut cnc = CentralizedNetworkConfigurator::new();
    let talker_mac = MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
    let stream_id = StreamId::new(talker_mac, 10);

    let tspec = TrafficSpecification {
        max_frame_size: 1000,
        max_interval_frames: 1,
        interval_us: 1000, // 1ms
    };

    // 1000 bytes * 8 bits / 0.001s = 8,000,000 bps
    let bw = CentralizedNetworkConfigurator::compute_stream_bandwidth(&tspec);
    assert_eq!(bw, 8_000_000);

    let talker = TsnTalker {
        stream_id,
        talker_mac,
        vlan_id: 100,
        priority: 7,
        tspec,
    };
    assert!(cnc.register_talker(talker).is_ok());

    let listener = TsnListener {
        stream_id,
        listener_mac: MacAddress([0x00, 0x66, 0x77, 0x88, 0x99, 0xAA]),
        reqs: UserToNetworkRequirements {
            max_latency_us: 3000, // 3ms
            num_seamless_trees: 1,
        },
    };

    let latency = cnc.register_listener(listener).unwrap();
    assert_eq!(latency, 2020);
    assert_eq!(cnc.talkers.len(), 1);
    assert_eq!(cnc.listeners.get(&stream_id).unwrap().len(), 1);
}

#[test]
fn test_tsn_stream_id_ordering_and_equality() {
    let mac1 = MacAddress([0x00, 0x00, 0x00, 0x00, 0x00, 0x01]);
    let mac2 = MacAddress([0x00, 0x00, 0x00, 0x00, 0x00, 0x02]);

    let sid1 = StreamId::new(mac1, 1);
    let sid2 = StreamId::new(mac2, 1);
    let sid3 = StreamId::new(mac1, 2);

    assert!(sid1 < sid2);
    assert!(sid1 < sid3);
    assert_eq!(sid1, StreamId::new(mac1, 1));
}
