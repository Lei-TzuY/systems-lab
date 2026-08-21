use std::cmp::Ordering;
use toy_tcpip::ptp_telecom::{
    ETHERTYPE_PTP_TELECOM, PTP_TELECOM_DEFAULT_LOCAL_PRIORITY, TelecomBmcaAttributes,
    TelecomClockType, TelecomProfileEngine,
};

#[test]
fn test_ptp_telecom_constants_and_bmca_ordering() {
    assert_eq!(ETHERTYPE_PTP_TELECOM, 0x88F7);
    assert_eq!(PTP_TELECOM_DEFAULT_LOCAL_PRIORITY, 128);

    let prtc_gm = TelecomBmcaAttributes::new_prtc_grandmaster([1; 8]);
    let slave = TelecomBmcaAttributes::new_slave_clock([2; 8]);

    assert_eq!(prtc_gm.compare_telecom_bmca(&slave), Ordering::Less); // PRTC wins
}

#[test]
fn test_ptp_telecom_profile_engine_lifecycle() {
    let slave_attr = TelecomBmcaAttributes::new_slave_clock([0xAA; 8]);
    let mut engine = TelecomProfileEngine::new(TelecomClockType::TelecomTimeSlaveClock, slave_attr);

    let gm = TelecomBmcaAttributes::new_prtc_grandmaster([0x11; 8]);
    let elected = engine.process_announce(gm.clone());

    assert!(elected);
    assert_eq!(engine.best_master, Some(gm));

    // Lower-quality clock arrives, should not replace current GM
    let degraded = TelecomBmcaAttributes {
        clock_class: 140, // degraded holdover
        clock_accuracy: 0x30,
        offset_scaled_log_variance: 0xFFFF,
        priority1: 128,
        priority2: 128,
        local_priority: 200,
        clock_identity: [0x99; 8],
        steps_removed: 2,
    };
    assert!(!engine.process_announce(degraded));
}
