// Adapted from OpenToonz toonz/sources/toonzlib/tcenterlinevectorizer.cpp
// (`organizeGraphs`) under BSD-3-Clause. See THIRD_PARTY_NOTICES.md.

//! Graph organization: prune short branches and merge through
//! degree-2 nodes.
//!
//! Operates in a loop until the graph is stable. Each pass:
//!
//! 1. Drops every edge whose path is shorter than
//!    `config.min_segment_length`.
//! 2. Merges any node whose remaining degree is 2 by concatenating its
//!    two incident edges into one.

use crate::config::CenterlineConfig;
use crate::skeleton::{Edge, Node, NodeKind, SkeletonGraph};

/// Returns an organized copy of `graph` with branches shorter than
/// `config.min_segment_length` pruned and degree-2 nodes merged.
pub(crate) fn organize(graph: SkeletonGraph, config: &CenterlineConfig) -> SkeletonGraph {
    let nodes = graph.nodes;
    let mut edges: Vec<Option<Edge>> = graph.edges.into_iter().map(Some).collect();
    let min_len = config.min_segment_length as usize;

    // Loop until a full pass changes nothing.
    loop {
        let mut changed = false;

        // Step 1: drop short edges that connect to at least one
        // endpoint. We keep short interior edges so we don't fragment
        // the graph; only stubs are pruned.
        for slot in &mut edges {
            let Some(e) = slot.as_ref() else {
                continue;
            };
            if e.path.len() >= min_len {
                continue;
            }
            let a_endpoint = matches!(nodes.get(e.a).map(|n| n.kind), Some(NodeKind::Endpoint));
            let b_endpoint = matches!(nodes.get(e.b).map(|n| n.kind), Some(NodeKind::Endpoint));
            if a_endpoint || b_endpoint {
                *slot = None;
                changed = true;
            }
        }

        // Step 2: merge through any node whose remaining degree is 2.
        // We collect candidate node indices first to avoid mutating
        // while iterating.
        let mut degree = vec![0u32; nodes.len()];
        for e in edges.iter().flatten() {
            if e.a < degree.len() {
                degree[e.a] += 1;
            }
            if e.b < degree.len() {
                degree[e.b] += 1;
            }
        }

        for ni in 0..nodes.len() {
            if degree[ni] != 2 {
                continue;
            }
            // Find the two edges touching this node.
            let mut incident: Vec<usize> = edges
                .iter()
                .enumerate()
                .filter_map(|(i, slot)| {
                    slot.as_ref().and_then(|e| {
                        if e.a == ni || e.b == ni {
                            Some(i)
                        } else {
                            None
                        }
                    })
                })
                .collect();
            if incident.len() != 2 {
                continue;
            }
            incident.sort_unstable();
            let ei2 = incident[1];
            let ei1 = incident[0];
            // Take both edges out, concatenate, push back as a single
            // edge between their far ends.
            let Some(e1) = edges[ei1].take() else {
                continue;
            };
            let Some(e2) = edges[ei2].take() else {
                continue;
            };
            let merged = merge_edges_through(&e1, &e2, ni);
            edges[ei1] = Some(merged);
            // ei2 stays None. Update degree array reflects this in the
            // next outer iteration; we set the local one too.
            let far_a = if e1.a == ni { e1.b } else { e1.a };
            let far_b = if e2.a == ni { e2.b } else { e2.a };
            // The two far nodes each lost one incident edge (the one
            // that touched `ni`) and gained one (the merged edge);
            // their degree is unchanged. `ni` itself drops to 0.
            degree[ni] = 0;
            // Avoid unused-variable lints when assertions are off.
            let _ = (far_a, far_b);
            changed = true;
        }

        if !changed {
            break;
        }
    }

    let edges: Vec<Edge> = edges.into_iter().flatten().collect();

    // Compact: drop nodes that no edge references and remap indices.
    let mut used = vec![false; nodes.len()];
    for e in &edges {
        if e.a < used.len() {
            used[e.a] = true;
        }
        if e.b < used.len() {
            used[e.b] = true;
        }
    }
    let mut remap = vec![usize::MAX; nodes.len()];
    let mut new_nodes: Vec<Node> = Vec::with_capacity(nodes.len());
    for (i, n) in nodes.iter().enumerate() {
        if used[i] {
            remap[i] = new_nodes.len();
            new_nodes.push(*n);
        }
    }
    let new_edges: Vec<Edge> = edges
        .into_iter()
        .map(|e| Edge {
            a: remap[e.a],
            b: remap[e.b],
            path: e.path,
        })
        .collect();

    SkeletonGraph {
        nodes: new_nodes,
        edges: new_edges,
    }
}

