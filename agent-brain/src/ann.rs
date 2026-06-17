//! In-memory top-K vector retrieval for large indexes (BM25 ∪ ANN hybrid).

use crate::db::store::CachedRow;
use crate::embed::dot_product;

#[derive(Debug, Clone, Copy)]
pub struct AnnSettings {
    pub enabled: bool,
    pub min_index: usize,
    pub top_k: usize,
}

impl Default for AnnSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            min_index: DEFAULT_ANN_MIN_INDEX,
            top_k: DEFAULT_ANN_TOP_K,
        }
    }
}

/// Minimum indexed rows before ANN candidate expansion activates.
pub const DEFAULT_ANN_MIN_INDEX: usize = 1_500;
pub const DEFAULT_ANN_TOP_K: usize = 100;

#[derive(Debug, Clone)]
pub struct AnnIndex {
    ids: Vec<String>,
    dims: usize,
    vectors: Vec<f32>,
}

impl AnnIndex {
    pub fn from_rows(rows: &[CachedRow]) -> Option<Self> {
        let mut ids = Vec::new();
        let mut vectors = Vec::new();
        let mut dims = 0usize;
        for row in rows {
            let Some(emb) = row.embedding.as_ref() else {
                continue;
            };
            if dims == 0 {
                dims = emb.len();
            }
            if emb.len() != dims || dims == 0 {
                continue;
            }
            ids.push(row.id.clone());
            vectors.extend_from_slice(emb);
        }
        if ids.is_empty() {
            return None;
        }
        Some(Self { ids, dims, vectors })
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_active(&self, min_index: usize) -> bool {
        self.len() >= min_index
    }

    /// Top-K by cosine similarity (vectors are unit-normalized at index time).
    pub fn top_k(&self, query: &[f32], k: usize) -> Vec<(String, f64)> {
        if query.len() != self.dims || self.ids.is_empty() {
            return Vec::new();
        }
        let take = k.min(self.ids.len());
        let mut scores: Vec<(usize, f64)> = (0..self.ids.len())
            .map(|i| {
                let start = i * self.dims;
                let slice = &self.vectors[start..start + self.dims];
                (i, dot_product(query, slice))
            })
            .collect();
        scores.select_nth_unstable_by(take - 1, |a, b| b.1.partial_cmp(&a.1).unwrap());
        scores.truncate(take);
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores
            .into_iter()
            .map(|(i, score)| (self.ids[i].clone(), score))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, emb: Vec<f32>) -> CachedRow {
        CachedRow::test_with_embedding(id, emb)
    }

    #[test]
    fn top_k_orders_by_similarity() {
        let rows = vec![
            row("a", vec![1.0, 0.0, 0.0]),
            row("b", vec![0.0, 1.0, 0.0]),
            row("c", vec![0.9, 0.1, 0.0]),
        ];
        let ann = AnnIndex::from_rows(&rows).unwrap();
        let top = ann.top_k(&[1.0, 0.0, 0.0], 2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, "a");
        assert_eq!(top[1].0, "c");
    }
}
