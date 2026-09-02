use toy_tcpip::ptp_pdv_filter::{PtpPdvFloorFilter, PtpTimestampSample};

#[test]
fn test_ptp_time_error_metrics_calculation() {
    let mut filter = PtpPdvFloorFilter::new(10, 10.0, 100);

    // Physical one-way delay = 20,000 ns
    // Injected time offset = +250 ns
    // Forward delay base = 20,250 ns
    // Reverse delay base = 19,750 ns
    // Observed offset = (20,250 - 19,750) / 2 = +250 ns
    for i in 0..10 {
        let t1 = (i as i64) * 1_000_000;
        let t2 = t1 + 20_250;
        let t3 = t2 + 50_000;
        let t4 = t3 + 19_750;

        filter.push_sample(PtpTimestampSample::new(i as u16, t1, t2, t3, t4));
    }

    let te = filter.compute_time_error_metrics().expect("Time error metrics");
    assert_eq!(te.cte_ns, 250.0);
    assert_eq!(te.dte_peak_to_peak_ns, 0);
    assert_eq!(te.max_abs_te_ns, 250);
    assert_eq!(te.sample_count, 10);
}

#[test]
fn test_ptp_iqr_outlier_filtering() {
    // Standard queuing delays between 10,000 and 12,000 ns, with two massive buffer exhaustion spikes (1,000,000 ns)
    let delays = vec![
        10_000, 10_200, 10_150, 10_300, 10_500, 10_450, 10_600, 10_550,
        11_000, 11_200, 11_100, 11_300, 11_500, 11_400, 11_600, 11_550,
        1_000_000, 1_500_000, // Severe outliers
    ];

    let filtered = PtpPdvFloorFilter::filter_iqr_outliers(&delays);
    assert!(!filtered.contains(&1_000_000));
    assert!(!filtered.contains(&1_500_000));
    assert!(filtered.len() >= 16);
}

#[test]
fn test_ptp_subwindow_lucky_packet_selection() {
    let mut filter = PtpPdvFloorFilter::new(40, 5.0, 150);

    // True forward delay floor = 15,000 ns
    // True reverse delay floor = 18,000 ns
    // True offset = (15,000 - 18,000) / 2 = -1,500 ns
    // True mean delay = (15,000 + 18,000) / 2 = 16,500 ns

    // 4 subwindows of 10 packets each.
    // In each subwindow, exactly packet #0 has clean floor (0 queuing delay).
    // Remaining packets experience random bursty queuing delay.
    for i in 0..40 {
        let is_lucky = (i % 10) == 0;
        let q_fwd = if is_lucky { 0 } else { ((i as i64 * 31) % 100) * 1_000 };
        let q_rev = if is_lucky { 0 } else { ((i as i64 * 47) % 120) * 1_000 };

        let t1 = (i as i64) * 10_000_000;
        let t2 = t1 + 15_000 + q_fwd;
        let t3 = t2 + 25_000;
        let t4 = t3 + 18_000 + q_rev;

        filter.push_sample(PtpTimestampSample::new(i as u16, t1, t2, t3, t4));
    }

    let estimate = filter
        .compute_subwindow_lucky_estimate(4)
        .expect("Subwindow lucky estimate");

    assert_eq!(estimate.forward_delay_floor_ns, 15_000);
    assert_eq!(estimate.reverse_delay_floor_ns, 18_000);
    assert_eq!(estimate.estimated_offset_ns, -1_500);
    assert_eq!(estimate.mean_path_delay_ns, 16_500);
}
