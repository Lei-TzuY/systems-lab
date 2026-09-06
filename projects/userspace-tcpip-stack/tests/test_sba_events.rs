use toy_tcpip::sba_events::{
    SbaEventExposureEngine, SbaEventNotification, SbaEventSubscription, SbaEventType,
};

#[test]
fn test_sba_event_subscription_lifecycle() {
    let mut engine = SbaEventExposureEngine::new();

    let sub_id = engine.subscribe(
        "pcf-policy-01",
        SbaEventType::PresenceInAreaOfInterest,
        "imsi-208950000000001",
        "https://pcf.5gcore.local/v1/events/aoi",
    );
    assert_eq!(sub_id, 1);
    assert_eq!(engine.subscriptions.len(), 1);

    let sample_sub: &SbaEventSubscription = &engine.subscriptions[0];
    assert_eq!(sample_sub.subscriber_nf_id, "pcf-policy-01");

    // Trigger event for different SUPI -> No match
    let dispatches1 = engine.trigger_event(
        SbaEventType::PresenceInAreaOfInterest,
        "imsi-208950000000099",
        1700000100,
        "Entered AOI Zone-Taipei",
    );
    assert_eq!(dispatches1, 0);

    // Trigger event for matching SUPI -> 1 match
    let dispatches2 = engine.trigger_event(
        SbaEventType::PresenceInAreaOfInterest,
        "imsi-208950000000001",
        1700000105,
        "Entered AOI Zone-Taipei",
    );
    assert_eq!(dispatches2, 1);
    assert_eq!(engine.notifications_log.len(), 1);

    let notif: &SbaEventNotification = &engine.notifications_log[0];
    assert_eq!(notif.supi, "imsi-208950000000001");

    // Unsubscribe
    assert!(engine.unsubscribe(sub_id));
    assert_eq!(engine.subscriptions.len(), 0);
}
