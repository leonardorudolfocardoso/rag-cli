//! Empirical sweeps of `chunk_size` and `k` over plaintext IETF RFCs in `tests/fixtures_plain`.
//!
//! Chunks come from `Plain`, a paragraph packer with no notion of headings or code,
//! isolating chunk size and `k` from any structure-aware chunking.
//!
//! - Offline self-checks: `cargo test --test sweep`
//! - Chunk-size sweep:    `cargo test --test sweep sweep_chunk_size -- --ignored --nocapture`
//! - Top-k sweep:         `cargo test --test sweep sweep_k -- --ignored --nocapture`

use std::{collections::HashSet, fmt::Write as _, path::PathBuf, time::Instant};

use rag_cli::{Chunker, Document, Hit, Markdown, OpenAiClient, Plain, Rag, chunk_documents, index};
use serde::Deserialize;

const CASES_JSON: &str = include_str!("sweep_cases.json");
const CHUNK_SIZES: &[usize] = &[200, 400, 800, 1100, 1500, 2200, 3000, 6000];
const KS: &[usize] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
const DEFAULT_SWEEP_K_CHUNK_SIZE: usize = 1100;

/// A chunk that retrieval must surface for a sweep case, matched by filename
/// and a case-insensitive single-line substring inside that chunk.
#[derive(Debug, Deserialize)]
struct Evidence {
    source: String,
    snippet: String,
}

#[derive(Debug, Deserialize)]
struct Case {
    id: String,
    cross_chunk: bool,
    q: String,
    evidence: Vec<Evidence>,
}

fn load_cases() -> Vec<Case> {
    serde_json::from_str(CASES_JSON).expect("tests/sweep_cases.json must be valid JSON")
}

fn plaintext_files() -> Vec<PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir("tests/fixtures_plain")
        .unwrap()
        .map(|e| e.unwrap().path())
        // Skip non-text files (e.g. macOS `.DS_Store`).
        .filter(|p| p.extension().is_some_and(|ext| ext == "txt"))
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

/// Returns `Some(1.0 / worst_rank)` (1-based) if every required evidence snippet
/// appears in `hits`, or `None` if any piece was missed.
fn reciprocal_rank(hits: &[Hit], evidence: &[Evidence]) -> Option<f32> {
    let mut worst_rank = 0usize;
    for ev in evidence {
        let rank = hits.iter().position(|h| hit_matches(h, ev))?;
        worst_rank = worst_rank.max(rank + 1);
    }
    (worst_rank > 0).then(|| 1.0 / worst_rank as f32)
}

/// The earlier sweeps ran `Markdown` on these RFCs; `Plain` must reproduce those chunks.
#[test]
fn plain_chunker_matches_markdown_on_rfcs() {
    for path in plaintext_files() {
        let text = std::fs::read_to_string(&path).unwrap();
        for &size in CHUNK_SIZES {
            assert_eq!(
                Plain { max_chars: size }.chunk(&text),
                Markdown { max_chars: size }.chunk(&text),
                "{} at chunk_size={size}",
                path.display()
            );
        }
    }
}

#[test]
fn sweep_cases_match_plaintext_fixtures() {
    let cases = load_cases();
    let files = plaintext_files();
    assert!(!cases.is_empty());
    assert!(!files.is_empty());

    let mut chunk_index: Vec<(PathBuf, String)> = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path).unwrap();
        let chunker = Plain {
            max_chars: DEFAULT_SWEEP_K_CHUNK_SIZE,
        };
        for c in chunker.chunk(&text) {
            chunk_index.push((path.clone(), c.to_lowercase()));
        }
    }

    let mut ids = HashSet::new();
    for c in &cases {
        assert!(ids.insert(c.id.as_str()), "duplicate case id: {}", c.id);
        assert!(!c.evidence.is_empty(), "case {} needs evidence", c.id);

        for ev in &c.evidence {
            let needle = ev.snippet.to_lowercase();
            let exists = chunk_index
                .iter()
                .any(|(p, text)| p.ends_with(&ev.source) && text.contains(&needle));
            assert!(
                exists,
                "case {}: snippet {:?} not found in {} at chunk_size={DEFAULT_SWEEP_K_CHUNK_SIZE}",
                c.id, ev.snippet, ev.source
            );
        }

        if c.cross_chunk {
            assert!(
                c.evidence.len() >= 2,
                "cross_chunk case {} needs >= 2 evidence items",
                c.id
            );
            let single_covers_all = chunk_index.iter().any(|(p, text)| {
                c.evidence
                    .iter()
                    .all(|ev| p.ends_with(&ev.source) && text.contains(&ev.snippet.to_lowercase()))
            });
            assert!(
                !single_covers_all,
                "cross_chunk case {} fits in a single chunk at chunk_size={DEFAULT_SWEEP_K_CHUNK_SIZE}",
                c.id
            );
        }
    }
}

