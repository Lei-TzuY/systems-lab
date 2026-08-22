//! Adversarial RFC 4456 input: ORIGINATOR_ID and CLUSTER_LIST as a hostile or
//! broken neighbour can put them on the wire.
//!
//! Every message here reaches the router under test as bytes over a real TCP
//! connection to port 179, written by a plain socket rather than by a second
//! speaker, so nothing is validated twice by a well-behaved sender first.
//!
//! Two kinds of outcome are being separated, and the difference matters:
//!
//! * a *malformed* attribute - wrong length, wrong flags, sent twice - is an
//!   UPDATE error, and the session must be reset with a NOTIFICATION;
//! * a *looped* route - our own ORIGINATOR_ID, our own cluster - is perfectly
//!   well-formed and the sender did nothing wrong, so the route is dropped and
//!   the session is left alone.
//!
//! Treating the second as the first would tear a fabric down every time
//! redundancy did its job.

mod common;

use common::bgp_lab::{RawBgpPeer, ip, prefix};
use toy_tcpip::bgp::{
    AsPath, BGP_ATTR_CLUSTER_LIST, BGP_ATTR_FLAG_EXT_LEN, BGP_ATTR_FLAG_OPTIONAL,
    BGP_ATTR_FLAG_TRANSITIVE, BGP_ATTR_ORIGINATOR_ID, BGP_ERR_UPDATE_MESSAGE, BGP_MARKER,
    BGP_MSG_UPDATE, BGP_SUB_ATTRIBUTE_FLAGS_ERROR, BGP_SUB_ATTRIBUTE_LENGTH_ERROR,
    BGP_SUB_MALFORMED_ATTRIBUTE_LIST, BgpOrigin, BgpPathAttributes, BgpPdu, BgpUpdateMessage,
    Ipv4Prefix, MAX_CLUSTER_LIST_LEN,
};
use toy_tcpip::bgp_caps::AfiSafi;
use toy_tcpip::bgp_evpn::{RouteTarget, encode_evpn_nlri_list};
use toy_tcpip::bgp_mp::MpReachNlri;
use toy_tcpip::bgp_router::BgpState;
use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::evpn::{EvpnNlri, RouteDistinguisher};
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::lab::evpn_rt;

const AS1: u32 = 65001;
const VNI: u32 = 5001;
const VTEP1: Ipv4Address = Ipv4Address([10, 0, 0, 1]);
const VTEP2: Ipv4Address = Ipv4Address([10, 0, 0, 2]);
/// The BGP identifier of the router under test.
const VICTIM_ID: Ipv4Address = Ipv4Address([9, 9, 9, 9]);

fn mac(last: u8) -> MacAddress {
    MacAddress([0x02, 0x00, 0x00, 0x00, 0x00, last])
}

/// An iBGP peer of the router under test, already ESTABLISHED with EVPN
/// negotiated. Internal, because reflection only happens inside one AS.
fn victim() -> RawBgpPeer {
    let mut peer = RawBgpPeer::connect_configured(AS1, AS1, VICTIM_ID, |r| {
        r.add_interface(
            "eth1",
            MacAddress([0x02, 0, 0, 0, 0xBB, 0x02]),
            ip(192, 168, 10, 1),
            24,
            "tenant",
        );
        r.enable_vtep(VTEP1, "eth0");
        r.add_evpn_instance(
            VNI,
            RouteDistinguisher::new(VTEP1, VNI as u16),
            &[evpn_rt(65001, VNI)],
            &[evpn_rt(65001, VNI)],
        );
        r.attach_evpn_access_port(VNI, "eth1");
    });
    peer.establish();
    assert_eq!(peer.state(), BgpState::Established);
    peer
}

/// The IPv4 prefix used by the well-formed half of these tests.
fn test_prefix() -> Ipv4Prefix {
    prefix(172, 30, 0, 0, 24)
}

/// A well-formed IPv4 UPDATE with whatever reflection metadata is asked for.
fn ipv4_update(originator: Option<Ipv4Address>, clusters: Vec<Ipv4Address>) -> BgpPdu {
    let mut attrs = BgpPathAttributes::new(BgpOrigin::Igp, AsPath::empty(), ip(10, 50, 0, 2));
    attrs.four_octet_as = true;
    attrs.local_pref = Some(100);
    attrs.originator_id = originator;
    attrs.cluster_list = clusters;
    BgpPdu::Update(BgpUpdateMessage::announce(attrs, vec![test_prefix()]))
}

