//! 5G Core Service Based Architecture Event Exposure Service (3GPP TS 29.518 Namf_EventExposure & TS 29.508 Nsmf_EventExposure).
//!
//! Implements 5G SBA Event Exposure subscription management and event notification dispatch
//! for analytics (NWDAF), policy (PCF), and exposure (NEF) consumers.

/// 5G Core Event Exposure Event Types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SbaEventType {
    LocationReport,
    PresenceInAreaOfInterest,
    TimezoneChange,
    PduSessionEstablishment,
    PduSessionRelease,
    LossOfConnectivity,
}

/// 5G SBA Event Subscription Model
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbaEventSubscription {
    pub sub_id: u32,
    pub subscriber_nf_id: String,
    pub event_type: SbaEventType,
    pub target_supi: String,      // "imsi-208950000000001" or "*" for any UE
    pub notification_uri: String, // Webhook target URI (e.g., "https://nef.5gcore.local/events/notify")
}

/// 5G SBA Event Notification Dispatch Record
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbaEventNotification {
    pub sub_id: u32,
    pub event_type: SbaEventType,
    pub supi: String,
    pub timestamp_sec: u64,
    pub details: String,
    pub destination_uri: String,
}

/// 5G SBA Event Exposure Engine
#[derive(Debug, Clone, Default)]
pub struct SbaEventExposureEngine {
    pub subscriptions: Vec<SbaEventSubscription>,
    pub notifications_log: Vec<SbaEventNotification>,
    pub next_sub_id: u32,
}

impl SbaEventExposureEngine {
    pub fn new() -> Self {
        SbaEventExposureEngine {
            subscriptions: Vec::new(),
            notifications_log: Vec::new(),
            next_sub_id: 1,
        }
    }

    /// Creates an event subscription (Namf_EventExposure_Subscribe / Nsmf_EventExposure_Subscribe)
    pub fn subscribe(
        &mut self,
        subscriber_nf_id: &str,
        event_type: SbaEventType,
        target_supi: &str,
        notification_uri: &str,
    ) -> u32 {
        let sub_id = self.next_sub_id;
        self.next_sub_id += 1;

        self.subscriptions.push(SbaEventSubscription {
            sub_id,
            subscriber_nf_id: subscriber_nf_id.to_string(),
            event_type,
            target_supi: target_supi.to_string(),
            notification_uri: notification_uri.to_string(),
        });

        sub_id
    }

    /// Triggers an internal 5G event and dispatches notifications to all matching subscribers
    pub fn trigger_event(
        &mut self,
        event_type: SbaEventType,
        supi: &str,
        timestamp_sec: u64,
        details: &str,
    ) -> usize {
        let mut dispatched_count = 0;

        for sub in &self.subscriptions {
            if sub.event_type == event_type && (sub.target_supi == "*" || sub.target_supi == supi) {
                let notif = SbaEventNotification {
                    sub_id: sub.sub_id,
                    event_type: event_type.clone(),
                    supi: supi.to_string(),
                    timestamp_sec,
                    details: details.to_string(),
                    destination_uri: sub.notification_uri.clone(),
                };
                self.notifications_log.push(notif);
                dispatched_count += 1;
            }
        }

        dispatched_count
    }

    /// Unsubscribes by Subscription ID
    pub fn unsubscribe(&mut self, sub_id: u32) -> bool {
        let initial_len = self.subscriptions.len();
        self.subscriptions.retain(|s| s.sub_id != sub_id);
        self.subscriptions.len() < initial_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sba_event_subscription_and_dispatch() {
        let mut engine = SbaEventExposureEngine::new();

        let sub1 = engine.subscribe(
            "nef-gateway-01",
            SbaEventType::LocationReport,
            "imsi-208950000000001",
            "https://nef.5gcore.local/v1/notify",
        );
        let sub2 = engine.subscribe(
            "nwdaf-analytics-01",
            SbaEventType::PduSessionEstablishment,
            "*",
            "https://nwdaf.5gcore.local/v1/pdu-events",
        );

        assert_eq!(sub1, 1);
        assert_eq!(sub2, 2);

        // Trigger Location Report for UE 1
        let count1 = engine.trigger_event(
            SbaEventType::LocationReport,
            "imsi-208950000000001",
            1700000000,
            "CellId=10101, TAC=0x0001",
        );
        assert_eq!(count1, 1);

        // Trigger PDU Session Establishment for UE 2 (matches wildcard *)
        let count2 = engine.trigger_event(
            SbaEventType::PduSessionEstablishment,
            "imsi-208950000000002",
            1700000010,
            "DNN=internet, S-NSSAI=1:0xFFFFFF",
        );
        assert_eq!(count2, 1);
        assert_eq!(engine.notifications_log.len(), 2);
    }
}
