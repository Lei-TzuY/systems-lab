//! Comprehensive Integration Tests for O-RAN WG4 ALD & RET Management Engine.

use toy_tcpip::oran_ald_mgmt::*;

#[test]
fn test_oran_ald_hdlc_framing_and_crc16() {
    // 1. Basic Framing & Deframing
    let addr = 0x01;
    let ctrl = 0x03; // UI-frame
    let payload = vec![0x06, 0xAA, 0xBB];

    let frame = OranAldManager::hdlc_frame(addr, ctrl, &payload);
    assert_eq!(frame[0], HDLC_FLAG);
    assert_eq!(frame[frame.len() - 1], HDLC_FLAG);

    let (dec_addr, dec_ctrl, dec_payload) =
        OranAldManager::hdlc_deframe(&frame).expect("HDLC deframe should succeed");
    assert_eq!(dec_addr, addr);
    assert_eq!(dec_ctrl, ctrl);
    assert_eq!(dec_payload, payload);

    // 2. Byte Stuffing Test (payload containing 0x7E and 0x7D)
    let tricky_payload = vec![0x10, HDLC_FLAG, 0x20, HDLC_ESCAPE, 0x30];
    let stuffed_frame = OranAldManager::hdlc_frame(addr, ctrl, &tricky_payload);

    // Ensure raw flag 0x7E does NOT appear inside frame body
    for &b in &stuffed_frame[1..stuffed_frame.len() - 1] {
        assert_ne!(b, HDLC_FLAG);
    }

    let (_, _, unstuffed) =
        OranAldManager::hdlc_deframe(&stuffed_frame).expect("Unstuffing should succeed");
    assert_eq!(unstuffed, tricky_payload);

    // 3. CRC Error Detection
    let mut corrupted = stuffed_frame.clone();
    let mid = corrupted.len() - 3;
    corrupted[mid] ^= 0xFF; // Invert bits
    let err = OranAldManager::hdlc_deframe(&corrupted);
    assert!(err.is_err());
}

#[test]
fn test_oran_ald_scan_and_device_discovery() {
    let mut mgr = OranAldManager::new();
    mgr.add_port(1);

    let ret1 = AldDevice::new_ret("RET_ANT100", 0x01, 0, 120);
    let mut tma1 = AldDevice::new_ret("TMA_BAND7", 0x02, 0, 0);
    tma1.device_type = AldDeviceType::Tma;

    mgr.add_device_to_port(1, ret1).unwrap();
    mgr.add_device_to_port(1, tma1).unwrap();

    // Duplicate address check
    let dup_ret = AldDevice::new_ret("RET_DUP", 0x01, 0, 100);
    assert!(mgr.add_device_to_port(1, dup_ret).is_err());

    // Execute ald-scan RPC
    let scan_results = mgr.ald_scan(1).expect("ALD scan should succeed");
    assert_eq!(scan_results.len(), 2);
    assert!(scan_results.contains(&"RET_ANT100".to_string()));
    assert!(scan_results.contains(&"TMA_BAND7".to_string()));

    // Test port power disabled
    mgr.ports.get_mut(&1).unwrap().dc_power_enabled = false;
    let fail_scan = mgr.ald_scan(1);
    assert!(fail_scan.is_err());
}

#[test]
fn test_oran_ald_ret_set_and_get_tilt() {
    let mut mgr = OranAldManager::new();
    mgr.add_port(1);

    let ret = AldDevice::new_ret("RET_SEC1", 0x05, 0, 100); // 0.0 to 10.0 deg
    mgr.add_device_to_port(1, ret).unwrap();

    // Calibrate RET first
    mgr.calibrate_ret(1, 0x05)
        .expect("Calibration should succeed");

    // Initial tilt is min_tilt (0 tenths)
    let initial_tilt = mgr.get_tilt(1, 0x05).unwrap();
    assert_eq!(initial_tilt, 0);

    // Set tilt to 6.5 degrees (65 tenths)
    let target = 65;
    let res = mgr
        .set_tilt(1, 0x05, target)
        .expect("SetTilt should succeed");
    assert_eq!(res, target);

    // Verify tilt readback
    let current = mgr.get_tilt(1, 0x05).unwrap();
    assert_eq!(current, 65);

    // Set tilt exceeding max limit (110 tenths > 100 max) -> should fail!
    let out_of_range = mgr.set_tilt(1, 0x05, 110);
    assert!(out_of_range.is_err());

    // Tilt remains at 65
    assert_eq!(mgr.get_tilt(1, 0x05).unwrap(), 65);
}