/// A well-formed EVPN MP_REACH UPDATE with whatever reflection metadata is asked
/// for. The Route Target is one the router imports, so nothing but the metadata
/// can be the reason it is refused.
fn evpn_update(originator: Option<Ipv4Address>, clusters: Vec<Ipv4Address>) -> BgpPdu {
    let nlri = EvpnNlri::build_mac_ip(
        RouteDistinguisher::new(VTEP2, VNI as u16),
        mac(0x22),
        None,
        VNI,
    );
    let mut attrs =
        BgpPathAttributes::new(BgpOrigin::Igp, AsPath::empty(), Ipv4Address::UNSPECIFIED);
    attrs.four_octet_as = true;
    attrs.local_pref = Some(100);
    attrs.ext_communities = vec![RouteTarget::as2(65001, VNI).to_bytes()];
    attrs.originator_id = originator;
    attrs.cluster_list = clusters;
    attrs.mp_reach = Some(MpReachNlri::with_ipv4_next_hop(
        AfiSafi::L2VPN_EVPN,
        VTEP2,
        encode_evpn_nlri_list(&[nlri]),
    ));
    BgpPdu::Update(BgpUpdateMessage::mp_announce(attrs))
}

/// Builds an UPDATE whose attribute block is written byte by byte, so a length or
/// a flag the encoder would never produce can be put on the wire.
///
/// `extra` is appended after ORIGIN, AS_PATH and NEXT_HOP, and one IPv4 prefix
/// follows the attributes, which is what makes those three mandatory.
fn update_with_raw_attributes(extra: &[u8]) -> Vec<u8> {
    let mut attrs = Vec::new();
    attrs.extend_from_slice(&[BGP_ATTR_FLAG_TRANSITIVE, 1, 1, 0]); // ORIGIN = IGP
    let path = AsPath::empty().encode_width(true);
    attrs.extend_from_slice(&[BGP_ATTR_FLAG_TRANSITIVE, 2, path.len() as u8]);
    attrs.extend_from_slice(&path);
    attrs.extend_from_slice(&[BGP_ATTR_FLAG_TRANSITIVE, 3, 4]); // NEXT_HOP
    attrs.extend_from_slice(&ip(10, 50, 0, 2).0);
    attrs.extend_from_slice(extra);

    let mut nlri = Vec::new();
    test_prefix().encode(&mut nlri);

    let mut body = Vec::new();
    body.extend_from_slice(&0u16.to_be_bytes());
    body.extend_from_slice(&(attrs.len() as u16).to_be_bytes());
    body.extend_from_slice(&attrs);
    body.extend_from_slice(&nlri);

    let mut frame = Vec::new();
    frame.extend_from_slice(&BGP_MARKER);
    frame.extend_from_slice(&((19 + body.len()) as u16).to_be_bytes());
    frame.push(BGP_MSG_UPDATE);
    frame.extend_from_slice(&body);
    frame
}

/// One raw attribute, one-octet length form.
fn raw_attr(flags: u8, type_code: u8, value: &[u8]) -> Vec<u8> {
    let mut out = vec![flags, type_code, value.len() as u8];
    out.extend_from_slice(value);
    out
}

/// Sends `frame`, then asserts the session was reset with an UPDATE error and
/// that nothing entered any RIB.
fn expect_update_error(peer: &mut RawBgpPeer, frame: &[u8], subcode: u8, what: &str) {
    peer.write(frame);
    let note = peer
        .notification()
        .unwrap_or_else(|| panic!("{}: no NOTIFICATION was sent", what));
    assert_eq!(
        note.error_code, BGP_ERR_UPDATE_MESSAGE,
        "{}: wrong NOTIFICATION code",
        what
    );
    assert_eq!(note.error_subcode, subcode, "{}: wrong subcode", what);
    assert_ne!(
        peer.state(),
        BgpState::Established,
        "{}: the session survived a malformed attribute",
        what
    );
    let bgp = peer.victim_bgp();
    assert_eq!(
        bgp.adj_rib_in.path_count(),
        0,
        "{}: a path was stored",
        what
    );
    assert_eq!(
        bgp.evpn_adj_rib_in.total_routes(),
        0,
        "{}: an EVPN route was stored",
        what
    );
}

