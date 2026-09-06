use std::io::{self, Read};
use toy_tcpip::tftp::{TftpError, TftpPacket};

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
    }
    out
}

fn field_token(field: &'static str) -> &'static str {
    match field {
        "error message" => "error-message",
        other => other,
    }
}

fn canonical_packet(packet: TftpPacket) -> String {
    match packet {
        TftpPacket::Rrq { filename, mode } => {
            format!("ok|rrq|{}|{}", hex(filename.as_bytes()), hex(mode.as_bytes()))
        }
        TftpPacket::Wrq { filename, mode } => {
            format!("ok|wrq|{}|{}", hex(filename.as_bytes()), hex(mode.as_bytes()))
        }
        TftpPacket::Data { block_num, data } => {
            format!("ok|data|{block_num}|{}", hex(&data))
        }
        TftpPacket::Ack { block_num } => format!("ok|ack|{block_num}"),
        TftpPacket::Error {
            error_code,
            message,
        } => format!("ok|error|{error_code}|{}", hex(message.as_bytes())),
    }
}

fn canonical_error(error: TftpError) -> String {
    match error {
        TftpError::PacketTooShort(length) => format!("err|packet-too-short|{length}"),
        TftpError::InvalidOpcode(opcode) => format!("err|invalid-opcode|{opcode}"),
        TftpError::MissingNullTerminator => "err|missing-null".to_string(),
        TftpError::InvalidUtf8(field) => {
            format!("err|invalid-utf8|{}", field_token(field))
        }
        TftpError::InvalidMode(mode) => format!("err|invalid-mode|{}", hex(mode.as_bytes())),
        TftpError::EmptyField(field) => format!("err|empty-field|{}", field_token(field)),
        TftpError::EmbeddedNull(field) => {
            format!("err|embedded-null|{}", field_token(field))
        }
        TftpError::TrailingData { opcode, length } => {
            format!("err|trailing-data|{opcode}|{length}")
        }
        TftpError::InvalidPacketLength { opcode, length } => {
            format!("err|invalid-length|{opcode}|{length}")
        }
        TftpError::DataBlockTooLarge(length) => format!("err|data-too-large|{length}"),
    }
}

fn main() -> io::Result<()> {
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input)?;

    let output = match TftpPacket::parse(&input) {
        Ok(packet) => canonical_packet(packet),
        Err(error) => canonical_error(error),
    };
    println!("{output}");
    Ok(())
}
