//! Empirical sweeps of `chunk_size` and `k` over plaintext IETF RFCs in `tests/fixtures_plain`.
//!
//! The `chunk_size` and `k` sweeps use `Plain`, a paragraph packer with no notion of headings
//! or code, isolating chunk size and `k` from any structure-aware chunking. The chunker sweep
//! then compares `Plain` against `Rfc`, which splits on the RFCs' section headings.
//!
//! - Offline self-checks: `cargo test --test sweep`
//! - Chunk-size sweep:    `cargo test --test sweep sweep_chunk_size -- --ignored --nocapture`
//! - Top-k sweep:         `cargo test --test sweep sweep_k -- --ignored --nocapture`
//! - Plain vs Rfc sweep:  `cargo test --test sweep sweep_chunker -- --ignored --nocapture`

use std::{collections::HashSet, fmt::Write as _, path::PathBuf, time::Instant};

use rag_cli::{
    Chunker, Document, Hit, Markdown, OpenAiClient, Plain, Rag, Rfc, chunk_documents, index,
};
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

/// Retrieval quality of the top `k` hits per case.
struct Score {
    single_hits: usize,
    single_total: usize,
    cross_hits: usize,
    cross_total: usize,
    mrr: f32,
    avg_prompt_chars: usize,
}

impl Score {
    /// Scores the first `k` of each case's `retrieved` hits (ranked best first).
    fn at_k(cases: &[Case], retrieved: &[Vec<Hit>], k: usize) -> Self {
        let mut s = Score {
            single_hits: 0,
            single_total: 0,
            cross_hits: 0,
            cross_total: 0,
            mrr: 0.0,
            avg_prompt_chars: 0,
        };
        let (mut rr_sum, mut prompt_chars) = (0.0f32, 0usize);

        for (c, hits) in cases.iter().zip(retrieved) {
            let hits = &hits[..k.min(hits.len())];
            prompt_chars += hits
                .iter()
                .map(|h| h.chunk.text.chars().count())
                .sum::<usize>();

            let rr = reciprocal_rank(hits, &c.evidence);
            let (hit, total) = if c.cross_chunk {
                (&mut s.cross_hits, &mut s.cross_total)
            } else {
                (&mut s.single_hits, &mut s.single_total)
            };
            *total += 1;
            *hit += rr.is_some() as usize;
            rr_sum += rr.unwrap_or(0.0);
        }

        s.mrr = rr_sum / cases.len() as f32;
        s.avg_prompt_chars = prompt_chars / cases.len();
        s
    }

    fn overall_hits(&self) -> usize {
        self.single_hits + self.cross_hits
    }

    fn overall_total(&self) -> usize {
        self.single_total + self.cross_total
    }
}

/// Fraction as a percentage, or 0 when there is nothing to count.
fn pct(hits: usize, total: usize) -> f32 {
    if total == 0 {
        0.0
    } else {
        100.0 * hits as f32 / total as f32
    }
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

    std::fs::create_dir_all("experiments/01-chunk-size").unwrap();
    std::fs::write("experiments/01-chunk-size/chunk_size.csv", csv).unwrap();
    println!("\nSaved experiments/01-chunk-size/chunk_size.csv");
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
        let s = Score::at_k(&cases, &retrieved, k);
        println!(
            "| {k:>2} | {:>4}/{:<3} | {:>3}/{:<3} | {:>2}/{:<2} {:>3.0}% | {:>5.3} | {:>16} |",
            s.single_hits,
            s.single_total,
            s.cross_hits,
            s.cross_total,
            s.overall_hits(),
            s.overall_total(),
            pct(s.overall_hits(), s.overall_total()),
            s.mrr,
            s.avg_prompt_chars
        );
        writeln!(
            csv,
            "{chunk_size},{k},{},{},{},{:.4},{},{},{:.4},{},{},{:.4},{:.4},{}",
            rag.chunk_count(),
            s.single_hits,
            s.single_total,
            pct(s.single_hits, s.single_total) / 100.0,
            s.cross_hits,
            s.cross_total,
            pct(s.cross_hits, s.cross_total) / 100.0,
            s.overall_hits(),
            s.overall_total(),
            pct(s.overall_hits(), s.overall_total()) / 100.0,
            s.mrr,
            s.avg_prompt_chars
        )
        .unwrap();
    }

    std::fs::create_dir_all("experiments/02-top-k").unwrap();
    std::fs::write("experiments/02-top-k/top_k.csv", csv).unwrap();
    println!("\nSaved experiments/02-top-k/top_k.csv");
}

