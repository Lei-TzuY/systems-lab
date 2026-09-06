use toy_tcpip::bgp_prefix_sid::{
    BGP_ATTR_PREFIX_SID, BGP_PREFIX_SID_TLV_IPV6_NODE_SID, BGP_PREFIX_SID_TLV_LABEL_INDEX,
    BGP_PREFIX_SID_TLV_ORIGINATOR_SRGB, BgpPrefixSidAttribute, LabelIndexTlv, OriginatorSrgbTlv,
};

#[test]
fn test_bgp_prefix_sid_constants() {
    assert_eq!(BGP_ATTR_PREFIX_SID, 40);
    assert_eq!(BGP_PREFIX_SID_TLV_LABEL_INDEX, 1);
    assert_eq!(BGP_PREFIX_SID_TLV_IPV6_NODE_SID, 2);
    assert_eq!(BGP_PREFIX_SID_TLV_ORIGINATOR_SRGB, 3);
}

#[test]
fn test_bgp_prefix_sid_roundtrip_codec() {
    let srgb = OriginatorSrgbTlv::new(16000, 8000);
    let srgb_bytes = srgb.serialize();
    let parsed_srgb = OriginatorSrgbTlv::parse(&srgb_bytes).unwrap();
    assert_eq!(parsed_srgb, srgb);

    let li = LabelIndexTlv::new(500);
    let li_bytes = li.serialize();
    let parsed_li = LabelIndexTlv::parse(&li_bytes).unwrap();
    assert_eq!(parsed_li, li);

    let attr = BgpPrefixSidAttribute::new(Some(500), Some(16000), Some(8000));
    assert_eq!(attr.calculate_absolute_label(16000), Some(16500));
}
