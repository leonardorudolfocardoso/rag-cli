# rag-cli

Stateless command-line Q&A (and Rust library) over local UTF-8 text files
(Markdown/Logseq notes, plaintext, IETF RFCs). Given a question and a list of
files, it chunks and embeds them in memory, retrieves the closest chunks, and
asks a chat model to answer from those chunks alone with `[n]` citations.

## Usage

Requires Rust (edition 2024) and an OpenAI API key.

```sh
export OPENAI_API_KEY=sk-...
cargo run --release -- --chunker markdown --question "What is backpressure?" notes/*.md
```

| Option | Default | Meaning |
| --- | --- | --- |
| `-q, --question` | required | The question to answer |
| `-k, --k` | 5 | How many chunks go into the prompt |
| `-c, --chunk-size` | 1100 | Maximum chunk size, in chars |
| `--chunker` | plain | `plain` (paragraphs only), `markdown` (headings, fenced code) or `rfc` (IETF RFC sections) |

If the answer isn't in the files, the model is told to say "I don't know".

## How it works

1. **Chunk**: paragraphs are packed into chunks of up to `--chunk-size` chars,
   and `--chunker` decides where else a chunk must break. `plain` only packs
   paragraphs; `markdown` also starts a chunk at each `#` heading (including
   Logseq `- ## ` bullets) and never splits fenced code at blank lines (only a
   single block exceeding `--chunk-size` is cut on char boundaries); `rfc`
   starts a chunk at each section heading, recognised from the page layout.
   Chunks don't overlap.
2. **Embed**: chunks are embedded with `text-embedding-3-small` in batches of
   32 (at most 4 requests in flight), and normalised so cosine similarity is a
   dot product.
3. **Retrieve**: the question is embedded the same way and an exact in-memory
   scan (`VectorStore`) picks the `k` most similar chunks.
4. **Answer**: the chunks, numbered and labelled with their file, go to
   `gpt-4o-mini` (temperature 0) with the question.

Rate limits, server errors, connection/body errors, and timeouts are retried
with exponential backoff (up to 4 total attempts, 500ms initial wait, 30s
per-request timeout). Nothing is cached, so every run embeds the files again.

## Library

The same pipeline is available from the crate:

```rust
let client = rag_cli::OpenAiClient::from_env()?;
let chunker = rag_cli::Markdown { max_chars: rag_cli::Rag::DEFAULT_MAX_CHUNK_CHARS };
let rag: rag_cli::Rag =
    rag_cli::index(client.clone(), client, &["notes/backpressure.md".into()], &chunker).await?;
let hits = rag.retrieve("What is backpressure?", 3).await?;
let answer = rag.ask("What is backpressure?", 3).await?;
println!("{}\nfrom: {:?} ({} hits)", answer.text, answer.sources(), hits.len());
```

`index` runs the whole pipeline. Its steps are also public: `Document::from_files` reads files, `chunk_documents` splits them with any `Chunker` (`Markdown`, `Plain`, `Rfc`), and `Rag::ingest` embeds the chunks; `Rag::DEFAULT_MAX_CHUNK_CHARS` (1100) is the CLI default chunk size.

## Evaluation

Retrieval sweeps run on 3 complete IETF RFCs (`tests/fixtures_plain`, 95,595
chars, 20 questions: 16 single-chunk + 4 cross-chunk, `text-embedding-3-small`).
On plaintext RFCs, `Markdown` produces identical chunks to `Plain` (verified
offline by `plain_chunker_matches_markdown_on_rfcs`).

Experiment 3 (`Plain` / `Markdown` vs `Rfc`, `k = 3`, from [chunker.csv](experiments/03-chunker/chunker.csv)):

| `chunk_size` | `Plain` / `Markdown` `overall@3` | `Rfc` `overall@3` | `Plain` / `Markdown` `MRR` | `Rfc` `MRR` | `Plain` / `Markdown` prompt chars | `Rfc` prompt chars |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 200 | 9/20 (45%) | 10/20 (50%) | 0.258 | 0.308 | 496 | 494 |
| 400 | 12/20 (60%) | 15/20 (75%) | 0.525 | 0.533 | 1,000 | 883 |
| 800 | 15/20 (75%) | 16/20 (80%) | 0.567 | 0.658 | 2,065 | 1,745 |
| **1100** | **17/20 (85%)** | 16/20 (80%) | **0.658** | 0.692 | 2,938 | 2,455 |
| 1500 | 17/20 (85%) | 16/20 (80%) | 0.633 | 0.692 | 4,120 | 2,944 |
| 2200 | 14/20 (70%) | 16/20 (80%) | 0.558 | 0.725 | 6,266 | 3,634 |
| 3000 | 15/20 (75%) | 16/20 (80%) | 0.583 | 0.742 | 8,467 | 4,611 |
| **6000** | 15/20 (75%) | **17/20 (85%)** | 0.558 | **0.767** | 17,303 | 5,276 |

