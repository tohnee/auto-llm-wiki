use std::collections::{HashMap, HashSet, VecDeque};

use wiki_core::{ClaimId, RankedClaim};
use wiki_storage::{GraphEdgeRecord, SqliteWikiRepository, StorageError};

pub type Result<T> = std::result::Result<T, StorageError>;

pub struct MempalaceGraphRetriever<'a> {
    repo: &'a SqliteWikiRepository,
    walk_depth: usize,
    max_neighbors: usize,
}

impl<'a> MempalaceGraphRetriever<'a> {
    pub fn new(repo: &'a SqliteWikiRepository, walk_depth: usize, max_neighbors: usize) -> Self {
        Self {
            repo,
            walk_depth,
            max_neighbors,
        }
    }

    pub fn retrieve(&self, query: &str) -> Result<Vec<RankedClaim>> {
        let nodes = self.repo.list_graph_nodes()?;
        let edges = self.repo.list_graph_edges()?;
        let adjacency = build_adjacency(&edges, self.max_neighbors);
        let start_nodes: Vec<_> = nodes
            .iter()
            .filter(|node| contains_any_token(&node.label, query))
            .map(|node| node.node_id.clone())
            .collect();

        let node_map: HashMap<_, _> = nodes
            .into_iter()
            .map(|node| (node.node_id.clone(), node))
            .collect();
        let mut claim_scores: HashMap<ClaimId, f64> = HashMap::new();

        for start in start_nodes {
            let mut queue = VecDeque::from([(start, 0usize)]);
            let mut seen = HashSet::new();
            while let Some((node_id, depth)) = queue.pop_front() {
                if !seen.insert(node_id.clone()) {
                    continue;
                }
                if let Some(node) = node_map.get(&node_id)
                    && node.node_type == "claim"
                    && let Ok(claim_id) = ClaimId::parse(&node.external_ref)
                {
                    let score = 1.0 / (depth.max(1) as f64);
                    *claim_scores.entry(claim_id).or_insert(0.0) += score;
                }
                if depth >= self.walk_depth {
                    continue;
                }
                if let Some(neighbors) = adjacency.get(&node_id) {
                    for neighbor in neighbors {
                        queue.push_back((neighbor.clone(), depth + 1));
                    }
                }
            }
        }

        let mut ranked: Vec<_> = claim_scores.into_iter().collect();
        ranked.sort_by(|left, right| right.1.total_cmp(&left.1));
        Ok(ranked
            .into_iter()
            .enumerate()
            .map(|(index, (claim_id, _))| RankedClaim::new(claim_id, index + 1))
            .collect())
    }
}

fn build_adjacency(edges: &[GraphEdgeRecord], max_neighbors: usize) -> HashMap<String, Vec<String>> {
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
    for edge in edges {
        adjacency
            .entry(edge.from_node.clone())
            .or_default()
            .push(edge.to_node.clone());
        adjacency
            .entry(edge.to_node.clone())
            .or_default()
            .push(edge.from_node.clone());
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort();
        neighbors.dedup();
        neighbors.truncate(max_neighbors);
    }
    adjacency
}

fn contains_any_token(label: &str, query: &str) -> bool {
    let haystack = label.to_lowercase();
    query.to_lowercase()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| token.len() >= 4)
        .any(|token| haystack.contains(token))
}
