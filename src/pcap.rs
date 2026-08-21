//! PCAP file reader and writer for offline packet analysis and replay.
//!
//! Implements parsing for standard Libpcap file formats with automatic
//! endianness detection (magic numbers `0xa1b2c3d4` and `0xd4c3b2a1`).

use std::io::{self, Read, Write};

pub const PCAP_MAGIC_LE: u32 = 0xa1b2c3d4;
pub const PCAP_MAGIC_BE: u32 = 0xd4c3b2a1;
pub const LINKTYPE_ETHERNET: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcapGlobalHeader {
    pub magic_number: u32,
    pub version_major: u16,
    pub version_minor: u16,
    pub thiszone: i32,
    pub sigfigs: u32,
    pub snaplen: u32,
    pub network: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcapPacket {
    pub ts_sec: u32,
    pub ts_usec: u32,
    pub incl_len: u32,
    pub orig_len: u32,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub enum PcapError {
    Io(io::Error),
    InvalidMagic(u32),
    UnsupportedLinkType(u32),
    UnexpectedEof,
    CorruptPacket(String),
}

impl From<io::Error> for PcapError {
    fn from(err: io::Error) -> Self {
        PcapError::Io(err)
    }
}

impl std::fmt::Display for PcapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PcapError::Io(e) => write!(f, "I/O error: {}", e),
            PcapError::InvalidMagic(m) => write!(f, "Invalid PCAP magic number: 0x{:08x}", m),
            PcapError::UnsupportedLinkType(lt) => write!(f, "Unsupported link type: {}", lt),
            PcapError::UnexpectedEof => write!(f, "Unexpected end of file"),
            PcapError::CorruptPacket(msg) => write!(f, "Corrupt packet header: {}", msg),
        }
    }
}

impl std::error::Error for PcapError {}

pub struct PcapReader<R> {
    reader: R,
    pub header: PcapGlobalHeader,
    is_big_endian: bool,
}

impl<R: Read> PcapReader<R> {
    pub fn new(mut reader: R) -> Result<Self, PcapError> {
        let mut magic_buf = [0u8; 4];
        reader.read_exact(&mut magic_buf).map_err(|e| {
            if e.kind() == io::ErrorKind::UnexpectedEof {
                PcapError::UnexpectedEof
            } else {
                PcapError::Io(e)
            }
        })?;

        let magic_raw = u32::from_le_bytes(magic_buf);
        let (is_big_endian, magic) = match magic_raw {
            PCAP_MAGIC_LE => (false, PCAP_MAGIC_LE),
            PCAP_MAGIC_BE => (true, PCAP_MAGIC_BE),
            other => return Err(PcapError::InvalidMagic(other)),
        };

        let mut gh_buf = [0u8; 20];
        reader.read_exact(&mut gh_buf)?;

        let (version_major, version_minor, thiszone, sigfigs, snaplen, network) = if is_big_endian {
            (
                u16::from_be_bytes([gh_buf[0], gh_buf[1]]),
                u16::from_be_bytes([gh_buf[2], gh_buf[3]]),
                i32::from_be_bytes([gh_buf[4], gh_buf[5], gh_buf[6], gh_buf[7]]),
                u32::from_be_bytes([gh_buf[8], gh_buf[9], gh_buf[10], gh_buf[11]]),
                u32::from_be_bytes([gh_buf[12], gh_buf[13], gh_buf[14], gh_buf[15]]),
                u32::from_be_bytes([gh_buf[16], gh_buf[17], gh_buf[18], gh_buf[19]]),
            )
        } else {
            (
                u16::from_le_bytes([gh_buf[0], gh_buf[1]]),
                u16::from_le_bytes([gh_buf[2], gh_buf[3]]),
                i32::from_le_bytes([gh_buf[4], gh_buf[5], gh_buf[6], gh_buf[7]]),
                u32::from_le_bytes([gh_buf[8], gh_buf[9], gh_buf[10], gh_buf[11]]),
                u32::from_le_bytes([gh_buf[12], gh_buf[13], gh_buf[14], gh_buf[15]]),
                u32::from_le_bytes([gh_buf[16], gh_buf[17], gh_buf[18], gh_buf[19]]),
            )
        };

        let header = PcapGlobalHeader {
            magic_number: magic,
            version_major,
            version_minor,
            thiszone,
            sigfigs,
            snaplen,
            network,
        };

        Ok(PcapReader {
            reader,
            header,
            is_big_endian,
        })
    }

