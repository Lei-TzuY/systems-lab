//! Network layer routing table and Longest Prefix Match (LPM) route lookup.

use crate::ipv4::Ipv4Address;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteEntry {
    pub destination: Ipv4Address,
    pub prefix_len: u8,
    pub netmask: u32,
    pub gateway: Option<Ipv4Address>,
    pub interface: String,
}

impl RouteEntry {
    pub fn new(
        destination: Ipv4Address,
        prefix_len: u8,
        gateway: Option<Ipv4Address>,
        interface: &str,
    ) -> Self {
        let netmask = if prefix_len == 0 {
            0u32
        } else {
            !((1u32 << (32 - prefix_len)) - 1)
        };

        RouteEntry {
            destination,
            prefix_len,
            netmask,
            gateway,
            interface: interface.to_string(),
        }
    }

    pub fn matches(&self, ip: Ipv4Address) -> bool {
        (ip.to_u32() & self.netmask) == (self.destination.to_u32() & self.netmask)
    }

    pub fn next_hop(&self, destination: Ipv4Address) -> Ipv4Address {
        self.gateway.unwrap_or(destination)
    }
}

impl fmt::Display for RouteEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let gw_str = match self.gateway {
            Some(gw) => gw.to_string(),
            None => "on-link".to_string(),
        };
        write!(
            f,
            "{}/{} via {} dev {}",
            self.destination, self.prefix_len, gw_str, self.interface
        )
    }
}

#[derive(Debug, Default, Clone)]
pub struct RoutingTable {
    routes: Vec<RouteEntry>,
}

impl RoutingTable {
    pub fn new() -> Self {
        RoutingTable { routes: Vec::new() }
    }

    pub fn add_route(
        &mut self,
        destination: Ipv4Address,
        prefix_len: u8,
        gateway: Option<Ipv4Address>,
        interface: &str,
    ) {
        let entry = RouteEntry::new(destination, prefix_len, gateway, interface);
        self.routes.push(entry);
        // Sort descending by prefix_len for simple LPM
        self.routes.sort_by(|a, b| b.prefix_len.cmp(&a.prefix_len));
    }

    pub fn lookup(&self, dst_ip: Ipv4Address) -> Option<&RouteEntry> {
        self.routes.iter().find(|r| r.matches(dst_ip))
    }

    pub fn all_routes(&self) -> &[RouteEntry] {
        &self.routes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_longest_prefix_match() {
        let mut rt = RoutingTable::new();
        // Default route 0.0.0.0/0 via 192.168.1.1
        rt.add_route(Ipv4Address::UNSPECIFIED, 0, Some(Ipv4Address::new(192, 168, 1, 1)), "eth0");
        // Subnet route 192.168.1.0/24 direct
        rt.add_route(Ipv4Address::new(192, 168, 1, 0), 24, None, "eth0");
        // Specific host route 192.168.1.50/32 direct
        rt.add_route(Ipv4Address::new(192, 168, 1, 50), 32, None, "eth0");

        // 192.168.1.50 matches /32
        let r1 = rt.lookup(Ipv4Address::new(192, 168, 1, 50)).unwrap();
        assert_eq!(r1.prefix_len, 32);

        // 192.168.1.20 matches /24
        let r2 = rt.lookup(Ipv4Address::new(192, 168, 1, 20)).unwrap();
        assert_eq!(r2.prefix_len, 24);

        // 8.8.8.8 matches /0 default gateway
        let r3 = rt.lookup(Ipv4Address::new(8, 8, 8, 8)).unwrap();
        assert_eq!(r3.prefix_len, 0);
        assert_eq!(r3.gateway, Some(Ipv4Address::new(192, 168, 1, 1)));
    }
}