// ============================================================================
// Malformed ORIGINATOR_ID
// ============================================================================

#[test]
fn test_an_originator_id_that_is_not_four_bytes_is_refused() {
    for len in [0usize, 1, 2, 3, 5, 8, 255] {
        let mut peer = victim();
        let value = vec![0xAAu8; len];
        let frame = update_with_raw_attributes(&raw_attr(
            BGP_ATTR_FLAG_OPTIONAL,
            BGP_ATTR_ORIGINATOR_ID,
            &value,
        ));
        expect_update_error(
            &mut peer,
            &frame,
            BGP_SUB_ATTRIBUTE_LENGTH_ERROR,
            &format!("ORIGINATOR_ID of {} bytes", len),
        );
    }
}

#[test]
fn test_an_originator_id_with_the_wrong_flags_is_refused() {
    // Optional and non-transitive, and nothing else. A well-known ORIGINATOR_ID
    // would have to be understood by every speaker; a transitive one would leak
    // out of the AS whose reflection it describes.
    for (flags, what) in [
        (0u8, "well-known"),
        (BGP_ATTR_FLAG_TRANSITIVE, "transitive but not optional"),
        (
            BGP_ATTR_FLAG_OPTIONAL | BGP_ATTR_FLAG_TRANSITIVE,
            "optional transitive",
        ),
    ] {
        let mut peer = victim();
        let frame =
            update_with_raw_attributes(&raw_attr(flags, BGP_ATTR_ORIGINATOR_ID, &ip(1, 2, 3, 4).0));
        expect_update_error(
            &mut peer,
            &frame,
            BGP_SUB_ATTRIBUTE_FLAGS_ERROR,
            &format!("ORIGINATOR_ID marked {}", what),
        );
    }
}

#[test]
fn test_a_duplicate_originator_id_is_refused() {
    let mut peer = victim();
    let mut extra = raw_attr(
        BGP_ATTR_FLAG_OPTIONAL,
        BGP_ATTR_ORIGINATOR_ID,
        &ip(1, 2, 3, 4).0,
    );
    extra.extend_from_slice(&raw_attr(
        BGP_ATTR_FLAG_OPTIONAL,
        BGP_ATTR_ORIGINATOR_ID,
        &ip(5, 6, 7, 8).0,
    ));
    let frame = update_with_raw_attributes(&extra);
    expect_update_error(
        &mut peer,
        &frame,
        BGP_SUB_MALFORMED_ATTRIBUTE_LIST,
        "two ORIGINATOR_ID attributes",
    );
}

// ============================================================================
// Malformed CLUSTER_LIST
// ============================================================================

#[test]
fn test_a_cluster_list_that_is_not_a_multiple_of_four_is_refused() {
    for len in [0usize, 1, 2, 3, 5, 6, 7, 9, 10, 11, 13] {
        let mut peer = victim();
        let value = vec![0xBBu8; len];
        let frame = update_with_raw_attributes(&raw_attr(
            BGP_ATTR_FLAG_OPTIONAL,
            BGP_ATTR_CLUSTER_LIST,
            &value,
        ));
        expect_update_error(
            &mut peer,
            &frame,
            BGP_SUB_ATTRIBUTE_LENGTH_ERROR,
            &format!("CLUSTER_LIST of {} bytes", len),
        );
    }
}

#[test]
fn test_a_cluster_list_with_the_wrong_flags_is_refused() {
    for (flags, what) in [
        (0u8, "well-known"),
        (BGP_ATTR_FLAG_TRANSITIVE, "transitive but not optional"),
        (
            BGP_ATTR_FLAG_OPTIONAL | BGP_ATTR_FLAG_TRANSITIVE,
            "optional transitive",
        ),
    ] {
        let mut peer = victim();
        let frame =
            update_with_raw_attributes(&raw_attr(flags, BGP_ATTR_CLUSTER_LIST, &ip(1, 1, 1, 1).0));
        expect_update_error(
            &mut peer,
            &frame,
            BGP_SUB_ATTRIBUTE_FLAGS_ERROR,
            &format!("CLUSTER_LIST marked {}", what),
        );
    }
}

