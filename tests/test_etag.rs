use toy_tcpip::etag::{ETHERTYPE_ETAG, ETagFrame, ETagHeader};
use toy_tcpip::ethernet::MacAddress;

#[test]
fn test_etag_encapsulation_and_decapsulation() {
    let dst = MacAddress([0x00, 0x1A, 0x2B, 0x3C, 0x4D, 0x5E]);
    let src = MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);

    let etag = ETagHeader {
        pcp: 7,
        dei: true,
        ingress_e_cid: 0xABCDE, // 20-bit port ID
        grp: 1,
        e_cid: 0x12345,          // 20-bit target port ID
        inner_ethertype: 0x86DD, // IPv6
    };

    let frame = ETagFrame::new(dst, src, etag, vec![1, 2, 3, 4, 5]);
    let raw = frame.serialize();

    let parsed = ETagFrame::parse(&raw).unwrap();
    assert_eq!(parsed.dst_mac, dst);
    assert_eq!(parsed.src_mac, src);
    assert_eq!(parsed.etag.pcp, 7);
    assert!(parsed.etag.dei);
    assert_eq!(parsed.etag.ingress_e_cid, 0xABCDE);
    assert_eq!(parsed.etag.grp, 1);
    assert_eq!(parsed.etag.e_cid, 0x12345);
    assert_eq!(parsed.etag.inner_ethertype, 0x86DD);
    assert_eq!(parsed.payload, vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_etag_constant() {
    assert_eq!(ETHERTYPE_ETAG, 0x893F);
}
