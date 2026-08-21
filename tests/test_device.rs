use toy_tcpip::device::{LoopbackDevice, NetDevice, VirtualTapDevice};
use toy_tcpip::ethernet::MacAddress;

#[test]
fn test_virtual_tap_device_queuing_and_mtu() {
    let mac = MacAddress([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    let mut tap = VirtualTapDevice::new("tap0", mac, 1500);

    assert_eq!(tap.name(), "tap0");
    assert_eq!(tap.mac_address(), mac);
    assert!(tap.is_up());

    // Valid frame <= MTU + 14
    let valid_frame = vec![0x42; 1514];
    assert!(tap.send_frame(&valid_frame).is_ok());
    assert_eq!(tap.pop_tx(), Some(valid_frame));

    // Oversized frame > MTU + 14
    let oversized = vec![0x42; 1600];
    assert!(tap.send_frame(&oversized).is_err());

    // Receive queue
    tap.push_rx(vec![1, 2, 3]);
    assert_eq!(tap.receive_frame(), Some(vec![1, 2, 3]));
    assert_eq!(tap.receive_frame(), None);
}

#[test]
fn test_loopback_device_tx_history() {
    let mac = MacAddress([0x00, 0x01, 0x02, 0x03, 0x04, 0x05]);
    let mut dev = LoopbackDevice::new("lo", mac);

    dev.send_frame(b"packet 1").unwrap();
    dev.send_frame(b"packet 2").unwrap();

    assert_eq!(dev.tx_history.len(), 2);
    assert_eq!(dev.tx_history[0], b"packet 1");
    assert_eq!(dev.tx_history[1], b"packet 2");
}
