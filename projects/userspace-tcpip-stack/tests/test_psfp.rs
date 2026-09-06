use toy_tcpip::psfp::{FlowMeter, GateState, MeterColor, PsfpFilterInstance, StreamGate};

#[test]
fn test_psfp_gate_cycling_and_transitions() {
    let mut gate = StreamGate::new(1, 500, 200); // 500µs cycle, 200µs open duration
    assert_eq!(gate.evaluate_time(0), GateState::Open);
    assert_eq!(gate.evaluate_time(200), GateState::Open);
    assert_eq!(gate.evaluate_time(201), GateState::Closed);
    assert_eq!(gate.evaluate_time(499), GateState::Closed);
}

#[test]
fn test_psfp_flow_meter_token_replenish() {
    let mut meter = FlowMeter::new(1, 1_000_000, 1000, true);
    assert_eq!(meter.meter_frame(800), MeterColor::Green);
    assert_eq!(meter.current_tokens, 200);

    // Frame of size 300 should fail (drop red)
    assert_eq!(meter.meter_frame(300), MeterColor::Red);

    // Replenish for 500µs -> +500 tokens -> 700 tokens
    meter.replenish(500);
    assert_eq!(meter.current_tokens, 700);
    assert_eq!(meter.meter_frame(300), MeterColor::Green);
}

#[test]
fn test_psfp_filter_instance_integration() {
    let gate = StreamGate::new(1, 1000, 300);
    let meter = FlowMeter::new(1, 2_000_000, 1500, true);
    let mut psfp = PsfpFilterInstance::new(42, 6, gate, meter);

    // Normal frame inside window
    let res = psfp.filter_and_police(150, 1000);
    assert_eq!(res, Ok(MeterColor::Green));
    assert_eq!(psfp.frames_passed, 1);

    // Frame outside window
    let res_drop_gate = psfp.filter_and_police(450, 200);
    assert!(res_drop_gate.is_err());
    assert_eq!(psfp.frames_dropped_gate, 1);
}