/// Every evidence snippet must sit inside one `Rfc` chunk at the default size too, so a
/// miss in the chunker sweep is a retrieval miss, not a snippet split by the chunker.
#[test]
fn sweep_cases_fit_rfc_chunks() {
    let chunker = Rfc {
        max_chars: DEFAULT_SWEEP_K_CHUNK_SIZE,
    };
    let mut chunk_index: Vec<(PathBuf, String)> = Vec::new();
    for path in plaintext_files() {
        let text = std::fs::read_to_string(&path).unwrap();
        for c in chunker.chunk(&text) {
            chunk_index.push((path.clone(), c.to_lowercase()));
        }
    }
    for c in load_cases() {
        for ev in &c.evidence {
            let needle = ev.snippet.to_lowercase();
            assert!(
                chunk_index
                    .iter()
                    .any(|(p, text)| p.ends_with(&ev.source) && text.contains(&needle)),
                "case {}: snippet {:?} not inside one Rfc chunk at chunk_size={DEFAULT_SWEEP_K_CHUNK_SIZE}",
                c.id,
                ev.snippet
            );
        }
    }
}

/// Embeds `docs` split by `chunker` and retrieves the top `max_k` hits for every case.
async fn retrieve_all(
    docs: &[Document],
    chunker: &impl Chunker,
    cases: &[Case],
    max_k: usize,
) -> (usize, usize, Vec<Vec<Hit>>) {
    let chunks = chunk_documents(docs, chunker);
    let avg_chunk_chars =
        chunks.iter().map(|c| c.text.chars().count()).sum::<usize>() / chunks.len();
    let client = OpenAiClient::from_env().unwrap();
    let rag: Rag = Rag::ingest(client.clone(), client, chunks).await.unwrap();

    let mut retrieved = Vec::with_capacity(cases.len());
    for c in cases {
        retrieved.push(rag.retrieve(&c.q, max_k).await.unwrap());
    }
    (rag.chunk_count(), avg_chunk_chars, retrieved)
}

/// `Plain` vs `Rfc` over every `chunk_size` and `k`: each (chunker, chunk_size) is embedded
/// once and retrieved at the largest `k`, then every smaller `k` is scored in memory.
#[tokio::test]
#[ignore]
async fn sweep_chunker() {
    const SHOWN_K: &[usize] = &[3, 5];
    let max_k = *KS.last().unwrap();
    let files = plaintext_files();
    let cases = load_cases();
    let docs = Document::from_files(&files).await.unwrap();

    println!(
        "Plaintext corpus: {} files, {} cases, chunk_size x k grid per chunker\n",
        files.len(),
        cases.len()
    );
    println!(
        "| chunker | chunk_size | chunks | avg chunk |  k | single@k | cross@k | overall@k |   MRR | avg prompt chars |"
    );
    println!(
        "|---------|-----------:|-------:|----------:|---:|---------:|--------:|----------:|------:|-----------------:|"
    );

    let mut csv = String::from(
        "chunker,chunk_size,k,chunks,avg_chunk_chars,single_hits,single_total,single_recall,cross_hits,cross_total,cross_recall,overall_hits,overall_total,overall_recall,mrr,avg_prompt_chars\n",
    );

    for name in ["plain", "rfc"] {
        for &size in CHUNK_SIZES {
            let (chunks, avg_chunk, retrieved) = match name {
                "plain" => retrieve_all(&docs, &Plain { max_chars: size }, &cases, max_k).await,
                _ => retrieve_all(&docs, &Rfc { max_chars: size }, &cases, max_k).await,
            };

            for &k in KS {
                let s = Score::at_k(&cases, &retrieved, k);
                if SHOWN_K.contains(&k) {
                    println!(
                        "| {name:<7} | {size:>10} | {chunks:>6} | {avg_chunk:>9} | {k:>2} | {:>4}/{:<3} | {:>3}/{:<3} | {:>2}/{:<2} {:>3.0}% | {:>5.3} | {:>16} |",
                        s.single_hits,
                        s.single_total,
                        s.cross_hits,
                        s.cross_total,
                        s.overall_hits(),
                        s.overall_total(),
                        pct(s.overall_hits(), s.overall_total()),
                        s.mrr,
                        s.avg_prompt_chars
                    );
                }
                writeln!(
                    csv,
                    "{name},{size},{k},{chunks},{avg_chunk},{},{},{:.4},{},{},{:.4},{},{},{:.4},{:.4},{}",
                    s.single_hits,
                    s.single_total,
                    pct(s.single_hits, s.single_total) / 100.0,
                    s.cross_hits,
                    s.cross_total,
                    pct(s.cross_hits, s.cross_total) / 100.0,
                    s.overall_hits(),
                    s.overall_total(),
                    pct(s.overall_hits(), s.overall_total()) / 100.0,
                    s.mrr,
                    s.avg_prompt_chars
                )
                .unwrap();
            }
        }
    }

    std::fs::create_dir_all("experiments/03-chunker").unwrap();
    std::fs::write("experiments/03-chunker/chunker.csv", csv).unwrap();
    println!("\nSaved experiments/03-chunker/chunker.csv");
}
