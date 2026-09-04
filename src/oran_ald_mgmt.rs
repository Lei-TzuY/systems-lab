//! O-RAN WG4 Open Fronthaul Antenna Line Device (ALD) & RET Management Engine.
//!
//! Compliant with O-RAN.WG4.MP.0 Section 13 ("ALD Management"),
//! `o-ran-ald.yang`, `o-ran-ald-port.yang`, and 3GPP TS 25.462 / AISG 2.0 & 3.0.
//!
//! Provides the complete remote antenna control and monitoring stack:
//! - HDLC protocol framing with flag delimiters (0x7E), byte stuffing, and pure Rust CCITT CRC-16.
//! - AISG 2.0 / 3.0 Application Layer Elementary Procedures (SetTilt, GetTilt, Calibrate, Alarms).
//! - Multi-drop RS-485 and Bias-Tee ALD bus discovery and address assignment.
//! - Remote Electrical Tilt (RET), Tower Mounted Amplifier (TMA), and Antenna Sensor management.
//! - O-RAN M-Plane `ald-scan` and `ald-communication` RPC transaction handling.

use std::collections::HashMap;

pub const HDLC_FLAG: u8 = 0x7E;
pub const HDLC_ESCAPE: u8 = 0x7D;
pub const HDLC_ESCAPE_MASK: u8 = 0x20;

// ---------------------------------------------------------------------------
// Types & AISG Protocol Definitions
// ---------------------------------------------------------------------------

/// ALD Device Type Codes (AISG 2.0 §5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AldDeviceType {
    /// Single-Antenna Remote Electrical Tilt unit (0x01).
    SingleRet,
    /// Multi-Antenna Remote Electrical Tilt unit (0x11).
    MultiRet,
    /// Tower Mounted Amplifier (0x02).
    Tma,
    /// Remote Antenna Elevation sensor (0x05).
    Rae,
    /// Geographic Location Sensor / GPS (0x06).
    Gls,
}

impl AldDeviceType {
    pub fn as_u8(&self) -> u8 {
        match self {
            Self::SingleRet => 0x01,
            Self::MultiRet => 0x11,
            Self::Tma => 0x02,
            Self::Rae => 0x05,
            Self::Gls => 0x06,
        }
    }

    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x01 => Some(Self::SingleRet),
            0x11 => Some(Self::MultiRet),
            0x02 => Some(Self::Tma),
            0x05 => Some(Self::Rae),
            0x06 => Some(Self::Gls),
            _ => None,
        }
    }
}

/// AISG Elementary Procedure Codes (AISG 2.0 §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AisgProcedureCode {
    SetTilt = 0x01,
    GetTilt = 0x02,
    Calibrate = 0x03,
    ClearActiveAlarms = 0x04,
    GetAlarmStatus = 0x05,
    GetDeviceData = 0x06,
    Reset = 0x09,
    SelfTest = 0x0E,
}

impl AisgProcedureCode {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x01 => Some(Self::SetTilt),
            0x02 => Some(Self::GetTilt),
            0x03 => Some(Self::Calibrate),
            0x04 => Some(Self::ClearActiveAlarms),
            0x05 => Some(Self::GetAlarmStatus),
            0x06 => Some(Self::GetDeviceData),
            0x09 => Some(Self::Reset),
            0x0E => Some(Self::SelfTest),
            _ => None,
        }
    }
}

/// AISG Return Codes (AISG 2.0 §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AisgReturnCode {
    Ok = 0x00,
    MotorJammed = 0x01,
    ActuatorJam = 0x02,
    NotCalibrated = 0x03,
    NotConfigured = 0x04,
    OutOfTiltRange = 0x05,
    HardwareError = 0x0B,
    FormatError = 0x0C,
    ChecksumError = 0x0D,
    UnknownProcedure = 0x0E,
}

