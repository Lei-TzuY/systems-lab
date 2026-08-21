use std::str::FromStr;
use toy_tcpip::bgp_ls_srv6::{
    BGP_LS_TLV_SRV6_END_SID, BGP_LS_TLV_SRV6_LOCATOR, BgpLsSrv6Database, Srv6EndSidTlv,
    Srv6LocatorTlv,
};
use toy_tcpip::ipv6::Ipv6Address;

#[test]
fn test_bgp_ls_srv6_constants() {
    assert_eq!(BGP_LS_TLV_SRV6_LOCATOR, 1162);
    assert_eq!(BGP_LS_TLV_SRV6_END_SID, 1106);

    let end_sid = Srv6EndSidTlv::new(1, Ipv6Address::from_str("2001:db8:100::1").unwrap());
    assert_eq!(end_sid.endpoint_behavior, 1);
}

#[test]
fn test_bgp_ls_srv6_database_longest_prefix_matching() {
    let mut db = BgpLsSrv6Database::new();
    let loc1 = Srv6LocatorTlv::new(0, 10, Ipv6Address::from_str("2001:db8:100::").unwrap(), 64);
    let loc2 = Srv6LocatorTlv::new(
        128,
        20,
        Ipv6Address::from_str("2001:db8:200::").unwrap(),
        64,
    );

    db.add_locator(loc1.clone());
    db.add_locator(loc2.clone());

    let sid1 = Ipv6Address::from_str("2001:db8:100::dead:beef").unwrap();
    let sid2 = Ipv6Address::from_str("2001:db8:200::1").unwrap();
    let sid_unknown = Ipv6Address::from_str("2001:db8:999::1").unwrap();

    let matched1 = db.find_locator_for_sid(&sid1).unwrap();
    assert_eq!(matched1.locator, loc1.locator);

    let matched2 = db.find_locator_for_sid(&sid2).unwrap();
    assert_eq!(matched2.locator, loc2.locator);
    assert_eq!(matched2.algorithm, 128);

    assert!(db.find_locator_for_sid(&sid_unknown).is_none());
}
