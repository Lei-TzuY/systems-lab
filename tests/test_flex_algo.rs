use toy_tcpip::flex_algo::{FlexAlgoDefinition, FlexAlgoEngine, FlexAlgoMetricType};

#[test]
fn test_flex_algo_multi_slice_constrained_spf() {
    let mut engine = FlexAlgoEngine::new();

    // Algo 128: Low Latency Slice
    engine.register_algo(FlexAlgoDefinition {
        algo_id: 128,
        metric_type: FlexAlgoMetricType::MinDelay,
        calculation_type: 0,
        exclude_affinity: 0,
        include_any_affinity: 0,
    });

    // Algo 130: High Bandwidth Slice (TE Metric)
    engine.register_algo(FlexAlgoDefinition {
        algo_id: 130,
        metric_type: FlexAlgoMetricType::TeMetric,
        calculation_type: 0,
        exclude_affinity: 0,
        include_any_affinity: 0,
    });

    // Topology:
    // Core1 --- Link1 (IGP=10, Delay=20, TE=100, Color=0) --- Core2
    // Core1 --- Link2 (IGP=100, Delay=5, TE=10, Color=0) --- Core2
    engine.add_link("Core1", "ViaLink1", 5, 10, 50, 0);
    engine.add_link("ViaLink1", "Core2", 5, 10, 50, 0);

    engine.add_link("Core1", "ViaLink2", 50, 2, 5, 0);
    engine.add_link("ViaLink2", "Core2", 50, 3, 5, 0);

    // Algo 128 (Min Delay) picks ViaLink2 (2+3 = 5us vs 10+10 = 20us)
    let (delay, path_delay) = engine.compute_flex_algo_spf(128, "Core1", "Core2").unwrap();
    assert_eq!(delay, 5);
    assert_eq!(path_delay, vec!["Core1", "ViaLink2", "Core2"]);

    // Algo 130 (TE Metric) picks ViaLink2 (5+5 = 10 vs 50+50 = 100)
    let (te, path_te) = engine.compute_flex_algo_spf(130, "Core1", "Core2").unwrap();
    assert_eq!(te, 10);
    assert_eq!(path_te, vec!["Core1", "ViaLink2", "Core2"]);
}