    pub fn next_packet(&mut self) -> Result<Option<PcapPacket>, PcapError> {
        let mut rec_hdr = [0u8; 16];
        match self.reader.read_exact(&mut rec_hdr) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(PcapError::Io(e)),
        }

        let (ts_sec, ts_usec, incl_len, orig_len) = if self.is_big_endian {
            (
                u32::from_be_bytes([rec_hdr[0], rec_hdr[1], rec_hdr[2], rec_hdr[3]]),
                u32::from_be_bytes([rec_hdr[4], rec_hdr[5], rec_hdr[6], rec_hdr[7]]),
                u32::from_be_bytes([rec_hdr[8], rec_hdr[9], rec_hdr[10], rec_hdr[11]]),
                u32::from_be_bytes([rec_hdr[12], rec_hdr[13], rec_hdr[14], rec_hdr[15]]),
            )
        } else {
            (
                u32::from_le_bytes([rec_hdr[0], rec_hdr[1], rec_hdr[2], rec_hdr[3]]),
                u32::from_le_bytes([rec_hdr[4], rec_hdr[5], rec_hdr[6], rec_hdr[7]]),
                u32::from_le_bytes([rec_hdr[8], rec_hdr[9], rec_hdr[10], rec_hdr[11]]),
                u32::from_le_bytes([rec_hdr[12], rec_hdr[13], rec_hdr[14], rec_hdr[15]]),
            )
        };

        if incl_len > 65535 * 4 {
            return Err(PcapError::CorruptPacket(format!("incl_len too large: {}", incl_len)));
        }

        let mut data = vec![0u8; incl_len as usize];
        self.reader.read_exact(&mut data)?;

        Ok(Some(PcapPacket {
            ts_sec,
            ts_usec,
            incl_len,
            orig_len,
            data,
        }))
    }

    pub fn read_all_packets(&mut self) -> Result<Vec<PcapPacket>, PcapError> {
        let mut packets = Vec::new();
        while let Some(pkt) = self.next_packet()? {
            packets.push(pkt);
        }
        Ok(packets)
    }
}

pub struct PcapWriter<W> {
    writer: W,
}

impl<W: Write> PcapWriter<W> {
    pub fn new(mut writer: W, snaplen: u32, network: u32) -> Result<Self, PcapError> {
        // Write standard little-endian PCAP global header
        writer.write_all(&PCAP_MAGIC_LE.to_le_bytes())?;
        writer.write_all(&2u16.to_le_bytes())?; // version_major = 2
        writer.write_all(&4u16.to_le_bytes())?; // version_minor = 4
        writer.write_all(&0i32.to_le_bytes())?; // thiszone = 0
        writer.write_all(&0u32.to_le_bytes())?; // sigfigs = 0
        writer.write_all(&snaplen.to_le_bytes())?;
        writer.write_all(&network.to_le_bytes())?;
        writer.flush()?;

        Ok(PcapWriter { writer })
    }

    pub fn write_packet(&mut self, ts_sec: u32, ts_usec: u32, data: &[u8]) -> Result<(), PcapError> {
        let len = data.len() as u32;
        self.writer.write_all(&ts_sec.to_le_bytes())?;
        self.writer.write_all(&ts_usec.to_le_bytes())?;
        self.writer.write_all(&len.to_le_bytes())?;
        self.writer.write_all(&len.to_le_bytes())?;
        self.writer.write_all(data)?;
        self.writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pcap_roundtrip() {
        let mut buffer = Vec::new();
        {
            let mut writer = PcapWriter::new(&mut buffer, 65535, LINKTYPE_ETHERNET).unwrap();
            writer.write_packet(100, 50, &[1, 2, 3, 4, 5]).unwrap();
            writer.write_packet(100, 60, &[6, 7, 8]).unwrap();
        }

        let mut reader = PcapReader::new(&buffer[..]).unwrap();
        assert_eq!(reader.header.network, LINKTYPE_ETHERNET);
        assert_eq!(reader.header.snaplen, 65535);

        let p1 = reader.next_packet().unwrap().unwrap();
        assert_eq!(p1.ts_sec, 100);
        assert_eq!(p1.ts_usec, 50);
        assert_eq!(p1.data, vec![1, 2, 3, 4, 5]);

        let p2 = reader.next_packet().unwrap().unwrap();
        assert_eq!(p2.ts_sec, 100);
        assert_eq!(p2.ts_usec, 60);
        assert_eq!(p2.data, vec![6, 7, 8]);

        let p3 = reader.next_packet().unwrap();
        assert!(p3.is_none());
    }
}
