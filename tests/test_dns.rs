use toy_tcpip::dns::{DnsMessage, DNS_CLASS_IN, DNS_TYPE_A};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_dns_encode_decode_queries_and_answers() {
    let query_raw = DnsMessage::build_query(0x1337, "google.com");
    let query = DnsMessage::parse(&query_raw).unwrap();

    assert_eq!(query.id, 0x1337);
    assert!(!query.is_response);
    assert!(query.recursion_desired);
    assert_eq!(query.questions.len(), 1);
    assert_eq!(query.questions[0].name, "google.com");
    assert_eq!(query.questions[0].qtype, DNS_TYPE_A);
    assert_eq!(query.questions[0].qclass, DNS_CLASS_IN);

    let resolved_ip = Ipv4Address::new(142, 250, 190, 78);
    let resp_raw = DnsMessage::build_response(0x1337, "google.com", resolved_ip, 300);
    let resp = DnsMessage::parse(&resp_raw).unwrap();

    assert_eq!(resp.id, 0x1337);
    assert!(resp.is_response);
    assert_eq!(resp.answers.len(), 1);
    assert_eq!(resp.answers[0].name, "google.com");
    assert_eq!(resp.answers[0].ip, resolved_ip);
    assert_eq!(resp.answers[0].ttl, 300);
}
