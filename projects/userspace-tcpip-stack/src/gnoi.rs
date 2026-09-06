//! gRPC Network Operations Interface (gNOI - Microservice Operations for Network Elements).
//!
//! Provides operational execution RPCs: System.Ping, System.Time, OS.Verify, and Healthz.Check.

use crate::ipv4::Ipv4Address;
use std::collections::HashMap;

pub const GNOI_PORT: u16 = 9339;
pub const GNOI_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GnoiHealthStatus {
    Healthy,
    Degraded,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GnoiHealthCheckResult {
    pub component: String,
    pub status: GnoiHealthStatus,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GnoiPingResult {
    pub sequence: u32,
    pub bytes: u32,
    pub rtt_us: u32,
    pub ttl: u8,
}

#[derive(Debug, Clone)]
pub struct GnoiServer {
    pub os_version: String,
    pub hostname: String,
    pub health_components: HashMap<String, GnoiHealthStatus>,
}

impl Default for GnoiServer {
    fn default() -> Self {
        Self::new()
    }
}

impl GnoiServer {
    pub fn new() -> Self {
        let mut health_components = HashMap::new();
        health_components.insert("SwitchingFabric".to_string(), GnoiHealthStatus::Healthy);
        health_components.insert("TransceiverOptics".to_string(), GnoiHealthStatus::Healthy);
        health_components.insert("BgpControlPlane".to_string(), GnoiHealthStatus::Healthy);
        health_components.insert("PowerSupplyUnit1".to_string(), GnoiHealthStatus::Healthy);
        health_components.insert("CoolingFans".to_string(), GnoiHealthStatus::Healthy);

        GnoiServer {
            os_version: "ToyNOS-v2.5.0-LTS".to_string(),
            hostname: "switch-leaf-01".to_string(),
            health_components,
        }
    }

    /// Executes a gNOI System.Ping RPC
    pub fn execute_ping(&self, _target: Ipv4Address, count: u32) -> Vec<GnoiPingResult> {
        let mut results = Vec::with_capacity(count as usize);
        for seq in 1..=count {
            results.push(GnoiPingResult {
                sequence: seq,
                bytes: 64,
                rtt_us: 250 + (seq * 15), // Simulated microsecond round-trip time
                ttl: 64,
            });
        }
        results
    }

    /// Executes a gNOI Healthz.Check RPC
    pub fn check_health(&self) -> Vec<GnoiHealthCheckResult> {
        self.health_components
            .iter()
            .map(|(comp, status)| GnoiHealthCheckResult {
                component: comp.clone(),
                status: status.clone(),
                message: format!("Component {} is {:?}", comp, status),
            })
            .collect()
    }

    /// Executes an OS.Verify RPC
    pub fn verify_os(&self) -> (&str, bool) {
        (&self.os_version, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gnoi_ping_execution() {
        let server = GnoiServer::new();
        let results = server.execute_ping(Ipv4Address::new(192, 168, 1, 1), 3);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].sequence, 1);
        assert_eq!(results[0].bytes, 64);
        assert!(results[0].rtt_us > 0);
    }

    #[test]
    fn test_gnoi_healthz_and_os_verify() {
        let server = GnoiServer::new();
        let health = server.check_health();
        assert_eq!(health.len(), 5);

        let (version, valid) = server.verify_os();
        assert_eq!(version, "ToyNOS-v2.5.0-LTS");
        assert!(valid);
    }
}