#[test]
fn test_a_duplicate_cluster_list_is_refused() {
    let mut peer = victim();
    let mut extra = raw_attr(
        BGP_ATTR_FLAG_OPTIONAL,
        BGP_ATTR_CLUSTER_LIST,
        &ip(1, 1, 1, 1).0,
    );
    extra.extend_from_slice(&raw_attr(
        BGP_ATTR_FLAG_OPTIONAL,
        BGP_ATTR_CLUSTER_LIST,
        &ip(2, 2, 2, 2).0,
    ));
    let frame = update_with_raw_attributes(&extra);
    expect_update_error(
        &mut peer,
        &frame,
        BGP_SUB_MALFORMED_ATTRIBUTE_LIST,
        "two CLUSTER_LIST attributes",
    );
}

#[test]
fn test_an_oversized_cluster_list_is_refused() {
    // One entry past the accepted ceiling. The extended length form is needed,
    // because the value is longer than a single length octet can describe.
    let mut peer = victim();
    let count = MAX_CLUSTER_LIST_LEN + 1;
    let mut value = Vec::with_capacity(count * 4);
    for i in 0..count {
        value.extend_from_slice(&[10, 0, (i >> 8) as u8, i as u8]);
    }
    let mut extra = vec![
        BGP_ATTR_FLAG_OPTIONAL | BGP_ATTR_FLAG_EXT_LEN,
        BGP_ATTR_CLUSTER_LIST,
    ];
    extra.extend_from_slice(&(value.len() as u16).to_be_bytes());
    extra.extend_from_slice(&value);
    let frame = update_with_raw_attributes(&extra);
    expect_update_error(
        &mut peer,
        &frame,
        BGP_SUB_ATTRIBUTE_LENGTH_ERROR,
        "a CLUSTER_LIST longer than the accepted ceiling",
    );
}

#[test]
fn test_a_cluster_list_at_exactly_the_ceiling_is_accepted() {
    // The boundary in the other direction: the largest legal list must work, or
    // the ceiling would be an off-by-one that silently breaks deep hierarchies.
    let mut peer = victim();
    let clusters: Vec<Ipv4Address> = (0..MAX_CLUSTER_LIST_LEN)
        .map(|i| Ipv4Address::new(10, 0, (i >> 8) as u8, i as u8))
        .collect();
    peer.write_pdu(ipv4_update(Some(ip(1, 2, 3, 4)), clusters.clone()));

    assert_eq!(
        peer.state(),
        BgpState::Established,
        "a legal maximum-length CLUSTER_LIST reset the session"
    );
    let stored = peer
        .victim_bgp()
        .adj_rib_in
        .iter_paths()
        .find(|p| p.prefix == test_prefix())
        .expect("the route was not stored")
        .clone();
    assert_eq!(stored.cluster_list, clusters);
    assert_eq!(stored.originator_id, Some(ip(1, 2, 3, 4)));
}

// ============================================================================
// Loop detection: well-formed, but ours
// ============================================================================

#[test]
fn test_a_route_whose_originator_is_us_is_dropped_without_resetting_the_session() {
    let mut peer = victim();
    peer.write_pdu(ipv4_update(Some(VICTIM_ID), vec![]));

    assert_eq!(
        peer.state(),
        BgpState::Established,
        "a looped route reset the session; it is not a protocol violation"
    );
    assert!(
        peer.notification().is_none(),
        "a NOTIFICATION was sent for a well-formed route"
    );
    let bgp = peer.victim_bgp();
    assert_eq!(
        bgp.adj_rib_in.path_count(),
        0,
        "a route carrying our own ORIGINATOR_ID was accepted"
    );
    assert_eq!(bgp.peers()[0].counters.originator_loops_rejected, 1);
    assert_eq!(bgp.peers()[0].counters.cluster_loops_rejected, 0);
}

