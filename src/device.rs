//! Network Device Driver Abstraction Layer.
//!
//! Provides the `NetDevice` trait and driver implementations for in-memory loopback,
//! offline PCAP playback/recording, and virtual OS TAP interfaces.

use crate::ethernet::MacAddress;
use crate::pcap::{LINKTYPE_ETHERNET, PcapPacket, PcapReader, PcapWriter};
use std::collections::VecDeque;
use std::fs::File;

pub trait NetDevice {
    fn name(&self) -> &str;
    fn mac_address(&self) -> MacAddress;
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), String>;
    fn receive_frame(&mut self) -> Option<Vec<u8>>;
    fn is_up(&self) -> bool;
}

/// In-memory Loopback / Virtual Cable Device
pub struct LoopbackDevice {
    pub name: String,
    pub mac: MacAddress,
    pub is_up: bool,
    pub rx_queue: VecDeque<Vec<u8>>,
    pub tx_history: Vec<Vec<u8>>,
}

impl LoopbackDevice {
    pub fn new(name: &str, mac: MacAddress) -> Self {
        LoopbackDevice {
            name: name.to_string(),
            mac,
            is_up: true,
            rx_queue: VecDeque::new(),
            tx_history: Vec::new(),
        }
    }

    pub fn inject_incoming(&mut self, frame: Vec<u8>) {
        self.rx_queue.push_back(frame);
    }
}

impl NetDevice for LoopbackDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn mac_address(&self) -> MacAddress {
        self.mac
    }

    fn send_frame(&mut self, frame: &[u8]) -> Result<(), String> {
        if !self.is_up {
            return Err("Device is down".to_string());
        }
        self.tx_history.push(frame.to_vec());
        Ok(())
    }

    fn receive_frame(&mut self) -> Option<Vec<u8>> {
        if !self.is_up {
            return None;
        }
        self.rx_queue.pop_front()
    }

    fn is_up(&self) -> bool {
        self.is_up
    }
}

/// PCAP File-Backed Device: Ingests packets from input PCAP and logs sent frames to output PCAP
pub struct PcapDevice {
    pub name: String,
    pub mac: MacAddress,
    reader: PcapReader<File>,
    writer: Option<PcapWriter<File>>,
    packet_counter: u32,
}

impl PcapDevice {
    pub fn new(
        name: &str,
        mac: MacAddress,
        input_file: File,
        output_file: Option<File>,
    ) -> Result<Self, String> {
        let reader = PcapReader::new(input_file).map_err(|e| e.to_string())?;
        let writer = match output_file {
            Some(f) => {
                Some(PcapWriter::new(f, 65535, LINKTYPE_ETHERNET).map_err(|e| e.to_string())?)
            }
            None => None,
        };

        Ok(PcapDevice {
            name: name.to_string(),
            mac,
            reader,
            writer,
            packet_counter: 0,
        })
    }
}

impl NetDevice for PcapDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn mac_address(&self) -> MacAddress {
        self.mac
    }

    fn send_frame(&mut self, frame: &[u8]) -> Result<(), String> {
        self.packet_counter += 1;
        if let Some(ref mut writer) = self.writer {
            writer
                .write_packet(1700000000, self.packet_counter * 1000, frame)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn receive_frame(&mut self) -> Option<Vec<u8>> {
        match self.reader.next_packet() {
            Ok(Some(PcapPacket { data, .. })) => Some(data),
            _ => None,
        }
    }

    fn is_up(&self) -> bool {
        true
    }
}

/// Virtual TAP Interface Driver
pub struct VirtualTapDevice {
    pub name: String,
    pub mac: MacAddress,
    pub mtu: usize,
    pub is_up: bool,
    tx_queue: VecDeque<Vec<u8>>,
    rx_queue: VecDeque<Vec<u8>>,
}

impl VirtualTapDevice {
    pub fn new(name: &str, mac: MacAddress, mtu: usize) -> Self {
        VirtualTapDevice {
            name: name.to_string(),
            mac,
            mtu,
            is_up: true,
            tx_queue: VecDeque::new(),
            rx_queue: VecDeque::new(),
        }
    }

    pub fn push_rx(&mut self, frame: Vec<u8>) {
        self.rx_queue.push_back(frame);
    }

    pub fn pop_tx(&mut self) -> Option<Vec<u8>> {
        self.tx_queue.pop_front()
    }
}

impl NetDevice for VirtualTapDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn mac_address(&self) -> MacAddress {
        self.mac
    }

    fn send_frame(&mut self, frame: &[u8]) -> Result<(), String> {
        if !self.is_up {
            return Err("TAP device is down".to_string());
        }
        if frame.len() > self.mtu + 14 {
            return Err(format!("Frame exceeds MTU ({})", self.mtu));
        }
        self.tx_queue.push_back(frame.to_vec());
        Ok(())
    }

    fn receive_frame(&mut self) -> Option<Vec<u8>> {
        if !self.is_up {
            return None;
        }
        self.rx_queue.pop_front()
    }

    fn is_up(&self) -> bool {
        self.is_up
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loopback_device_send_and_receive() {
        let mac = MacAddress([0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc]);
        let mut dev = LoopbackDevice::new("lo0", mac);

        assert_eq!(dev.name(), "lo0");
        assert_eq!(dev.mac_address(), mac);
        assert!(dev.is_up());

        dev.inject_incoming(vec![1, 2, 3, 4]);
        let rx = dev.receive_frame().unwrap();
        assert_eq!(rx, vec![1, 2, 3, 4]);
        assert_eq!(dev.receive_frame(), None);

        dev.send_frame(&[5, 6, 7, 8]).unwrap();
        assert_eq!(dev.tx_history.len(), 1);
        assert_eq!(dev.tx_history[0], vec![5, 6, 7, 8]);
    }
}