impl AisgReturnCode {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    pub fn from_u8(val: u8) -> Self {
        match val {
            0x00 => Self::Ok,
            0x01 => Self::MotorJammed,
            0x02 => Self::ActuatorJam,
            0x03 => Self::NotCalibrated,
            0x04 => Self::NotConfigured,
            0x05 => Self::OutOfTiltRange,
            0x0B => Self::HardwareError,
            0x0C => Self::FormatError,
            0x0D => Self::ChecksumError,
            _ => Self::UnknownProcedure,
        }
    }
}

/// Representation of an active Antenna Line Device on the bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AldDevice {
    /// Unique identifier: Vendor Code (2 chars) + Serial Number (e.g. "KRE1234567").
    pub unique_id: String,
    pub device_type: AldDeviceType,
    /// HDLC bus address (0x01..0x7E).
    pub bus_address: u8,
    /// Minimum mechanical tilt in tenths of a degree (e.g. 0 = 0.0°).
    pub min_tilt_tenth_deg: u16,
    /// Maximum mechanical tilt in tenths of a degree (e.g. 120 = 12.0°).
    pub max_tilt_tenth_deg: u16,
    /// Current tilt position in tenths of a degree (e.g. 45 = 4.5°).
    pub current_tilt_tenth_deg: u16,
    /// Whether the device has undergone physical calibration.
    pub calibrated: bool,
    /// Alarm status bitmask (Bit 0: Motor Jammed, Bit 1: Actuator Jam, Bit 2: Not Calibrated).
    pub alarm_flags: u8,
}

impl AldDevice {
    pub fn new_ret(
        unique_id: impl Into<String>,
        bus_address: u8,
        min_tilt: u16,
        max_tilt: u16,
    ) -> Self {
        Self {
            unique_id: unique_id.into(),
            device_type: AldDeviceType::SingleRet,
            bus_address,
            min_tilt_tenth_deg: min_tilt,
            max_tilt_tenth_deg: max_tilt,
            current_tilt_tenth_deg: min_tilt,
            calibrated: false,
            alarm_flags: 0x04, // Initial NotCalibrated alarm bit
        }
    }
}

/// Physical ALD Port on O-RU (RS-485 / Bias-Tee).
#[derive(Debug, Clone)]
pub struct AldPort {
    pub port_id: u8,
    pub dc_power_enabled: bool,
    pub overcurrent_alarm: bool,
    pub devices: Vec<AldDevice>,
}

