//! Flexible Algorithms for Segment Routing (SR-Flex-Algo - RFC 9350 / RFC 9351).
//!
//! Enables network slicing and custom constraint-based SPF routing (Min Delay, TE Metric, Affinity Exclusions)
//! for Segment Routing (SR-MPLS and SRv6).

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexAlgoMetricType {
    IgpMetric = 0,
    MinDelay = 1,
    TeMetric = 2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlexAlgoDefinition {
    pub algo_id: u8, // User-defined range: 128..255
    pub metric_type: FlexAlgoMetricType,
    pub calculation_type: u8, // 0 = Shortest Path First
    pub exclude_affinity: u32, // Bitmask of colors/affinities to prune
    pub include_any_affinity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlexAlgoLink {
    pub from: String,
    pub to: String,
    pub igp_cost: u32,
    pub delay_us: u32,
    pub te_cost: u32,
    pub admin_group: u32, // Affinity bitmask
}

#[derive(Debug, Clone, Default)]
pub struct FlexAlgoEngine {
    pub fad_map: HashMap<u8, FlexAlgoDefinition>,
    pub links: Vec<FlexAlgoLink>,
    pub nodes: HashSet<String>,
}

impl FlexAlgoEngine {
    pub fn new() -> Self {
        FlexAlgoEngine {
            fad_map: HashMap::new(),
            links: Vec::new(),
            nodes: HashSet::new(),
        }
    }

    pub fn register_algo(&mut self, fad: FlexAlgoDefinition) {
        self.fad_map.insert(fad.algo_id, fad);
    }

    pub fn add_node(&mut self, name: &str) {
        self.nodes.insert(name.to_string());
    }

    pub fn add_link(
        &mut self,
        from: &str,
        to: &str,
        igp_cost: u32,
        delay_us: u32,
        te_cost: u32,
        admin_group: u32,
    ) {
        self.nodes.insert(from.to_string());
        self.nodes.insert(to.to_string());
        self.links.push(FlexAlgoLink {
            from: from.to_string(),
            to: to.to_string(),
            igp_cost,
            delay_us,
            te_cost,
            admin_group,
        });
        self.links.push(FlexAlgoLink {
            from: to.to_string(),
            to: from.to_string(),
            igp_cost,
            delay_us,
            te_cost,
            admin_group,
        });
    }

    /// Computes the constrained SPF path for a specific Flex-Algo ID from source to destination
    pub fn compute_flex_algo_spf(
        &self,
        algo_id: u8,
        src: &str,
        dst: &str,
    ) -> Option<(u32, Vec<String>)> {
        let fad = self.fad_map.get(&algo_id)?;

        let mut dist: HashMap<String, u32> = HashMap::new();
        let mut prev: HashMap<String, Option<String>> = HashMap::new();
        let mut visited: HashSet<String> = HashSet::new();

        for node in &self.nodes {
            dist.insert(node.clone(), u32::MAX);
            prev.insert(node.clone(), None);
        }
        dist.insert(src.to_string(), 0);

        while visited.len() < self.nodes.len() {
            let mut u_node = None;
            let mut min_d = u32::MAX;

            for (node, &d) in &dist {
                if !visited.contains(node) && d < min_d {
                    min_d = d;
                    u_node = Some(node.clone());
                }
            }

            let u = match u_node {
                Some(u) => u,
                None => break,
            };

            visited.insert(u.clone());

            for link in &self.links {
                if link.from == u {
                    // Check Affinity Exclusions
                    if (link.admin_group & fad.exclude_affinity) != 0 {
                        continue; // Pruned link
                    }

                    // Check Include-Any Affinity
                    if fad.include_any_affinity != 0 && (link.admin_group & fad.include_any_affinity) == 0 {
                        continue;
                    }

                    let metric = match fad.metric_type {
                        FlexAlgoMetricType::IgpMetric => link.igp_cost,
                        FlexAlgoMetricType::MinDelay => link.delay_us,
                        FlexAlgoMetricType::TeMetric => link.te_cost,
                    };

                    let alt = dist[&u].saturating_add(metric);
                    if alt < dist.get(&link.to).copied().unwrap_or(u32::MAX) {
                        dist.insert(link.to.clone(), alt);
                        prev.insert(link.to.clone(), Some(u.clone()));
                    }
                }
            }
        }

        let total_cost = *dist.get(dst)?;
        if total_cost == u32::MAX {
            return None;
        }

        // Reconstruct path
        let mut path = Vec::new();
        let mut curr = dst.to_string();
        path.push(curr.clone());

        while let Some(Some(p)) = prev.get(&curr) {
            path.push(p.clone());
            if p == src {
                break;
            }
            curr = p.clone();
        }

        path.reverse();
        Some((total_cost, path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flex_algo_min_delay_vs_igp_cost() {
        let mut engine = FlexAlgoEngine::new();

        // Register Algo 128: Min Delay
        engine.register_algo(FlexAlgoDefinition {
            algo_id: 128,
            metric_type: FlexAlgoMetricType::MinDelay,
            calculation_type: 0,
            exclude_affinity: 0,
            include_any_affinity: 0,
        });

        // Register Algo 129: Exclude Red Links (0x02)
        engine.register_algo(FlexAlgoDefinition {
            algo_id: 129,
            metric_type: FlexAlgoMetricType::IgpMetric,
            calculation_type: 0,
            exclude_affinity: 0x02,
            include_any_affinity: 0,
        });

        // Topology:
        // Path A: NodeA --- (IGP: 10, Delay: 100us, Color: 0x01) --- NodeB
        // Path B: NodeA --- (IGP: 100, Delay: 10us, Color: 0x02) --- NodeB
        engine.add_link("NodeA", "NodeB_Via_A", 10, 100, 10, 0x01);
        engine.add_link("NodeB_Via_A", "NodeB", 10, 100, 10, 0x01);

        engine.add_link("NodeA", "NodeB_Via_B", 100, 10, 100, 0x02);
        engine.add_link("NodeB_Via_B", "NodeB", 100, 10, 100, 0x02);

        // Algo 128 (Min Delay) picks Path B (total delay 20us vs 200us)
        let (delay_metric, path_delay) = engine.compute_flex_algo_spf(128, "NodeA", "NodeB").unwrap();
        assert_eq!(delay_metric, 20);
        assert_eq!(path_delay, vec!["NodeA", "NodeB_Via_B", "NodeB"]);

        // Algo 129 (Exclude Red 0x02) prunes Path B and picks Path A
        let (igp_metric, path_igp) = engine.compute_flex_algo_spf(129, "NodeA", "NodeB").unwrap();
        assert_eq!(igp_metric, 20);
        assert_eq!(path_igp, vec!["NodeA", "NodeB_Via_A", "NodeB"]);
    }
}
