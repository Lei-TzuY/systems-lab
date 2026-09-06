use std::io::{self, Read};

use filesystem_lab::directory_codec::decode_directory_entry;

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
    }
    out
}

fn canonical_error(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::UnexpectedEof => "err|unexpected-eof",
        io::ErrorKind::InvalidData => "err|invalid-data",
        io::ErrorKind::InvalidInput => "err|invalid-input",
        _ => "err|other",
    }
}

fn main() -> io::Result<()> {
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input)?;

    match decode_directory_entry(&input) {
        Ok(entry) => println!(
            "ok|{}|{}|{}",
            entry.parent,
            entry.target,
            hex(entry.name.as_bytes())
        ),
        Err(error) => println!("{}", canonical_error(&error)),
    }
    Ok(())
}
