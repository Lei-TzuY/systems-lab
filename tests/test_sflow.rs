use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::sflow::{SflowCounterSample, SflowDatagram, SflowFlowSample, SflowSample, SFLOW_FORMAT_COUNTER_SAMPLE, SFLOW_FORMAT_FLOW_SAMPLE, SFLOW_UDP_PORT, SFLOW_VERSION_5};

#[test]
fn test_sflow_datagram_encoding_and_parsing() {
    let agent = Ipv4Address::new(172, 20, 0, 1);
    let mut dgram = SflowDatagram::new(agent, 1000, 720000);

    let flow = SflowFlowSample {
        seq_num: 1,
        source_id: 1,
        sampling_rate: 2000,
        sample_pool: 100000,
        drops: 0,
        input_if: 10,
        output_if: 20,
        orig_packet_len: 256,
        sampled_header: vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x08, 0x00],
    };
    dgram.samples.push(SflowSample::Flow(flow));

    let counter = SflowCounterSample {
        seq_num: 1,
        source_id: 1,
        if_index: 10,
        if_speed_bps: 40_000_000_000,
        in_octets: 50_000_000,
        in_packets: 100_000,
        out_octets: 40_000_000,
        out_packets: 80_000,
    };
    dgram.samples.push(SflowSample::Counter(counter));

    let raw = dgram.serialize();
    let parsed = SflowDatagram::parse(&raw).unwrap();

    assert_eq!(parsed.version, SFLOW_VERSION_5);
    assert_eq!(parsed.agent_ip, agent);
    assert_eq!(parsed.seq_num, 1000);
    assert_eq!(parsed.uptime_ms, 720000);
    assert_eq!(parsed.samples.len(), 2);

    match &parsed.samples[0] {
        SflowSample::Flow(f) => {
            assert_eq!(f.sampling_rate, 2000);
            assert_eq!(f.input_if, 10);
            assert_eq!(f.output_if, 20);
            assert_eq!(f.orig_packet_len, 256);
        }
        _ => panic!("Expected flow sample"),
    }

    match &parsed.samples[1] {
        SflowSample::Counter(c) => {
            assert_eq!(c.if_index, 10);
            assert_eq!(c.if_speed_bps, 40_000_000_000);
            assert_eq!(c.in_packets, 100_000);
        }
        _ => panic!("Expected counter sample"),
    }

    assert_eq!(SFLOW_UDP_PORT, 6343);
    assert_eq!(SFLOW_FORMAT_FLOW_SAMPLE, 1);
    assert_eq!(SFLOW_FORMAT_COUNTER_SAMPLE, 2);
}
