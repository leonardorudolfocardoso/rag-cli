use crate::{Embedding, Model};

pub struct VectorStore<E: Model> {
    items: Vec<(String, Embedding<E>)>,
}

impl<E: Model> VectorStore<E> {
    pub fn new() -> Self {
        VectorStore { items: vec![] }
    }

    pub fn insert(&mut self, text: String, vec: Embedding<E>) {
        self.items.push((text, vec))
    }

    pub fn top_k(&self, q: &Embedding<E>, k: usize) -> Vec<(f32, &str)> {
        // score all + sort + truncate
        let mut scored: Vec<(f32, &str)> = Vec::with_capacity(self.items.len());
        for (text, item) in &self.items {
            let score = item.cosine(q);
            scored.push((score, text));
        }
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        scored.truncate(k);
        scored
    }
}

impl<E: Model> Default for VectorStore<E> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::VectorStore;
    use crate::{Embedding, Model};

    /// 2-D model so tests can use hand-computable vectors.
    struct Tiny;
    impl Model for Tiny {
        const NAME: &'static str = "tiny";
        const DIMS: usize = 2;
    }

    const EPS: f32 = 1e-4;
    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < EPS
    }

    fn emb(v: [f32; 2]) -> Embedding<Tiny> {
        Embedding::new(v.to_vec()).unwrap()
    }

    fn sample() -> VectorStore<Tiny> {
        let mut s = VectorStore::new();
        s.insert("a".into(), emb([1.0, 0.0]));
        s.insert("b".into(), emb([0.0, 1.0]));
        s.insert("c".into(), emb([1.0, 1.0])); // not unit length: Embedding::new must normalise
        s
    }

    #[test]
    fn ranks_by_cosine() {
        // q = [1, 0.1]: cos with a ≈ 0.9950, c ≈ 0.7740, b ≈ 0.0995
        let s = sample();
        let top = s.top_k(&emb([1.0, 0.1]), 3);
        let texts: Vec<&str> = top.iter().map(|(_, t)| *t).collect();
        assert_eq!(texts, ["a", "c", "b"]);
        assert!(close(top[0].0, 0.99504), "got {}", top[0].0);
        assert!(close(top[1].0, 0.77396), "got {}", top[1].0);
        assert!(close(top[2].0, 0.09950), "got {}", top[2].0);
    }

    #[test]
    fn unnormalised_input_scores_as_cosine() {
        // stored [3, 0] vs query [1, 1]: cos = 1/√2 ≈ 0.7071.
        // If Embedding::new didn't normalise, the score would be 3.0.
        let mut s = VectorStore::new();
        s.insert("x".into(), emb([3.0, 0.0]));
        let top = s.top_k(&emb([1.0, 1.0]), 1);
        assert!(close(top[0].0, 0.70711), "got {}", top[0].0);
    }

    #[test]
    fn zero_vectors_score_zero() {
        let mut s = VectorStore::new();
        s.insert("zero".into(), emb([0.0, 0.0]));
        s.insert("a".into(), emb([1.0, 0.0]));

        // zero stored vector: no NaN, scores 0
        let top = s.top_k(&emb([1.0, 0.0]), 2);
        assert_eq!(top[0].1, "a");
        assert_eq!(top[1].1, "zero");
        assert!(close(top[1].0, 0.0), "got {}", top[1].0);

        // zero query: everything scores 0
        let top = s.top_k(&emb([0.0, 0.0]), 2);
        assert!(top.iter().all(|(score, _)| close(*score, 0.0)));
    }

    #[test]
    fn query_length_does_not_matter() {
        // cosine ignores length: q and 10·q must give the same scores
        let s = sample();
        let small = s.top_k(&emb([1.0, 0.1]), 3);
        let big = s.top_k(&emb([10.0, 1.0]), 3);
        assert_eq!(small.len(), 3);
        assert_eq!(big.len(), 3);
        for ((sa, ta), (sb, tb)) in small.iter().zip(&big) {
            assert_eq!(ta, tb);
            assert!(close(*sa, *sb), "{sa} vs {sb}");
        }
    }

    #[test]
    fn opposite_vector_scores_minus_one() {
        let s = sample();
        let top = s.top_k(&emb([-1.0, 0.0]), 3);
        let (score, text) = top.last().unwrap();
        assert_eq!(*text, "a"); // "a" is opposite
        assert!(close(*score, -1.0), "got {score}");
    }

    #[test]
    fn k_limits_results() {
        let s = sample();
        let q = emb([1.0, 0.1]);
        assert_eq!(s.top_k(&q, 1).len(), 1);
        assert_eq!(s.top_k(&q, 0).len(), 0);
        assert_eq!(s.top_k(&q, 10).len(), 3); // k > len: all, no panic
    }

    #[test]
    fn empty_store_returns_nothing() {
        let s = VectorStore::<Tiny>::new();
        assert!(s.top_k(&emb([1.0, 0.0]), 3).is_empty());
    }

    #[test]
    fn wrong_dimension_is_rejected() {
        let err = Embedding::<Tiny>::new(vec![1.0]).err().unwrap();
        assert!(err.contains("expected 2 dims, got 1"), "got {err}");
    }
}
