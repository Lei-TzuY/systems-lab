use toy_tcpip::tftp::{TftpFileServer, TftpPacket, TFTP_BLOCK_SIZE};

#[test]
fn test_tftp_packet_encoding_and_parsing() {
    let rrq = TftpPacket::Rrq {
        filename: "ubuntu.iso".to_string(),
        mode: "octet".to_string(),
    };
    let parsed_rrq = TftpPacket::parse(&rrq.serialize()).unwrap();
    assert_eq!(parsed_rrq, rrq);

    let wrq = TftpPacket::Wrq {
        filename: "upload.dat".to_string(),
        mode: "netascii".to_string(),
    };
    let parsed_wrq = TftpPacket::parse(&wrq.serialize()).unwrap();
    assert_eq!(parsed_wrq, wrq);

    let err = TftpPacket::Error {
        error_code: 2,
        message: "Access violation".to_string(),
    };
    let parsed_err = TftpPacket::parse(&err.serialize()).unwrap();
    assert_eq!(parsed_err, err);
}

#[test]
fn test_tftp_file_server_download_stream() {
    let mut server = TftpFileServer::new();
    let large_file = vec![0xfe; 1200]; // 2 x 512 + 176 bytes
    server.add_file("large.bin", large_file);

    // Block 1
    let b1 = server.handle_read_request("large.bin", 1);
    if let TftpPacket::Data { block_num, data } = b1 {
        assert_eq!(block_num, 1);
        assert_eq!(data.len(), TFTP_BLOCK_SIZE);
    } else {
        panic!("Expected Data packet");
    }

    // Block 2
    let b2 = server.handle_read_request("large.bin", 2);
    if let TftpPacket::Data { block_num, data } = b2 {
        assert_eq!(block_num, 2);
        assert_eq!(data.len(), TFTP_BLOCK_SIZE);
    } else {
        panic!("Expected Data packet");
    }

    // Block 3 (Terminal block)
    let b3 = server.handle_read_request("large.bin", 3);
    if let TftpPacket::Data { block_num, data } = b3 {
        assert_eq!(block_num, 3);
        assert_eq!(data.len(), 176); // < 512 signals EOF
    } else {
        panic!("Expected Data packet");
    }
}
