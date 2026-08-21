use toy_tcpip::ldap::{LDAP_PORT, LDAP_SUCCESS, LdapMessage, LdapOp, LdapServer};

#[test]
fn test_ldap_bind_and_search_framing() {
    let msg = LdapMessage::new_bind_request(10, "cn=admin,dc=example,dc=org", "adminpass");
    let raw = msg.serialize();

    let parsed = LdapMessage::parse(&raw).unwrap();
    assert_eq!(parsed.message_id, 10);
    assert_eq!(LDAP_PORT, 389);
}

#[test]
fn test_ldap_server_directory_lookup() {
    let srv = LdapServer::new();
    let search = LdapMessage::new_search_request(
        11,
        "dc=example,dc=org",
        "(objectClass=*)",
        &["cn", "mail"],
    );
    let responses = srv.handle_request(&search);

    assert_eq!(responses.len(), 3); // 2 Entries + 1 SearchResultDone

    let mut found_alice = false;
    let mut found_bob = false;
    let mut found_done = false;

    for resp in &responses {
        match &resp.protocol_op {
            LdapOp::SearchResultEntry {
                object_name,
                attributes,
            } => {
                if object_name.contains("alice") {
                    found_alice = true;
                    assert!(attributes.iter().any(|(k, _)| k == "mail"));
                }
                if object_name.contains("bob") {
                    found_bob = true;
                }
            }
            LdapOp::SearchResultDone { result_code, .. } => {
                found_done = true;
                assert_eq!(*result_code, LDAP_SUCCESS);
            }
            _ => {}
        }
    }

    assert!(found_alice);
    assert!(found_bob);
    assert!(found_done);
}
