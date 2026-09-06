//! gRPC Network Management Interface (gNMI - OpenConfig Streaming Telemetry & Config).
//!
//! Models gNMI RPCs (Capabilities, Get, Set, Subscribe) and OpenConfig schema tree paths.

use std::collections::HashMap;

pub const GNMI_PORT: u16 = 9339;
pub const GNMI_VERSION: &str = "0.7.0";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GnmiPath {
    pub origin: String,
    pub elements: Vec<String>,
}

impl GnmiPath {
    pub fn parse(path_str: &str) -> Self {
        let trimmed = path_str.trim_matches('/');
        if trimmed.is_empty() {
            return GnmiPath {
                origin: "openconfig".to_string(),
                elements: Vec::new(),
            };
        }

        let mut elements = Vec::new();
        let mut current = String::new();
        let mut in_bracket = false;

        for ch in trimmed.chars() {
            match ch {
                '[' => {
                    in_bracket = true;
                    current.push(ch);
                }
                ']' => {
                    in_bracket = false;
                    current.push(ch);
                }
                '/' if !in_bracket => {
                    if !current.is_empty() {
                        elements.push(current);
                        current = String::new();
                    }
                }
                _ => current.push(ch),
            }
        }
        if !current.is_empty() {
            elements.push(current);
        }

        GnmiPath {
            origin: "openconfig".to_string(),
            elements,
        }
    }

    pub fn to_string_path(&self) -> String {
        format!("/{}", self.elements.join("/"))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GnmiValue {
    StringVal(String),
    IntVal(i64),
    UintVal(u64),
    BoolVal(bool),
    JsonVal(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct GnmiUpdate {
    pub path: GnmiPath,
    pub val: GnmiValue,
    pub timestamp_ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GnmiSubscriptionMode {
    Stream,
    Poll,
    OnChange,
}

#[derive(Debug, Clone)]
pub struct GnmiServer {
    pub datastore: HashMap<String, GnmiValue>,
    pub timestamp_ns: u64,
}

impl Default for GnmiServer {
    fn default() -> Self {
        Self::new()
    }
}

impl GnmiServer {
    pub fn new() -> Self {
        let mut server = GnmiServer {
            datastore: HashMap::new(),
            timestamp_ns: 1700000000000000000,
        };

        // Initialize standard OpenConfig telemetry nodes
        server.set_value(
            "/interfaces/interface[name=HundredGigE0/1]/state/admin-status",
            GnmiValue::StringVal("UP".to_string()),
        );
        server.set_value(
            "/interfaces/interface[name=HundredGigE0/1]/state/oper-status",
            GnmiValue::StringVal("UP".to_string()),
        );
        server.set_value(
            "/interfaces/interface[name=HundredGigE0/1]/state/counters/in-octets",
            GnmiValue::UintVal(104857600),
        );
        server.set_value(
            "/interfaces/interface[name=HundredGigE0/1]/state/counters/out-octets",
            GnmiValue::UintVal(89200100),
        );
        server.set_value(
            "/system/state/hostname",
            GnmiValue::StringVal("switch-spine-01".to_string()),
        );

        server
    }

    pub fn set_value(&mut self, path: &str, value: GnmiValue) {
        let gnmi_path = GnmiPath::parse(path);
        self.datastore.insert(gnmi_path.to_string_path(), value);
    }

    pub fn get(&self, path_query: &str) -> Vec<GnmiUpdate> {
        let mut updates = Vec::new();
        let normalized = GnmiPath::parse(path_query).to_string_path();

        for (path, val) in &self.datastore {
            if path.starts_with(&normalized) {
                updates.push(GnmiUpdate {
                    path: GnmiPath::parse(path),
                    val: val.clone(),
                    timestamp_ns: self.timestamp_ns,
                });
            }
        }
        updates
    }

    pub fn handle_subscribe(
        &self,
        path_query: &str,
        mode: GnmiSubscriptionMode,
    ) -> (GnmiSubscriptionMode, Vec<GnmiUpdate>) {
        let updates = self.get(path_query);
        (mode, updates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gnmi_get_and_path_filtering() {
        let server = GnmiServer::new();
        let updates = server.get("/interfaces/interface[name=HundredGigE0/1]/state");
        assert_eq!(updates.len(), 4);

        let octets =
            server.get("/interfaces/interface[name=HundredGigE0/1]/state/counters/in-octets");
        assert_eq!(octets.len(), 1);
        assert_eq!(octets[0].val, GnmiValue::UintVal(104857600));
    }

    #[test]
    fn test_gnmi_subscribe_stream_mode() {
        let server = GnmiServer::new();
        let (mode, stream_updates) =
            server.handle_subscribe("/system/state/hostname", GnmiSubscriptionMode::OnChange);

        assert_eq!(mode, GnmiSubscriptionMode::OnChange);
        assert_eq!(stream_updates.len(), 1);
        assert_eq!(
            stream_updates[0].val,
            GnmiValue::StringVal("switch-spine-01".to_string())
        );
    }
}
