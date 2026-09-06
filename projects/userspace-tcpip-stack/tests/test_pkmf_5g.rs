//! Integration tests for 3GPP TS 29.559 / TS 33.536 5G PKMF (Public Key Management Function).

use toy_tcpip::pkmf_5g::*;

// ---------------------------------------------------------------------------
// 1. Key Request and PC5 Sidelink KDF Happy Path
// ---------------------------------------------------------------------------

#[test]
fn test_pkmf_key_request_and_pc5_kdf_happy_path() {
    let mut pkmf = PkmfEngine::new("pkmf-core-01");
    let group = "prose-group-police-01";
    let cop1 = "imsi-208950000000001";
    let cop2 = "imsi-208950000000002";

    pkmf.create_prose_group(group, vec![cop1, cop2], 1000);

    // Request group key for cop1
    let resp = pkmf
        .request_group_key(cop1, group, 1050)
        .expect("Group key request failed");

    assert_eq!(resp.prose_group_id, group);
    assert_eq!(resp.pgk_id, 1);
    assert_ne!(resp.pgk, [0u8; 32]);
    assert_eq!(resp.valid_until_epoch_s, 1000 + 86400);

    // Derive PC5 session encryption & integrity keys
    let nonce = [0xAA; 16];
    let traffic_keys = PkmfEngine::derive_pc5_session_keys(&resp.pgk, &nonce);
    assert_ne!(traffic_keys.pek, [0u8; 16]);
    assert_ne!(traffic_keys.pik, [0u8; 16]);
    assert_ne!(traffic_keys.pek, traffic_keys.pik);
}

// ---------------------------------------------------------------------------
// 2. Unauthorized Member Rejection
// ---------------------------------------------------------------------------

#[test]
fn test_pkmf_unauthorized_member_rejection() {
    let mut pkmf = PkmfEngine::new("pkmf-core-02");
    let group = "prose-group-firefighters";
    pkmf.create_prose_group(group, vec!["imsi-fire-01"], 1000);

    let err = pkmf.request_group_key("imsi-rogue-intruder", group, 1050);
    assert_eq!(err, Err(PkmfError::UnauthorizedGroupMember));
}

// ---------------------------------------------------------------------------
// 3. Key Expiration and Automatic Rollover
// ---------------------------------------------------------------------------

#[test]
fn test_pkmf_key_expiration_and_automatic_rollover() {
    let mut pkmf = PkmfEngine::new("pkmf-core-03");
    let group = "prose-group-convoy-fleet";
    let driver = "imsi-truck-01";

    pkmf.create_prose_group(group, vec![driver], 1000);

    // Initial key (pgk_id = 1)
    let key1 = pkmf.request_group_key(driver, group, 1050).unwrap();
    assert_eq!(key1.pgk_id, 1);

    // Fast-forward past default lifetime (86400s -> t = 90000)
    let key2 = pkmf.request_group_key(driver, group, 90000).unwrap();
    assert_eq!(key2.pgk_id, 2);
    assert_ne!(key1.pgk, key2.pgk);
    assert_eq!(key2.valid_until_epoch_s, 90000 + 86400);
}

// ---------------------------------------------------------------------------
// 4. Eviction and Emergency Key Rollover (Forward Secrecy)
// ---------------------------------------------------------------------------

#[test]
fn test_pkmf_eviction_and_emergency_key_rollover() {
    let mut pkmf = PkmfEngine::new("pkmf-core-04");
    let group = "prose-group-paramedic";
    let medic1 = "imsi-paramedic-01";
    let traitor = "imsi-compromised-agent";

    pkmf.create_prose_group(group, vec![medic1, traitor], 1000);

    let key_initial = pkmf.request_group_key(medic1, group, 1050).unwrap();
    assert_eq!(key_initial.pgk_id, 1);

    // Evict compromised agent at t = 2000
    pkmf.revoke_group_member(group, traitor, 2000)
        .expect("Revocation failed");

    // Medic 1 receives emergency rolled-over key (pgk_id = 2)
    let key_new = pkmf.request_group_key(medic1, group, 2010).unwrap();
    assert_eq!(key_new.pgk_id, 2);
    assert_ne!(key_initial.pgk, key_new.pgk);

    // Evicted agent is permanently barred
    let evicted_err = pkmf.request_group_key(traitor, group, 2020);
    assert_eq!(evicted_err, Err(PkmfError::UnauthorizedGroupMember));
}

// ---------------------------------------------------------------------------
// 5. Unknown Group Handling
// ---------------------------------------------------------------------------

#[test]
fn test_pkmf_unknown_group_handling() {
    let mut pkmf = PkmfEngine::new("pkmf-core-05");
    let err = pkmf.request_group_key("imsi-12345", "non-existent-group", 1000);
    assert_eq!(err, Err(PkmfError::GroupNotFound));
}
