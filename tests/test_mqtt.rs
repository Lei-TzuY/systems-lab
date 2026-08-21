use toy_tcpip::mqtt::{MqttBroker, MqttPacket, MqttPacketType, MQTT_PORT};

#[test]
fn test_mqtt_packet_connect_and_publish() {
    let connect = MqttPacket::build_connect("Client-007", true);
    let raw = connect.serialize();

    let parsed = MqttPacket::parse(&raw).unwrap();
    assert_eq!(parsed.packet_type, MqttPacketType::Connect);
    assert_eq!(MQTT_PORT, 1883);

    let pub_pkt = MqttPacket::build_publish("factory/node1/telemetry", b"{\"status\": \"ok\"}", 0, None);
    let raw_pub = pub_pkt.serialize();
    let parsed_pub = MqttPacket::parse(&raw_pub).unwrap();
    assert_eq!(parsed_pub.packet_type, MqttPacketType::Publish);
    assert_eq!(parsed_pub.topic.as_deref(), Some("factory/node1/telemetry"));
    assert_eq!(parsed_pub.payload[2 + "factory/node1/telemetry".len()..], *b"{\"status\": \"ok\"}");
}

#[test]
fn test_mqtt_broker_topic_fanout() {
    let mut broker = MqttBroker::new();
    broker.subscribe("iot/device/1", "ControllerApp");
    broker.subscribe("iot/device/1", "LoggerService");

    let subs = broker.publish("iot/device/1");
    assert_eq!(subs.len(), 2);
    assert!(subs.contains(&"ControllerApp".to_string()));
    assert!(subs.contains(&"LoggerService".to_string()));
}
