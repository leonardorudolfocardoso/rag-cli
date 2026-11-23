//! Retrieval and end-to-end evaluations against `tests/fixtures`.
//!
//! - Offline self-checks: `cargo test --test eval`
//! - Retrieval only:      `cargo test --test eval eval_retrieval -- --ignored --nocapture`
//! - End-to-end (chat):   `cargo test --test eval eval_e2e -- --ignored --nocapture`

use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

use rag_cli::{Hit, Rag, chunk_markdown};
use serde::Deserialize;

const K: usize = 3;
const CASES_JSON: &str = include_str!("eval_cases.json");

/// Eval case bucket used to break down retrieval and answer metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Category {
    /// Single-chunk lookup using wording close to the note.
    Direct,
    /// Single-chunk lookup phrased differently from the note's literal terms.
    Paraphrase,
    /// Answer requires combining at least two distinct chunks.
    CrossChunk,
    /// Fictional/internal project fact the model cannot know from pretraining.
    Private,
    /// Near-miss question sharing fixture vocabulary whose answer is absent.
    Unanswerable,
}

impl Category {
    fn label(self) -> &'static str {
        match self {
            Category::Direct => "direct",
            Category::Paraphrase => "paraphrase",
            Category::CrossChunk => "cross_chunk",
            Category::Private => "private",
            Category::Unanswerable => "unanswerable",
        }
    }
}

/// A chunk that retrieval must surface for an answerable eval case, matched by
/// source filename and a case-insensitive substring inside that chunk.
#[derive(Debug, Deserialize)]
struct Evidence {
    source: String,
    snippet: String,
}

#[derive(Debug, Deserialize)]
struct Case {
    id: String,
    category: Category,
    q: String,
    evidence: Vec<Evidence>,
    keywords: Vec<String>,
}

fn load_cases() -> Vec<Case> {
    serde_json::from_str(CASES_JSON).expect("tests/eval_cases.json must be valid JSON")
}

fn fixture_files() -> Vec<PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir("tests/fixtures")
        .unwrap()
        .map(|e| e.unwrap().path())
        // Skip non-note entries (e.g. macOS `.DS_Store`) so `read_to_string` never chokes on them.
        .filter(|p| p.extension().is_some_and(|ext| ext == "md"))
        .collect();
    files.sort();
    files
}

fn hit_matches(hit: &Hit, ev: &Evidence) -> bool {
    hit.chunk.source.ends_with(&ev.source)
        && hit
            .chunk
            .text
            .to_lowercase()
            .contains(&ev.snippet.to_lowercase())
}

/// Checks chunk-level retrieval for an answerable case.
///
/// Returns `Some(rr)` where `rr = 1.0 / worst_rank` (1-based) if every required
/// evidence snippet was found in `hits`, or `None` if any evidence was missed.
fn reciprocal_rank(hits: &[Hit], evidence: &[Evidence]) -> Option<f32> {
    let mut worst_rank = 0usize;
    for ev in evidence {
        let rank = hits.iter().position(|h| hit_matches(h, ev))?;
        worst_rank = worst_rank.max(rank + 1);
    }
    (worst_rank > 0).then(|| 1.0 / worst_rank as f32)
}

fn keywords_match(answer: &str, keywords: &[String]) -> bool {
    let lower = answer.to_lowercase();
    keywords.iter().all(|k| lower.contains(&k.to_lowercase()))
}

#[derive(Default)]
struct Bucket {
    total: usize,
    hits: usize,
    rr_sum: f32,
    correct: usize,
}

fn print_score_range(label: &str, scores: &[f32]) {
    if scores.is_empty() {
        return;
    }
    let min = scores.iter().copied().fold(f32::INFINITY, f32::min);
    let max = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let avg = scores.iter().sum::<f32>() / scores.len() as f32;
    println!("  {label:<14} top-1 cosine: min={min:.3} avg={avg:.3} max={max:.3}");
}

