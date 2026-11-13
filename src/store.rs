pub struct VectorStore {
    items: Vec<(String, Vec<f32>)>,
}

fn normalise(vec: &mut [f32]) {
    let norm = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0_f32 {
        for item in vec.iter_mut() {
            *item /= norm
        }
    }
}

impl VectorStore {
    pub fn new() -> Self {
        VectorStore { items: vec![] }
    }

    pub fn insert(&mut self, text: String, mut vec: Vec<f32>) {
        normalise(&mut vec);
        self.items.push((text, vec))
    }

    pub fn top_k(&self, q: &[f32], k: usize) -> Vec<(f32, &str)> {
        // items are unit length, so the query must be too for dot == cosine
        let mut q = q.to_vec();
        normalise(&mut q);

        // score all + sort + truncate
        let mut scored: Vec<(f32, &str)> = Vec::with_capacity(self.items.len());
        for (text, value) in &self.items {
            let score = q.iter().zip(value).map(|(a, b)| a * b).sum();
            scored.push((score, text));
        }
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        scored.truncate(k);
        scored
    }
}

impl Default for VectorStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::VectorStore;

    const EPS: f32 = 1e-4;
    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < EPS
    }

    fn sample() -> VectorStore {
        let mut s = VectorStore::new();
        s.insert("a".into(), vec![1.0, 0.0]);
        s.insert("b".into(), vec![0.0, 1.0]);
        s.insert("c".into(), vec![1.0, 1.0]); // not unit length: insert must normalise
        s
    }

    #[test]
    fn ranks_by_cosine() {
        // q = [1, 0.1]: cos with a ≈ 0.9950, c ≈ 0.7740, b ≈ 0.0995
        let s = sample();
        let top = s.top_k(&[1.0, 0.1], 3);
        let texts: Vec<&str> = top.iter().map(|(_, t)| *t).collect();
        assert_eq!(texts, ["a", "c", "b"]);
        assert!(close(top[0].0, 0.99504), "got {}", top[0].0);
        assert!(close(top[1].0, 0.77396), "got {}", top[1].0);
        assert!(close(top[2].0, 0.09950), "got {}", top[2].0);
    }

    #[test]
    fn insert_normalises() {
        // stored [3, 0] vs query [1, 1]: cos = 1/√2 ≈ 0.7071.
        // Without normalising on insert the score would be 3.0 (or 4.24 unnormalised both ways).
        let mut s = VectorStore::new();
        s.insert("x".into(), vec![3.0, 0.0]);
        let top = s.top_k(&[1.0, 1.0], 1);
        assert!(close(top[0].0, 0.70711), "got {}", top[0].0);
    }

    #[test]
    fn zero_vectors_score_zero() {
        let mut s = VectorStore::new();
        s.insert("zero".into(), vec![0.0, 0.0]);
        s.insert("a".into(), vec![1.0, 0.0]);

        // zero stored vector: no NaN, scores 0
        let top = s.top_k(&[1.0, 0.0], 2);
        assert_eq!(top[0].1, "a");
        assert_eq!(top[1].1, "zero");
        assert!(close(top[1].0, 0.0), "got {}", top[1].0);

        // zero query: everything scores 0
        let top = s.top_k(&[0.0, 0.0], 2);
        assert!(top.iter().all(|(score, _)| close(*score, 0.0)));
    }

    #[test]
    fn query_length_does_not_matter() {
        // cosine ignores length: q and 10·q must give the same scores
        let s = sample();
        let small = s.top_k(&[1.0, 0.1], 3);
        let big = s.top_k(&[10.0, 1.0], 3);
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
        let top = s.top_k(&[-1.0, 0.0], 3);
        let (score, text) = top.last().unwrap();
        assert_eq!(*text, "a"); // "a" is opposite
        assert!(close(*score, -1.0), "got {score}");
    }

    #[test]
    fn k_limits_results() {
        let s = sample();
        assert_eq!(s.top_k(&[1.0, 0.1], 1).len(), 1);
        assert_eq!(s.top_k(&[1.0, 0.1], 0).len(), 0);
        assert_eq!(s.top_k(&[1.0, 0.1], 10).len(), 3); // k > len: all, no panic
    }

    #[test]
    fn empty_store_returns_nothing() {
        let s = VectorStore::new();
        assert!(s.top_k(&[1.0, 0.0], 3).is_empty());
    }
}
