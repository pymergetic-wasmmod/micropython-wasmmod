//! Dependency closure resolution over ``wasmmod.deps`` (MPWD).
//!
//! Builds an install/load order that respects cycles via strongly connected
//! components (Tarjan): within an SCC all peers load before any ``mp_pack_load``.

use std::collections::{HashMap, HashSet};

use super::pack::{self, DepEntry};

/// One package pin in the closure.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DepNode {
    pub name: String,
    pub version: String,
}

impl DepNode {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }

    pub fn key(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }
}

/// Ordered SCCs: each inner vec is a cycle group (or singleton); outer order
/// is dependency-before-dependent (leaves / callees first).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Closure {
    pub sccs: Vec<Vec<DepNode>>,
}

impl Closure {
    /// Flat iteration in load order (SCC groups expanded left-to-right).
    pub fn nodes(&self) -> impl Iterator<Item = &DepNode> {
        self.sccs.iter().flat_map(|scc| scc.iter())
    }

    pub fn len(&self) -> usize {
        self.sccs.iter().map(|s| s.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Provider of dep edges for a package already fetched (or known).
pub trait DepSource {
    /// Return direct deps of ``node`` (from MPWD or an index). Empty if none.
    fn deps_of(&mut self, node: &DepNode) -> Result<Vec<DepNode>, String>;
}

/// In-memory graph for tests / pre-resolved maps: ``name@version → deps``.
pub struct MapDepSource {
    pub edges: HashMap<String, Vec<DepNode>>,
}

impl DepSource for MapDepSource {
    fn deps_of(&mut self, node: &DepNode) -> Result<Vec<DepNode>, String> {
        Ok(self.edges.get(&node.key()).cloned().unwrap_or_default())
    }
}

/// Parse MPWD from artifact bytes into owned ``DepNode``s.
pub fn deps_from_artifact(bytes: &[u8]) -> Vec<DepNode> {
    let Some(payload) = pack::deps_find_section(bytes) else {
        return Vec::new();
    };
    let Some(info) = pack::deps_parse(payload) else {
        return Vec::new();
    };
    info.deps
        .iter()
        .map(|d: &DepEntry<'_>| DepNode::new(d.name, d.version))
        .collect()
}

/// Walk the dep graph from ``root`` and return SCCs in load order.
///
/// Cycles (A↔B) share an SCC so the loader can register all peers before run.
pub fn resolve_closure<S: DepSource>(
    root: DepNode,
    source: &mut S,
) -> Result<Closure, String> {
    // Build adjacency (name@version keys) via BFS discovery.
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    let mut nodes: HashMap<String, DepNode> = HashMap::new();
    let mut queue = vec![root.clone()];
    nodes.insert(root.key(), root);

    while let Some(cur) = queue.pop() {
        let key = cur.key();
        if adj.contains_key(&key) {
            continue;
        }
        let deps = source.deps_of(&cur)?;
        let mut keys = Vec::with_capacity(deps.len());
        for d in deps {
            let dk = d.key();
            keys.push(dk.clone());
            if !nodes.contains_key(&dk) {
                nodes.insert(dk.clone(), d.clone());
                queue.push(d);
            }
        }
        adj.insert(key, keys);
    }

    let scc_keys = tarjan_sccs(&adj);
    // Tarjan emits SCCs in reverse topological order of the condensation DAG
    // (along original edges depender→dependee). That is already deps-first
    // (dependees / leaves before dependents).
    let mut sccs: Vec<Vec<DepNode>> = scc_keys
        .into_iter()
        .map(|group| {
            group
                .into_iter()
                .filter_map(|k| nodes.get(&k).cloned())
                .collect()
        })
        .filter(|g: &Vec<DepNode>| !g.is_empty())
        .collect();

    // Ensure root's SCC is present even with no edges.
    if sccs.is_empty() {
        if let Some(n) = nodes.values().next() {
            sccs.push(vec![n.clone()]);
        }
    }

    Ok(Closure { sccs })
}

/// Tarjan SCC. Returns components in the order discovered (reverse topo of
/// the condensation DAG).
fn tarjan_sccs(adj: &HashMap<String, Vec<String>>) -> Vec<Vec<String>> {
    let mut index = 0u32;
    let mut stack: Vec<String> = Vec::new();
    let mut on_stack: HashSet<String> = HashSet::new();
    let mut indices: HashMap<String, u32> = HashMap::new();
    let mut lowlink: HashMap<String, u32> = HashMap::new();
    let mut result: Vec<Vec<String>> = Vec::new();

    fn strongconnect(
        v: &str,
        adj: &HashMap<String, Vec<String>>,
        index: &mut u32,
        stack: &mut Vec<String>,
        on_stack: &mut HashSet<String>,
        indices: &mut HashMap<String, u32>,
        lowlink: &mut HashMap<String, u32>,
        result: &mut Vec<Vec<String>>,
    ) {
        indices.insert(v.to_string(), *index);
        lowlink.insert(v.to_string(), *index);
        *index += 1;
        stack.push(v.to_string());
        on_stack.insert(v.to_string());

        if let Some(neighbors) = adj.get(v) {
            for w in neighbors {
                if !indices.contains_key(w) {
                    strongconnect(
                        w, adj, index, stack, on_stack, indices, lowlink, result,
                    );
                    let lw = *lowlink.get(w).unwrap_or(&u32::MAX);
                    let lv = *lowlink.get(v).unwrap_or(&u32::MAX);
                    lowlink.insert(v.to_string(), lv.min(lw));
                } else if on_stack.contains(w) {
                    let iw = *indices.get(w).unwrap_or(&u32::MAX);
                    let lv = *lowlink.get(v).unwrap_or(&u32::MAX);
                    lowlink.insert(v.to_string(), lv.min(iw));
                }
            }
        }

        if lowlink.get(v) == indices.get(v) {
            let mut comp = Vec::new();
            loop {
                let w = stack.pop().expect("tarjan stack");
                on_stack.remove(&w);
                comp.push(w.clone());
                if w == v {
                    break;
                }
            }
            result.push(comp);
        }
    }

    let mut keys: Vec<String> = adj.keys().cloned().collect();
    keys.sort();
    for v in keys {
        if !indices.contains_key(&v) {
            strongconnect(
                &v,
                adj,
                &mut index,
                &mut stack,
                &mut on_stack,
                &mut indices,
                &mut lowlink,
                &mut result,
            );
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_source(edges: &[(&str, &str, &[(&str, &str)])]) -> MapDepSource {
        let mut m = HashMap::new();
        for (name, ver, deps) in edges {
            let node = DepNode::new(*name, *ver);
            m.insert(
                node.key(),
                deps.iter()
                    .map(|(n, v)| DepNode::new(*n, *v))
                    .collect(),
            );
        }
        MapDepSource { edges: m }
    }

    #[test]
    fn linear_deps_order() {
        // c <- b <- a  (a depends on b, b on c)
        let mut src = map_source(&[
            ("a", "1", &[("b", "1")]),
            ("b", "1", &[("c", "1")]),
            ("c", "1", &[]),
        ]);
        let closure = resolve_closure(DepNode::new("a", "1"), &mut src).unwrap();
        let names: Vec<_> = closure.nodes().map(|n| n.name.as_str()).collect();
        assert_eq!(names, vec!["c", "b", "a"]);
        assert_eq!(closure.sccs.len(), 3);
    }

    #[test]
    fn cyclic_ab_same_scc() {
        let mut src = map_source(&[
            ("a", "1", &[("b", "1")]),
            ("b", "1", &[("a", "1")]),
        ]);
        let closure = resolve_closure(DepNode::new("a", "1"), &mut src).unwrap();
        assert_eq!(closure.sccs.len(), 1);
        let mut names: Vec<_> = closure.sccs[0].iter().map(|n| n.name.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn cycle_with_leaf() {
        // a ↔ b, both depend on c
        let mut src = map_source(&[
            ("a", "1", &[("b", "1"), ("c", "1")]),
            ("b", "1", &[("a", "1"), ("c", "1")]),
            ("c", "1", &[]),
        ]);
        let closure = resolve_closure(DepNode::new("a", "1"), &mut src).unwrap();
        assert!(closure.sccs.len() >= 2);
        assert_eq!(closure.sccs[0].len(), 1);
        assert_eq!(closure.sccs[0][0].name, "c");
        let mut cycle: Vec<_> = closure.sccs[1].iter().map(|n| n.name.clone()).collect();
        cycle.sort();
        assert_eq!(cycle, vec!["a", "b"]);
    }

    #[test]
    fn deps_from_artifact_empty() {
        assert!(deps_from_artifact(b"\0asm\x01\0\0\0").is_empty());
    }
}