#[test]
fn eval_cases_match_fixture_chunks() {
    let cases = load_cases();
    let files = fixture_files();
    assert!(!cases.is_empty());

    let mut chunk_index: Vec<(PathBuf, String)> = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path).unwrap();
        for c in chunk_markdown(&text, Rag::DEFAULT_CHUNK_SIZE) {
            chunk_index.push((path.clone(), c.to_lowercase()));
        }
    }

    let mut ids = HashSet::new();
    for c in &cases {
        assert!(ids.insert(c.id.as_str()), "duplicate case id: {}", c.id);
        assert!(
            !c.keywords.is_empty(),
            "case {} must declare keywords",
            c.id
        );

        if c.category == Category::Unanswerable {
            assert!(
                c.evidence.is_empty(),
                "unanswerable case {} must have empty evidence",
                c.id
            );
            continue;
        }

        assert!(
            !c.evidence.is_empty(),
            "answerable case {} must declare evidence",
            c.id
        );

        for ev in &c.evidence {
            let needle = ev.snippet.to_lowercase();
            let exists = chunk_index
                .iter()
                .any(|(p, text)| p.ends_with(&ev.source) && text.contains(&needle));
            assert!(
                exists,
                "case {}: snippet {:?} not found in any chunk of {}",
                c.id, ev.snippet, ev.source
            );
        }

        if c.category == Category::CrossChunk {
            assert!(
                c.evidence.len() >= 2,
                "cross_chunk case {} needs at least 2 evidence items",
                c.id
            );
            let single_chunk_covers_all = chunk_index.iter().any(|(p, text)| {
                c.evidence
                    .iter()
                    .all(|ev| p.ends_with(&ev.source) && text.contains(&ev.snippet.to_lowercase()))
            });
            assert!(
                !single_chunk_covers_all,
                "cross_chunk case {} is satisfied by a single chunk",
                c.id
            );
        }
    }
}

#[test]
fn reciprocal_rank_scores_single_and_multi_chunk_cases() {
    use rag_cli::Chunk;
    use std::path::Path;

    let hit = |source: &str, text: &str, score: f32| Hit {
        score,
        chunk: Chunk {
            source: Path::new(source).into(),
            text: text.into(),
        },
    };

    let hits = vec![
        hit("a.md", "irrelevant header", 0.9),
        hit("a.md", "pipe handles backpressure", 0.8),
        hit("b.md", "leaves streams open on error", 0.7),
    ];

    // Wrong chunk in the right file does not count as a hit.
    let single = vec![Evidence {
        source: "a.md".into(),
        snippet: "pipe handles".into(),
    }];
    assert_eq!(reciprocal_rank(&hits, &single), Some(0.5));

    // Cross-chunk uses the worst rank needed to collect all evidence.
    let multi = vec![
        Evidence {
            source: "a.md".into(),
            snippet: "pipe handles".into(),
        },
        Evidence {
            source: "b.md".into(),
            snippet: "leaves streams open".into(),
        },
    ];
    assert_eq!(reciprocal_rank(&hits, &multi), Some(1.0 / 3.0));

    let missed = vec![Evidence {
        source: "a.md".into(),
        snippet: "not present".into(),
    }];
    assert_eq!(reciprocal_rank(&hits, &missed), None);
}

#[tokio::test]
#[ignore]
async fn eval_retrieval() {
    let files = fixture_files();
    let cases = load_cases();
    let rag: Rag = Rag::ingest(&files).await.unwrap();
    println!(
        "Corpus: {} files, {} chunks (k={K}, chunk_size={})\n",
        files.len(),
        rag.chunk_count(),
        Rag::DEFAULT_CHUNK_SIZE
    );

    let mut by_cat: BTreeMap<Category, Bucket> = BTreeMap::new();
    let mut ans_scores = Vec::new();
    let mut unans_scores = Vec::new();

    for c in &cases {
        let hits = rag.retrieve(&c.q, K).await.unwrap();
        let top1 = hits.first().map(|h| h.score).unwrap_or(0.0);

        if c.category == Category::Unanswerable {
            unans_scores.push(top1);
            let top_src = hits
                .first()
                .map(|h| h.chunk.source.display().to_string())
                .unwrap_or_default();
            println!("UNANS | top1={top1:.3} ({top_src}) | [{}] {}", c.id, c.q);
            continue;
        }

        ans_scores.push(top1);
        let rr = reciprocal_rank(&hits, &c.evidence);
        let bucket = by_cat.entry(c.category).or_default();
        bucket.total += 1;
        if let Some(r) = rr {
            bucket.hits += 1;
            bucket.rr_sum += r;
        }

        println!(
            "{} rr={:.2} top1={top1:.3} | [{}] {}",
            if rr.is_some() { "R✓" } else { "R✗" },
            rr.unwrap_or(0.0),
            c.id,
            c.q
        );
        if rr.is_none() {
            for (i, h) in hits.iter().enumerate() {
                let preview: String = h.chunk.text.replace('\n', " ").chars().take(80).collect();
                println!(
                    "   [{}] {:.3} {} :: {preview}",
                    i + 1,
                    h.score,
                    h.chunk.source.display()
                );
            }
        }
    }

    let (mut total, mut hits, mut rr_sum) = (0usize, 0usize, 0.0f32);
    println!("\nRetrieval breakdown (answerable only):");
    for (cat, b) in &by_cat {
        total += b.total;
        hits += b.hits;
        rr_sum += b.rr_sum;
        println!(
            "  {:<14} recall@{K} = {}/{} ({:.0}%), MRR = {:.3}",
            cat.label(),
            b.hits,
            b.total,
            100.0 * b.hits as f32 / b.total as f32,
            b.rr_sum / b.total as f32
        );
    }

    let recall = hits as f32 / total as f32;
    let mrr = rr_sum / total as f32;
    println!(
        "  {:<14} recall@{K} = {hits}/{total} ({:.0}%), MRR = {mrr:.3}",
        "OVERALL",
        100.0 * recall
    );
    print_score_range("answerable", &ans_scores);
    print_score_range("unanswerable", &unans_scores);

    assert!(
        recall >= 0.80,
        "chunk-level recall@{K} ({recall:.2}) below 0.80"
    );
}