#[tokio::test]
#[ignore]
async fn sweep_chunk_size() {
    const K: usize = 3;
    let files = plaintext_files();
    let cases = load_cases();
    let total_chars: usize = files
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap().chars().count())
        .sum();

    println!(
        "Plaintext corpus: {} files, {} chars, {} cases (fixed k={K})\n",
        files.len(),
        total_chars,
        cases.len()
    );
    println!(
        "| chunk_size | chunks | single@{K} | overall@{K} |   MRR | avg prompt chars | ingest |"
    );
    println!("|-----------:|-------:|---------:|----------:|------:|-----------------:|-------:|");

    let mut csv = String::from(
        "chunk_size,k,chunks,single_hits,single_total,single_recall,overall_hits,overall_total,overall_recall,mrr,avg_prompt_chars,ingest_secs\n",
    );

    let docs = Document::from_files(&files).await.unwrap();
    for &size in CHUNK_SIZES {
        let t0 = Instant::now();
        let client = OpenAiClient::from_env().unwrap();
        let rag: Rag = Rag::ingest(
            client.clone(),
            client,
            chunk_documents(&docs, &Plain { max_chars: size }),
        )
        .await
        .unwrap();
        let ingest_s = t0.elapsed().as_secs_f32();

        let (mut single_total, mut single_hits) = (0usize, 0usize);
        let (mut all_hits, mut rr_sum, mut prompt_chars) = (0usize, 0.0f32, 0usize);

        for c in &cases {
            let hits = rag.retrieve(&c.q, K).await.unwrap();
            prompt_chars += hits
                .iter()
                .map(|h| h.chunk.text.chars().count())
                .sum::<usize>();

            let rr = reciprocal_rank(&hits, &c.evidence);
            if !c.cross_chunk {
                single_total += 1;
                single_hits += rr.is_some() as usize;
            }
            if let Some(r) = rr {
                all_hits += 1;
                rr_sum += r;
            }
        }

        let single_recall = single_hits as f32 / single_total as f32;
        let all_recall = all_hits as f32 / cases.len() as f32;
        let mrr = rr_sum / cases.len() as f32;
        let avg_prompt = prompt_chars / cases.len();

        println!(
            "| {size:>10} | {:>6} | {:>2}/{:<2} {:>3.0}% | {:>2}/{:<2} {:>3.0}% | {mrr:>5.3} | {avg_prompt:>16} | {ingest_s:>5.1}s |",
            rag.chunk_count(),
            single_hits,
            single_total,
            100.0 * single_recall,
            all_hits,
            cases.len(),
            100.0 * all_recall
        );
        writeln!(
            csv,
            "{size},{K},{},{single_hits},{single_total},{single_recall:.4},{all_hits},{},{all_recall:.4},{mrr:.4},{avg_prompt},{ingest_s:.1}",
            rag.chunk_count(),
            cases.len()
        )
        .unwrap();
    }

    std::fs::create_dir_all("experiments").unwrap();
    std::fs::write("experiments/chunk_size.csv", csv).unwrap();
    println!("\nSaved experiments/chunk_size.csv");
}

#[tokio::test]
#[ignore]
async fn sweep_k() {
    let chunk_size = std::env::var("CHUNK_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_SWEEP_K_CHUNK_SIZE);
    let max_k = *KS.last().unwrap();

    let files = plaintext_files();
    let cases = load_cases();
    let client = OpenAiClient::from_env().unwrap();
    let rag: Rag = index(
        client.clone(),
        client,
        &files,
        &Plain {
            max_chars: chunk_size,
        },
    )
    .await
    .unwrap();

    // Embed each query once at `max_k`, then slice in memory for every k.
    let mut retrieved = Vec::with_capacity(cases.len());
    for c in &cases {
        retrieved.push(rag.retrieve(&c.q, max_k).await.unwrap());
    }

    println!(
        "Plaintext corpus: {} files, {} chunks (chunk_size={chunk_size}), {} cases\n",
        files.len(),
        rag.chunk_count(),
        cases.len()
    );
    println!("|  k | single@k | cross@k | overall@k |   MRR | avg prompt chars |");
    println!("|---:|---------:|--------:|----------:|------:|-----------------:|");

    let mut csv = String::from(
        "chunk_size,k,chunks,single_hits,single_total,single_recall,cross_hits,cross_total,cross_recall,overall_hits,overall_total,overall_recall,mrr,avg_prompt_chars\n",
    );

    for &k in KS {
        let (mut single_total, mut single_hits) = (0usize, 0usize);
        let (mut cross_total, mut cross_hits) = (0usize, 0usize);
        let (mut all_hits, mut rr_sum, mut prompt_chars) = (0usize, 0.0f32, 0usize);

        for (c, full_hits) in cases.iter().zip(&retrieved) {
            let slice = &full_hits[..k.min(full_hits.len())];
            prompt_chars += slice
                .iter()
                .map(|h| h.chunk.text.chars().count())
                .sum::<usize>();

            let rr = reciprocal_rank(slice, &c.evidence);
            if c.cross_chunk {
                cross_total += 1;
                cross_hits += rr.is_some() as usize;
            } else {
                single_total += 1;
                single_hits += rr.is_some() as usize;
            }
            if let Some(r) = rr {
                all_hits += 1;
                rr_sum += r;
            }
        }

        let single_recall = single_hits as f32 / single_total as f32;
        let cross_recall = cross_hits as f32 / cross_total as f32;
        let all_recall = all_hits as f32 / cases.len() as f32;
        let mrr = rr_sum / cases.len() as f32;
        let avg_prompt = prompt_chars / cases.len();

        println!(
            "| {k:>2} | {:>4}/{:<3} | {:>3}/{:<3} | {:>2}/{:<2} {:>3.0}% | {mrr:>5.3} | {avg_prompt:>16} |",
            single_hits,
            single_total,
            cross_hits,
            cross_total,
            all_hits,
            cases.len(),
            100.0 * all_recall
        );
        writeln!(
            csv,
            "{chunk_size},{k},{},{single_hits},{single_total},{single_recall:.4},{cross_hits},{cross_total},{cross_recall:.4},{all_hits},{},{all_recall:.4},{mrr:.4},{avg_prompt}",
            rag.chunk_count(),
            cases.len()
        )
        .unwrap();
    }

    std::fs::create_dir_all("experiments").unwrap();
    std::fs::write("experiments/top_k.csv", csv).unwrap();
    println!("\nSaved experiments/top_k.csv");
}
