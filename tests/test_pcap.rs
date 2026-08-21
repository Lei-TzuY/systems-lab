use std::io::Cursor;
use toy_tcpip::pcap::{PcapError, PcapReader, PcapWriter, LINKTYPE_ETHERNET, PCAP_MAGIC_LE};

#[test]
fn test_pcap_write_and_read_multiple_packets() {
    let mut buffer = Vec::new();

    {
        let mut writer = PcapWriter::new(&mut buffer, 65535, LINKTYPE_ETHERNET).expect("writer init");
        writer.write_packet(1000, 100, b"Packet #1 content").unwrap();
        writer.write_packet(1000, 200, b"Packet #2 content - longer payload").unwrap();
        writer.write_packet(1000, 300, b"").unwrap();
    }

    let mut reader = PcapReader::new(Cursor::new(&buffer)).expect("reader init");
    assert_eq!(reader.header.magic_number, PCAP_MAGIC_LE);
    assert_eq!(reader.header.network, LINKTYPE_ETHERNET);

    let p1 = reader.next_packet().unwrap().expect("p1");
    assert_eq!(p1.ts_sec, 1000);
    assert_eq!(p1.ts_usec, 100);
    assert_eq!(p1.data, b"Packet #1 content");

    let p2 = reader.next_packet().unwrap().expect("p2");
    assert_eq!(p2.ts_sec, 1000);
    assert_eq!(p2.ts_usec, 200);
    assert_eq!(p2.data, b"Packet #2 content - longer payload");

    let p3 = reader.next_packet().unwrap().expect("p3");
    assert_eq!(p3.ts_sec, 1000);
    assert_eq!(p3.ts_usec, 300);
    assert_eq!(p3.data, b"");

    assert!(reader.next_packet().unwrap().is_none());
}

#[test]
fn test_pcap_invalid_magic() {
    let corrupt_data = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
    let res = PcapReader::new(Cursor::new(&corrupt_data));
    assert!(matches!(res.err(), Some(PcapError::InvalidMagic(_))));
}