#[tokio::test]
#[ignore]
async fn eval_e2e() {
    let files = fixture_files();
    let cases = load_cases();
    let rag: Rag = Rag::ingest(&files).await.unwrap();
    println!(
        "Corpus: {} files, {} chunks (k={K}, chunk_size={})\n",
        files.len(),
        rag.chunk_count(),
        Rag::DEFAULT_CHUNK_SIZE
    );

    let mut by_cat: BTreeMap<Category, Bucket> = BTreeMap::new();
    let (mut unans_total, mut unans_abstained) = (0usize, 0usize);

    for c in &cases {
        let a = rag.ask(&c.q, K).await.unwrap();
        let ok = keywords_match(&a.text, &c.keywords);

        if c.category == Category::Unanswerable {
            unans_total += 1;
            unans_abstained += ok as usize;
            println!(
                "ABST {} | [{}] {}\n   → {}",
                if ok { "✓" } else { "✗" },
                c.id,
                c.q,
                a.text
            );
            continue;
        }

        let rr = reciprocal_rank(&a.hits, &c.evidence);
        let bucket = by_cat.entry(c.category).or_default();
        bucket.total += 1;
        if let Some(r) = rr {
            bucket.hits += 1;
            bucket.rr_sum += r;
        }
        bucket.correct += ok as usize;

        println!(
            "{} {} | [{}] {}\n   → {}",
            if rr.is_some() { "R✓" } else { "R✗" },
            if ok { "A✓" } else { "A✗" },
            c.id,
            c.q,
            a.text
        );
    }

    let (mut total, mut hits, mut rr_sum, mut correct) = (0usize, 0usize, 0.0f32, 0usize);
    println!("\nEnd-to-end breakdown:");
    for (cat, b) in &by_cat {
        total += b.total;
        hits += b.hits;
        rr_sum += b.rr_sum;
        correct += b.correct;
        println!(
            "  {:<14} recall@{K} = {}/{}, MRR = {:.3}, accuracy = {}/{}",
            cat.label(),
            b.hits,
            b.total,
            b.rr_sum / b.total as f32,
            b.correct,
            b.total
        );
    }

    let recall = hits as f32 / total as f32;
    let mrr = rr_sum / total as f32;
    let accuracy = correct as f32 / total as f32;
    let abstention = unans_abstained as f32 / unans_total as f32;
    println!(
        "  {:<14} recall@{K} = {hits}/{total} ({:.0}%), MRR = {mrr:.3}, accuracy = {correct}/{total} ({:.0}%)",
        "ANSWERABLE",
        100.0 * recall,
        100.0 * accuracy
    );
    println!(
        "  {:<14} abstention = {unans_abstained}/{unans_total} ({:.0}%)",
        "UNANSWERABLE",
        100.0 * abstention
    );

    assert!(
        recall >= 0.80,
        "chunk-level recall@{K} ({recall:.2}) below 0.80"
    );
    assert!(
        accuracy >= 0.80,
        "answerable accuracy ({accuracy:.2}) below 0.80"
    );
    assert!(
        abstention >= 0.80,
        "unanswerable abstention ({abstention:.2}) below 0.80"
    );
}
