use toy_tcpip::ti_lfa::TiLfaEngine;

#[test]
fn test_ti_lfa_protection_path_computation() {
    let mut engine = TiLfaEngine::new();

    // Setup Ring Topology:
    // R1 (101) --- 10 --- R2 (102) --- 10 --- R3 (103)
    //  |                                        |
    //  10                                       10
    //  |                                        |
    // R6 (106) --- 10 --- R5 (105) --- 10 --- R4 (104)

    engine.add_node("R1", 101);
    engine.add_node("R2", 102);
    engine.add_node("R3", 103);
    engine.add_node("R4", 104);
    engine.add_node("R5", 105);
    engine.add_node("R6", 106);

    engine.add_link("R1", "R2", 10, 201);
    engine.add_link("R2", "R3", 10, 202);
    engine.add_link("R3", "R4", 10, 203);
    engine.add_link("R4", "R5", 10, 204);
    engine.add_link("R5", "R6", 10, 205);
    engine.add_link("R6", "R1", 10, 206);

    // Primary route from R1 to R3 goes through R2.
    // If link (R1, R2) fails, TI-LFA routes via R6 -> R5 -> R4 -> R3.
    let protection = engine.compute_protection("R1", "R3", "R2").unwrap();

    assert_eq!(protection.primary_next_hop, "R2");
    assert_eq!(protection.backup_next_hop, "R6");
    assert_eq!(protection.failed_link, ("R1".to_string(), "R2".to_string()));
    assert!(protection.backup_segment_list.contains(&103));
}