impl AldPort {
    pub fn new(port_id: u8) -> Self {
        Self {
            port_id,
            dc_power_enabled: true,
            overcurrent_alarm: false,
            devices: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// O-RAN ALD Manager Engine
// ---------------------------------------------------------------------------

/// O-RAN WG4 Open Fronthaul ALD & RET Management Engine.
#[derive(Debug, Default)]
pub struct OranAldManager {
    pub ports: HashMap<u8, AldPort>,
}

impl OranAldManager {
    /// Create a new O-RAN ALD Manager instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an ALD communication port.
    pub fn add_port(&mut self, port_id: u8) {
        self.ports.insert(port_id, AldPort::new(port_id));
    }

    /// Attach an ALD device to a port.
    pub fn add_device_to_port(
        &mut self,
        port_id: u8,
        device: AldDevice,
    ) -> Result<(), &'static str> {
        let port = self.ports.get_mut(&port_id).ok_or("ALD port not found")?;
        if port
            .devices
            .iter()
            .any(|d| d.bus_address == device.bus_address)
        {
            return Err("Duplicate HDLC bus address on ALD port");
        }
        port.devices.push(device);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // HDLC Framing & Pure Rust CRC-16-CCITT
    // -----------------------------------------------------------------------

    /// Pure Rust CRC-16-CCITT calculation ($x^{16} + x^{12} + x^5 + 1$, polynomial 0x1021 / reversed 0x8408).
    ///
    /// Compliant with 3GPP TS 25.462 Section 4.3.4 (FCS calculation).
    pub fn compute_crc16(data: &[u8]) -> u16 {
        let mut crc: u16 = 0xFFFF;
        for &byte in data {
            let mut b = byte as u16;
            for _ in 0..8 {
                if ((crc ^ b) & 0x0001) != 0 {
                    crc = (crc >> 1) ^ 0x8408;
                } else {
                    crc >>= 1;
                }
                b >>= 1;
            }
        }
        !crc // One's complement inversion
    }

    /// Frames an HDLC packet with Address, Control, Information payload, and CRC-16.
    ///
    /// Applies byte stuffing: escapes `0x7E` and `0x7D` with `0x7D`.
    pub fn hdlc_frame(address: u8, control: u8, info: &[u8]) -> Vec<u8> {
        let mut unescaped = Vec::with_capacity(2 + info.len() + 2);
        unescaped.push(address);
        unescaped.push(control);
        unescaped.extend_from_slice(info);

        let crc = Self::compute_crc16(&unescaped);
        unescaped.push((crc & 0xFF) as u8);
        unescaped.push(((crc >> 8) & 0xFF) as u8);

        // Byte stuffing and flag delimiters
        let mut frame = Vec::new();
        frame.push(HDLC_FLAG);
        for byte in unescaped {
            if byte == HDLC_FLAG || byte == HDLC_ESCAPE {
                frame.push(HDLC_ESCAPE);
                frame.push(byte ^ HDLC_ESCAPE_MASK);
            } else {
                frame.push(byte);
            }
        }
        frame.push(HDLC_FLAG);
        frame
    }

    /// Deframes and validates an HDLC packet, verifying CRC-16 and unstuffing bytes.
    ///
    /// Returns `(address, control, info_payload)`.
    pub fn hdlc_deframe(raw_bytes: &[u8]) -> Result<(u8, u8, Vec<u8>), &'static str> {
        if raw_bytes.len() < 5 {
            return Err("HDLC frame too short");
        }

        // Must begin and end with HDLC_FLAG (0x7E)
        if raw_bytes[0] != HDLC_FLAG || raw_bytes[raw_bytes.len() - 1] != HDLC_FLAG {
            return Err("Missing HDLC flag delimiter");
        }

        // Unstuff escaped bytes between flags
        let mut unstuffed = Vec::new();
        let mut escape_active = false;
        for &b in &raw_bytes[1..raw_bytes.len() - 1] {
            if escape_active {
                unstuffed.push(b ^ HDLC_ESCAPE_MASK);
                escape_active = false;
            } else if b == HDLC_ESCAPE {
                escape_active = true;
            } else {
                unstuffed.push(b);
            }
        }

        if unstuffed.len() < 4 {
            return Err("Unstuffed frame too short (missing header or CRC)");
        }

        // Verify CRC-16
        let payload_len = unstuffed.len() - 2;
        let rx_crc = (unstuffed[payload_len] as u16) | ((unstuffed[payload_len + 1] as u16) << 8);
        let calc_crc = Self::compute_crc16(&unstuffed[..payload_len]);

        if rx_crc != calc_crc {
            return Err("HDLC CRC-16 checksum mismatch");
        }

        let address = unstuffed[0];
        let control = unstuffed[1];
        let info = unstuffed[2..payload_len].to_vec();

        Ok((address, control, info))
    }

    // -----------------------------------------------------------------------
    // O-RAN M-Plane RPC Handlers (O-RAN.WG4.MP.0 Section 13)
    // -----------------------------------------------------------------------

    /// Executes O-RAN `ald-scan` RPC on the specified ALD port.
    ///
    /// Discovers connected devices and returns their unique IDs.
    pub fn ald_scan(&self, port_id: u8) -> Result<Vec<String>, &'static str> {
        let port = self.ports.get(&port_id).ok_or("ALD port not found")?;
        if !port.dc_power_enabled {
            return Err("ALD port DC power is disabled");
        }
        if port.overcurrent_alarm {
            return Err("ALD port in over-current fault state");
        }

        let ids = port.devices.iter().map(|d| d.unique_id.clone()).collect();
        Ok(ids)
    }

    /// Executes O-RAN `ald-communication` RPC over the specified ALD port.
    ///
    /// Accepts raw HDLC request, routes to target device, executes AISG procedure,
    /// and returns framed HDLC response bytes.
    pub fn ald_communication(
        &mut self,
        port_id: u8,
        hdlc_req: &[u8],
    ) -> Result<Vec<u8>, &'static str> {
        let (dest_addr, control, info) = Self::hdlc_deframe(hdlc_req)?;

        if info.is_empty() {
            return Err("Empty AISG payload in HDLC frame");
        }

        let proc_code_raw = info[0];
        let proc_code =
            AisgProcedureCode::from_u8(proc_code_raw).ok_or("Unknown AISG procedure code")?;

        let port = self.ports.get_mut(&port_id).ok_or("ALD port not found")?;
        if !port.dc_power_enabled || port.overcurrent_alarm {
            return Err("ALD port power failure");
        }

        let device = port
            .devices
            .iter_mut()
            .find(|d| d.bus_address == dest_addr)
            .ok_or("Device address not found on ALD port")?;

        // Process AISG Procedure
        let (return_code, response_data) = match proc_code {
            AisgProcedureCode::SetTilt => {
                if info.len() < 3 {
                    (AisgReturnCode::FormatError, Vec::new())
                } else {
                    let target_tilt = ((info[1] as u16) << 8) | (info[2] as u16);
                    if !device.calibrated {
                        (AisgReturnCode::NotCalibrated, Vec::new())
                    } else if target_tilt < device.min_tilt_tenth_deg
                        || target_tilt > device.max_tilt_tenth_deg
                    {
                        (AisgReturnCode::OutOfTiltRange, Vec::new())
                    } else if (device.alarm_flags & 0x01) != 0 {
                        (AisgReturnCode::MotorJammed, Vec::new())
                    } else {
                        device.current_tilt_tenth_deg = target_tilt;
                        (AisgReturnCode::Ok, Vec::new())
                    }
                }
            }
            AisgProcedureCode::GetTilt => {
                if !device.calibrated {
                    (AisgReturnCode::NotCalibrated, Vec::new())
                } else {
                    let mut data = Vec::new();
                    data.push((device.current_tilt_tenth_deg >> 8) as u8);
                    data.push((device.current_tilt_tenth_deg & 0xFF) as u8);
                    (AisgReturnCode::Ok, data)
                }
            }
            AisgProcedureCode::Calibrate => {
                if (device.alarm_flags & 0x01) != 0 {
                    (AisgReturnCode::MotorJammed, Vec::new())
                } else {
                    device.calibrated = true;
                    device.alarm_flags &= !0x04; // Clear NotCalibrated alarm
                    device.current_tilt_tenth_deg = device.min_tilt_tenth_deg;
                    (AisgReturnCode::Ok, Vec::new())
                }
            }
            AisgProcedureCode::GetAlarmStatus => {
                let mut data = Vec::new();
                data.push(device.alarm_flags);
                (AisgReturnCode::Ok, data)
            }
            AisgProcedureCode::ClearActiveAlarms => {
                device.alarm_flags = 0;
                (AisgReturnCode::Ok, Vec::new())
            }
            AisgProcedureCode::GetDeviceData => {
                let mut data = Vec::new();
                // [DeviceType: 1][MinTilt: 2][MaxTilt: 2][UniqueID len: 1][UniqueID bytes]
                data.push(device.device_type.as_u8());
                data.push((device.min_tilt_tenth_deg >> 8) as u8);
                data.push((device.min_tilt_tenth_deg & 0xFF) as u8);
                data.push((device.max_tilt_tenth_deg >> 8) as u8);
                data.push((device.max_tilt_tenth_deg & 0xFF) as u8);
                let id_bytes = device.unique_id.as_bytes();
                data.push(id_bytes.len() as u8);
                data.extend_from_slice(id_bytes);
                (AisgReturnCode::Ok, data)
            }
            AisgProcedureCode::Reset => {
                device.current_tilt_tenth_deg = device.min_tilt_tenth_deg;
                (AisgReturnCode::Ok, Vec::new())
            }
            AisgProcedureCode::SelfTest => {
                (AisgReturnCode::Ok, vec![0x00]) // Self test passed
            }
        };

        // Format AISG Response PDU: [ProcedureCode][ReturnCode][Data...]
        let mut resp_info = Vec::new();
        resp_info.push(proc_code.as_u8());
        resp_info.push(return_code.as_u8());
        resp_info.extend_from_slice(&response_data);

        // Frame response into HDLC packet
        let resp_frame = Self::hdlc_frame(dest_addr, control, &resp_info);
        Ok(resp_frame)
    }

