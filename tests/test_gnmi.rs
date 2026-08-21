use toy_tcpip::gnmi::{GnmiPath, GnmiServer, GnmiSubscriptionMode, GnmiValue, GNMI_PORT, GNMI_VERSION};

#[test]
fn test_gnmi_path_parsing_and_conversion() {
    let path = GnmiPath::parse("/interfaces/interface[name=HundredGigE0/1]/state/counters/in-octets");
    assert_eq!(path.elements.len(), 5);
    assert_eq!(path.elements[1], "interface[name=HundredGigE0/1]");
    assert_eq!(path.to_string_path(), "/interfaces/interface[name=HundredGigE0/1]/state/counters/in-octets");
}

#[test]
fn test_gnmi_get_and_set_datastore() {
    let mut server = GnmiServer::new();
    server.set_value("/system/config/motd", GnmiValue::StringVal("Welcome to Router Core".to_string()));

    let updates = server.get("/system/config/motd");
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].val, GnmiValue::StringVal("Welcome to Router Core".to_string()));
}

#[test]
fn test_gnmi_subscribe_mode() {
    let server = GnmiServer::new();
    let (mode, updates) = server.handle_subscribe("/system/state/hostname", GnmiSubscriptionMode::Stream);
    assert_eq!(mode, GnmiSubscriptionMode::Stream);
    assert_eq!(updates.len(), 1);
}

#[test]
fn test_gnmi_constants() {
    assert_eq!(GNMI_PORT, 9339);
    assert_eq!(GNMI_VERSION, "0.7.0");
}