#[test]
fn test_a_route_carrying_our_own_cluster_is_dropped_without_resetting_the_session() {
    let mut peer = victim();
    // The router's cluster ID defaults to its BGP identifier.
    peer.write_pdu(ipv4_update(Some(ip(1, 2, 3, 4)), vec![VICTIM_ID]));

    assert_eq!(peer.state(), BgpState::Established);
    assert!(peer.notification().is_none());
    let bgp = peer.victim_bgp();
    assert_eq!(bgp.adj_rib_in.path_count(), 0);
    assert_eq!(bgp.peers()[0].counters.cluster_loops_rejected, 1);
}

#[test]
fn test_our_cluster_is_found_anywhere_in_the_list_not_only_at_the_front() {
    let mut peer = victim();
    peer.write_pdu(ipv4_update(
        Some(ip(1, 2, 3, 4)),
        vec![ip(20, 0, 0, 1), ip(20, 0, 0, 2), VICTIM_ID, ip(20, 0, 0, 3)],
    ));
    assert_eq!(peer.state(), BgpState::Established);
    assert_eq!(peer.victim_bgp().adj_rib_in.path_count(), 0);
    assert_eq!(
        peer.victim_bgp().peers()[0].counters.cluster_loops_rejected,
        1
    );
}

#[test]
fn test_an_evpn_route_whose_originator_is_us_is_dropped_and_programs_nothing() {
    let mut peer = victim();
    peer.write_pdu(evpn_update(Some(VICTIM_ID), vec![]));

    assert_eq!(peer.state(), BgpState::Established);
    assert!(peer.notification().is_none());
    let bgp = peer.victim_bgp();
    assert_eq!(
        bgp.evpn_adj_rib_in.total_routes(),
        0,
        "an EVPN route carrying our own ORIGINATOR_ID was accepted"
    );
    assert_eq!(bgp.peers()[0].counters.originator_loops_rejected, 1);
    assert_eq!(
        peer.lab
            .router("victim")
            .unwrap()
            .vtep()
            .unwrap()
            .remote_mac_count(),
        0,
        "a looped EVPN route programmed a tunnel"
    );
}

#[test]
fn test_an_evpn_route_carrying_our_own_cluster_is_dropped_and_programs_nothing() {
    let mut peer = victim();
    peer.write_pdu(evpn_update(Some(ip(1, 2, 3, 4)), vec![VICTIM_ID]));

    assert_eq!(peer.state(), BgpState::Established);
    let bgp = peer.victim_bgp();
    assert_eq!(bgp.evpn_adj_rib_in.total_routes(), 0);
    assert_eq!(bgp.peers()[0].counters.cluster_loops_rejected, 1);
    assert_eq!(
        peer.lab
            .router("victim")
            .unwrap()
            .vtep()
            .unwrap()
            .remote_mac_count(),
        0
    );
}

#[test]
fn test_a_route_from_a_different_cluster_and_originator_is_accepted() {
    // The control for the two tests above: identical in every way except that
    // the metadata names somebody else.
    let mut peer = victim();
    peer.write_pdu(evpn_update(
        Some(ip(1, 2, 3, 4)),
        vec![ip(20, 0, 0, 1), ip(20, 0, 0, 2)],
    ));

    assert_eq!(peer.state(), BgpState::Established);
    let bgp = peer.victim_bgp();
    assert_eq!(bgp.evpn_adj_rib_in.total_routes(), 1);
    assert_eq!(bgp.peers()[0].counters.originator_loops_rejected, 0);
    assert_eq!(bgp.peers()[0].counters.cluster_loops_rejected, 0);

    let path = bgp.evpn_adj_rib_in.iter_paths().next().unwrap();
    assert_eq!(path.originator_id, Some(ip(1, 2, 3, 4)));
    assert_eq!(path.cluster_list, vec![ip(20, 0, 0, 1), ip(20, 0, 0, 2)]);
    assert_eq!(
        peer.lab
            .router("victim")
            .unwrap()
            .vtep()
            .unwrap()
            .lookup_remote(VNI, &mac(0x22)),
        Some(VTEP2),
        "a legitimately reflected EVPN route did not program the overlay"
    );
}

// ============================================================================
// Reflection metadata on things that should not carry it
// ============================================================================