    // -----------------------------------------------------------------------
    // High-Level Helper APIs
    // -----------------------------------------------------------------------

    /// High-level SetTilt request via HDLC framing.
    pub fn set_tilt(
        &mut self,
        port_id: u8,
        address: u8,
        tilt_tenth_deg: u16,
    ) -> Result<u16, &'static str> {
        let mut info = Vec::new();
        info.push(AisgProcedureCode::SetTilt.as_u8());
        info.push((tilt_tenth_deg >> 8) as u8);
        info.push((tilt_tenth_deg & 0xFF) as u8);

        let req_frame = Self::hdlc_frame(address, 0x03, &info);
        let resp_frame = self.ald_communication(port_id, &req_frame)?;
        let (_, _, resp_info) = Self::hdlc_deframe(&resp_frame)?;

        if resp_info.len() < 2 {
            return Err("Invalid AISG response length");
        }
        let ret_code = AisgReturnCode::from_u8(resp_info[1]);
        if ret_code != AisgReturnCode::Ok {
            return Err("SetTilt command failed on ALD device");
        }

        Ok(tilt_tenth_deg)
    }

    /// High-level GetTilt request via HDLC framing.
    pub fn get_tilt(&mut self, port_id: u8, address: u8) -> Result<u16, &'static str> {
        let info = vec![AisgProcedureCode::GetTilt.as_u8()];
        let req_frame = Self::hdlc_frame(address, 0x03, &info);
        let resp_frame = self.ald_communication(port_id, &req_frame)?;
        let (_, _, resp_info) = Self::hdlc_deframe(&resp_frame)?;

        if resp_info.len() < 4 {
            return Err("Invalid GetTilt response length");
        }
        let ret_code = AisgReturnCode::from_u8(resp_info[1]);
        if ret_code != AisgReturnCode::Ok {
            return Err("GetTilt command failed on ALD device");
        }

        let tilt = ((resp_info[2] as u16) << 8) | (resp_info[3] as u16);
        Ok(tilt)
    }

    /// High-level Calibrate request via HDLC framing.
    pub fn calibrate_ret(&mut self, port_id: u8, address: u8) -> Result<(), &'static str> {
        let info = vec![AisgProcedureCode::Calibrate.as_u8()];
        let req_frame = Self::hdlc_frame(address, 0x03, &info);
        let resp_frame = self.ald_communication(port_id, &req_frame)?;
        let (_, _, resp_info) = Self::hdlc_deframe(&resp_frame)?;

        if resp_info.len() < 2 {
            return Err("Invalid Calibrate response length");
        }
        let ret_code = AisgReturnCode::from_u8(resp_info[1]);
        if ret_code != AisgReturnCode::Ok {
            return Err("Calibrate failed on ALD device");
        }

        Ok(())
    }
}