#[test]
fn test_oran_ald_ret_calibration_and_jam_alarm() {
    let mut mgr = OranAldManager::new();
    mgr.add_port(1);

    let ret = AldDevice::new_ret("RET_JAMMED", 0x07, 10, 90);
    mgr.add_device_to_port(1, ret).unwrap();

    // 1. Uncalibrated operations fail
    assert!(mgr.get_tilt(1, 0x07).is_err());
    assert!(mgr.set_tilt(1, 0x07, 50).is_err());

    // 2. Inject Motor Jammed alarm (Bit 0)
    let port = mgr.ports.get_mut(&1).unwrap();
    let dev = port
        .devices
        .iter_mut()
        .find(|d| d.bus_address == 0x07)
        .unwrap();
    dev.alarm_flags |= 0x01; // Motor jammed

    // Calibration fails due to motor jam!
    assert!(mgr.calibrate_ret(1, 0x07).is_err());

    // 3. Clear active alarms via HDLC
    let clear_pdu = vec![AisgProcedureCode::ClearActiveAlarms.as_u8()];
    let clear_req = OranAldManager::hdlc_frame(0x07, 0x03, &clear_pdu);
    let clear_resp = mgr.ald_communication(1, &clear_req).unwrap();
    let (_, _, resp_info) = OranAldManager::hdlc_deframe(&clear_resp).unwrap();
    assert_eq!(resp_info[1], AisgReturnCode::Ok.as_u8());

    // 4. Calibration now succeeds
    mgr.calibrate_ret(1, 0x07)
        .expect("Calibration should succeed after clearing jam");
    assert_eq!(mgr.get_tilt(1, 0x07).unwrap(), 10);
}

#[test]
fn test_oran_ald_mplane_rpc_communication() {
    let mut mgr = OranAldManager::new();
    mgr.add_port(2);

    let ret = AldDevice::new_ret("RET_COMM_TEST", 0x09, 0, 140);
    mgr.add_device_to_port(2, ret).unwrap();

    // 1. GetDeviceData (0x06) RPC
    let dev_data_pdu = vec![AisgProcedureCode::GetDeviceData.as_u8()];
    let req_frame = OranAldManager::hdlc_frame(0x09, 0x03, &dev_data_pdu);

    let resp_bytes = mgr
        .ald_communication(2, &req_frame)
        .expect("ald-communication RPC should succeed");
    let (addr, _, info) = OranAldManager::hdlc_deframe(&resp_bytes).unwrap();
    assert_eq!(addr, 0x09);
    assert_eq!(info[0], AisgProcedureCode::GetDeviceData.as_u8());
    assert_eq!(info[1], AisgReturnCode::Ok.as_u8());

    // info[2] is DeviceType (SingleRet = 0x01)
    assert_eq!(info[2], AldDeviceType::SingleRet.as_u8());
    // Max tilt = 140
    let max_tilt = ((info[5] as u16) << 8) | (info[6] as u16);
    assert_eq!(max_tilt, 140);

    // 2. SelfTest (0x0E) RPC
    let selftest_pdu = vec![AisgProcedureCode::SelfTest.as_u8()];
    let st_req = OranAldManager::hdlc_frame(0x09, 0x03, &selftest_pdu);
    let st_resp = mgr.ald_communication(2, &st_req).unwrap();
    let (_, _, st_info) = OranAldManager::hdlc_deframe(&st_resp).unwrap();
    assert_eq!(st_info[0], AisgProcedureCode::SelfTest.as_u8());
    assert_eq!(st_info[1], AisgReturnCode::Ok.as_u8());
    assert_eq!(st_info[2], 0x00); // Pass
}