#[test]
fn test_reflection_metadata_on_an_unnegotiated_family_is_still_refused() {
    // A session that never negotiated EVPN gets EVPN NLRI, dressed up as a
    // reflection. The capability check must fire regardless of the metadata.
    let mut peer = RawBgpPeer::connect_configured(AS1, AS1, VICTIM_ID, |_| {});
    peer.establish_legacy();
    assert_eq!(peer.state(), BgpState::Established);

    peer.write_pdu(evpn_update(Some(ip(1, 2, 3, 4)), vec![ip(20, 0, 0, 1)]));
    assert_ne!(
        peer.state(),
        BgpState::Established,
        "EVPN NLRI was accepted on a session that never negotiated the family"
    );
    assert_eq!(peer.victim_bgp().evpn_adj_rib_in.total_routes(), 0);
}

#[test]
fn test_a_truncated_cluster_list_attribute_is_refused_not_read_short() {
    // The length octet claims eight bytes but only four follow, and the
    // attribute block ends there. Reading what is present would silently accept
    // half a cluster list.
    let mut peer = victim();
    let mut extra = vec![BGP_ATTR_FLAG_OPTIONAL, BGP_ATTR_CLUSTER_LIST, 8];
    extra.extend_from_slice(&ip(1, 1, 1, 1).0);
    let frame = update_with_raw_attributes(&extra);
    expect_update_error(
        &mut peer,
        &frame,
        BGP_SUB_ATTRIBUTE_LENGTH_ERROR,
        "a CLUSTER_LIST whose length runs past the attribute block",
    );
}

#[test]
fn test_a_peer_that_flaps_mid_update_leaves_nothing_behind() {
    // Half a reflected UPDATE, then the connection goes away. Nothing may be
    // stored from a message that was never completed.
    let mut peer = victim();
    let frame = {
        let mut attrs = BgpPathAttributes::new(BgpOrigin::Igp, AsPath::empty(), ip(10, 50, 0, 2));
        attrs.four_octet_as = true;
        attrs.originator_id = Some(ip(1, 2, 3, 4));
        attrs.cluster_list = vec![ip(20, 0, 0, 1)];
        BgpPdu::Update(BgpUpdateMessage::announce(attrs, vec![test_prefix()])).serialize()
    };
    let half = frame.len() / 2;
    peer.write(&frame[..half]);
    assert_eq!(peer.victim_bgp().adj_rib_in.path_count(), 0);
    peer.disconnect();
    peer.lab.run_until(250, 30_000, |_| false);

    let bgp = peer.victim_bgp();
    assert_eq!(
        bgp.adj_rib_in.path_count(),
        0,
        "a partial UPDATE left a path behind after the peer vanished"
    );
    assert_eq!(bgp.evpn_adj_rib_in.total_routes(), 0);
    assert_ne!(bgp.peers()[0].state, BgpState::Established);
}

#[test]
fn test_every_reflection_attribute_length_is_survivable() {
    // A sweep rather than a list of cases: for each of the two attributes, every
    // length from nothing to well past what is legal. The only requirement is
    // that the process survives and no route is stored from a refused message.
    for type_code in [BGP_ATTR_ORIGINATOR_ID, BGP_ATTR_CLUSTER_LIST] {
        for len in 0usize..=64 {
            let mut peer = victim();
            let value = vec![0x5Au8; len];
            let frame =
                update_with_raw_attributes(&raw_attr(BGP_ATTR_FLAG_OPTIONAL, type_code, &value));
            peer.write(&frame);

            let legal = match type_code {
                BGP_ATTR_ORIGINATOR_ID => len == 4,
                _ => len > 0 && len.is_multiple_of(4),
            };
            if legal {
                assert_eq!(
                    peer.state(),
                    BgpState::Established,
                    "attribute {} of {} legal bytes reset the session",
                    type_code,
                    len
                );
            } else {
                assert_ne!(
                    peer.state(),
                    BgpState::Established,
                    "attribute {} of {} bytes was accepted",
                    type_code,
                    len
                );
                assert_eq!(peer.victim_bgp().adj_rib_in.path_count(), 0);
            }
        }
    }
}
