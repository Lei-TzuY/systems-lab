//! Integration tests for PTP PDV Floor Filter & Min-Delay Estimator (ITU-T G.8275.2)

use toy_tcpip::ptp_pdv_filter::{PtpPdvFloorFilter, PtpTimestampSample};

#[test]
fn test_ptp_pdv_floor_filter_in_congested_network() {
    // 50-sample window, 5% floor selection, 200 ns cluster spread
    let mut filter = PtpPdvFloorFilter::new(50, 5.0, 200);

    // True physical delay = 25,000 ns (25 µs)
    // True clock offset = -1,200 ns
    // Base forward delay: 25,000 - 1,200 = 23,800 ns
    // Base reverse delay: 25,000 + 1,200 = 26,200 ns

    // 50 sync/delay_resp cycles with heavy asymmetric packet queuing bursts
    for seq in 0..50 {
        // Congested packets experience +10,000 to +150,000 ns random queuing delay
        // Clean floor packets occur periodically
        let is_clean = (seq % 10) == 0;
        let fwd_pdv = if is_clean { 0 } else { ((seq as i64 * 37) % 150) * 1_000 };
        let rev_pdv = if is_clean { 0 } else { ((seq as i64 * 53) % 200) * 1_000 };

        let t1 = (seq as i64) * 10_000_000;
        let t2 = t1 + 23_800 + fwd_pdv;
        let t3 = t2 + 50_000;
        let t4 = t3 + 26_200 + rev_pdv;

        filter.push_sample(PtpTimestampSample::new(seq, t1, t2, t3, t4));
    }

    let estimate = filter.compute_estimate().expect("Estimate should converge");

    // The floor filter discards all congested queuing peaks and converges on exact base values
    assert_eq!(estimate.forward_delay_floor_ns, 23_800);
    assert_eq!(estimate.reverse_delay_floor_ns, 26_200);
    assert_eq!(estimate.mean_path_delay_ns, 25_000);
    assert_eq!(estimate.estimated_offset_ns, -1_200);
    assert_eq!(estimate.valid_samples_in_window, 50);
}