- At the CLI default (`chunk_size = 1100`, `k = 5`): `Plain` / `Markdown` reaches `18/20` (90%, `MRR 0.668`, `4,916` prompt chars) and `Rfc` reaches `17/20` (85%, `MRR 0.704`, `3,982` prompt chars).
- Sweeping `k` at `chunk_size = 1100` ([top_k.csv](experiments/02-top-k/top_k.csv)): `Plain` reaches `20/20` (100%, `4/4` cross-chunk, `MRR 0.684`) at `k = 7` (`6,877` prompt chars); `Rfc` ranks single-chunk evidence higher (`single@1` up to `14/16` vs `10/16`) but is weaker on cross-chunk recall, reaching `20/20` only at `chunk_size = 2200–3000`, `k = 10`.
- Full CSVs, charts, and metric definitions live in [experiments/README.md](experiments/README.md).
- Separately, `tests/eval.rs` runs `Markdown` (`chunk_size = 1100`, `k = 3`) over 18 Markdown notes (`tests/fixtures`, 28 cases: 23 answerable, 5 unanswerable) as an `>= 80%` regression gate on recall@3, keyword accuracy, and abstention.

## Tests

```sh
cargo test                                                        # unit tests, offline
cargo test --test eval eval_retrieval -- --ignored --nocapture    # retrieval only (embeddings)
cargo test --test eval eval_e2e -- --ignored --nocapture          # full pipeline (embeddings + chat)
cargo test --test sweep sweep_chunk_size -- --ignored --nocapture # chunk_size curve on plaintext RFCs
cargo test --test sweep sweep_k -- --ignored --nocapture          # top-k curve on plaintext RFCs
cargo test --test sweep sweep_chunker -- --ignored --nocapture    # Plain vs Rfc chunker on plaintext RFCs
```

The eval ingests the Markdown notes in `tests/fixtures` and runs the cases in
`tests/eval_cases.json` across five categories: `direct`, `paraphrase`,
`cross_chunk` (both cross-file and intra-file), `private` (fictional project
notes not in model pretraining), and `unanswerable` near-misses.

- **`eval_retrieval`** calls only the embeddings API and measures chunk-level
  **recall@3** and **MRR** on answerable cases (verifying both the file and an
  expected evidence snippet inside the returned chunk), while reporting the
  top-1 cosine score distribution for unanswerable cases separately.
- **`eval_e2e`** runs the full pipeline and reports chunk-level **recall@3**,
  **MRR**, keyword **accuracy** on answerable cases, and **abstention** rate on
  unanswerable cases separately.

To compare settings, change one thing (chunk size, `k`, chunker, prompt),
re-run `eval_retrieval` or `eval_e2e`, and compare the breakdown by category.
Other API-calling tests (latency measurements for embed and chat) are also
`#[ignore]`d and run with `cargo test -- --ignored`.

## Limits

### Project limits

- **Stateless and in-memory only:** every run re-reads and re-embeds all input
  files with no disk cache or persistent index, and `VectorStore::top_k` does an
  exact linear scan—intended for small local document sets, not large corpora.
- **Dense retrieval only:** no lexical/BM25 hybrid search or reranking, so
  queries relying on exact identifiers, rare symbols, or negation inherit
  bi-encoder blind spots.
- **Simple chunking:** no chunk overlap; split subsections lose their parent
  heading context; `Rfc` leaves page headers/footers inside chunks and falls
  back to `Plain` when body text is not indented.
- **Narrow I/O and fixed models:** UTF-8 text files only (no PDF/HTML/binary
  parsing), single-shot Q&A only (no conversation history), hardcoded OpenAI
  models (`text-embedding-3-small`, `gpt-4o-mini`), and no token-budget cap on
  `k * chunk_size`.

### What the eval does not prove

Even with chunk-level recall, MRR, separated abstention, and plaintext sweeps,
passing these tests is a regression guard rather than full proof of quality:

- **Keywords do not measure faithfulness or citation accuracy.** Substring
  checks still miss negations, extra unsupported claims tacked onto a matching
  word, and wrong or missing `[n]` citations, while valid paraphrases can fail.
  Catching groundedness and citation errors requires a semantic judge (e.g.
  LLM-as-a-judge) on top of the retrieved chunks.
- **Synthetic fixtures are cleaner than messy real notes.** The fixtures use
  short, well-structured Markdown or Logseq sections that fit under the 1100-char
  default chunk size (only two sections in `RAG and Agents.md` exceed it), so
  long unheaded prose walls, messy tables, and mid-section splits are barely
  exercised.
- **RFC sweeps are small and retrieval-only.** 3 RFCs and 20 questions (only 4
  cross-chunk, so 1 case moves `overall` by 5 percentage points) measure
  retrieval only—no LLM answer quality or abstention—with `~0.01` MRR run-to-run
  jitter on near-ties, and 2 snippets split across `Rfc` boundaries at
  `chunk_size` `200–400`.
- **Scale, variance and operational bounds.** ~100 Markdown chunks and 28
  questions are enough to see category-level trends, not to characterize
  behavior across thousands of documents. Hosted LLMs are not strictly
  deterministic even at temperature 0, each case runs once, and latency/cost are
  not asserted.