/// Concatenates two edges that share node `pivot` into one.
///
/// Reverses each edge's path as needed so the merged path runs from
/// `e1`'s far end through `pivot` to `e2`'s far end.
fn merge_edges_through(e1: &Edge, e2: &Edge, pivot: usize) -> Edge {
    // Walk e1 so its tail (last vertex) is at `pivot`.
    let (a_far, mut p1) = if e1.b == pivot {
        (e1.a, e1.path.clone())
    } else {
        let mut p = e1.path.clone();
        p.reverse();
        (e1.b, p)
    };

    // Walk e2 so its head (first vertex) is at `pivot`.
    let (b_far, mut p2) = if e2.a == pivot {
        (e2.b, e2.path.clone())
    } else {
        let mut p = e2.path.clone();
        p.reverse();
        (e2.a, p)
    };

    // Drop the duplicated pivot vertex from p2.
    if !p2.is_empty() {
        p2.remove(0);
    }
    p1.append(&mut p2);

    Edge {
        a: a_far,
        b: b_far,
        path: p1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skeleton::{Edge, Node, NodeKind, SkeletonGraph};

    fn build_node(x: u32, y: u32, kind: NodeKind) -> Node {
        Node { x, y, kind }
    }

    #[test]
    fn organize_drops_branch_shorter_than_min_segment() {
        // Main bar (5 px path) plus a 1-pixel stub branch.
        let main = Edge {
            a: 0,
            b: 1,
            path: vec![
                (0.0, 0.0, 1.0),
                (1.0, 0.0, 1.0),
                (2.0, 0.0, 1.0),
                (3.0, 0.0, 1.0),
                (4.0, 0.0, 1.0),
            ],
        };
        let stub = Edge {
            a: 2,
            b: 3,
            path: vec![(2.0, 0.0, 1.0)],
        };
        let graph = SkeletonGraph {
            nodes: vec![
                build_node(0, 0, NodeKind::Endpoint),
                build_node(4, 0, NodeKind::Endpoint),
                build_node(2, 0, NodeKind::Branch),
                build_node(2, 1, NodeKind::Endpoint),
            ],
            edges: vec![main, stub],
        };
        let cfg = CenterlineConfig {
            min_segment_length: 3,
            ..CenterlineConfig::default()
        };
        let out = organize(graph, &cfg);
        assert_eq!(out.edges.len(), 1, "stub should be pruned");
    }

    #[test]
    fn organize_merges_through_degree_2_node() {
        // Two edges meeting at a middle node; min_segment_length 1
        // keeps both. The middle node has degree 2 and should be
        // merged.
        let e1 = Edge {
            a: 0,
            b: 1,
            path: vec![(0.0, 0.0, 1.0), (1.0, 0.0, 1.0), (2.0, 0.0, 1.0)],
        };
        let e2 = Edge {
            a: 1,
            b: 2,
            path: vec![(2.0, 0.0, 1.0), (3.0, 0.0, 1.0), (4.0, 0.0, 1.0)],
        };
        let graph = SkeletonGraph {
            nodes: vec![
                build_node(0, 0, NodeKind::Endpoint),
                build_node(2, 0, NodeKind::Branch),
                build_node(4, 0, NodeKind::Endpoint),
            ],
            edges: vec![e1, e2],
        };
        let out = organize(graph, &CenterlineConfig::default());
        assert_eq!(out.edges.len(), 1, "two edges should merge into one");
        // Merged path runs (0..=4), 5 vertices total.
        assert_eq!(out.edges[0].path.len(), 5);
    }

    #[test]
    fn organize_keeps_long_branch() {
        let bar = Edge {
            a: 0,
            b: 1,
            path: vec![
                (0.0, 0.0, 1.0),
                (1.0, 0.0, 1.0),
                (2.0, 0.0, 1.0),
                (3.0, 0.0, 1.0),
            ],
        };
        let graph = SkeletonGraph {
            nodes: vec![
                build_node(0, 0, NodeKind::Endpoint),
                build_node(3, 0, NodeKind::Endpoint),
            ],
            edges: vec![bar],
        };
        let cfg = CenterlineConfig {
            min_segment_length: 2,
            ..CenterlineConfig::default()
        };
        let out = organize(graph, &cfg);
        assert_eq!(out.edges.len(), 1);
    }

    #[test]
    fn merge_edges_concatenates_paths() {
        let e1 = Edge {
            a: 0,
            b: 1,
            path: vec![(0.0, 0.0, 1.0), (1.0, 0.0, 1.0), (2.0, 0.0, 1.0)],
        };
        let e2 = Edge {
            a: 1,
            b: 2,
            path: vec![(2.0, 0.0, 1.0), (3.0, 0.0, 1.0), (4.0, 0.0, 1.0)],
        };
        let merged = merge_edges_through(&e1, &e2, 1);
        assert_eq!(merged.a, 0);
        assert_eq!(merged.b, 2);
        assert_eq!(merged.path.len(), 5);
        assert!((merged.path.last().unwrap().0 - 4.0).abs() < 1e-6);
    }
}
