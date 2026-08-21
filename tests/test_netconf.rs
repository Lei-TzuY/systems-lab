use toy_tcpip::netconf::{NetconfServer, NETCONF_EOM_1_0, NETCONF_PORT};

#[test]
fn test_netconf_xml_rpc_operations_and_datastore() {
    let mut server = NetconfServer::new();

    // 1. Hello exchange
    let client_hello = "<hello xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\"><capabilities><capability>urn:ietf:params:netconf:base:1.1</capability></capabilities></hello>]]>]]>";
    let resp_hello = server.handle_request(client_hello);
    assert_eq!(resp_hello.contains("<session-id>101</session-id>"), true);
    assert_eq!(resp_hello.contains("urn:ietf:params:netconf:base:1.0"), true);
    assert_eq!(resp_hello.ends_with(NETCONF_EOM_1_0), true);

    // 2. Get-Config (Running)
    let get_req = "<rpc message-id=\"101\" xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\"><get-config><source><running/></source></get-config></rpc>]]>]]>";
    let resp_get = server.handle_request(get_req);
    assert_eq!(resp_get.contains("<rpc-reply message-id=\"101\""), true);
    assert_eq!(resp_get.contains("<name>eth0</name>"), true);

    // 3. Edit-Config (Candidate)
    let edit_req = "<rpc message-id=\"102\" xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\"><edit-config><target><candidate/></target><config><interfaces><interface><name>eth1</name><ipv4>10.0.0.1/24</ipv4></interface></interfaces></config></edit-config></rpc>]]>]]>";
    let resp_edit = server.handle_request(edit_req);
    assert_eq!(resp_edit.contains("<ok/>"), true);

    // 4. Commit
    let commit_req = "<rpc message-id=\"103\" xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\"><commit/></rpc>]]>]]>";
    let resp_commit = server.handle_request(commit_req);
    assert_eq!(resp_commit.contains("<ok/>"), true);

    // Verify candidate config committed into running
    let get_running = "<rpc message-id=\"104\" xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\"><get-config><source><running/></source></get-config></rpc>]]>]]>";
    let resp_running = server.handle_request(get_running);
    assert_eq!(resp_running.contains("<name>eth1</name>"), true);

    assert_eq!(NETCONF_PORT, 830);
}
