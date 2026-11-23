# rag-cli

Answer questions about your Markdown notes from the command line. It embeds the
notes, finds the chunks closest to the question and asks a chat model to answer
from them alone, citing the chunks it used.

## Usage

Requires Rust (edition 2024) and an OpenAI API key.

```sh
export OPENAI_API_KEY=sk-...
cargo run --release -- --question "What is backpressure?" notes/*.md
```

| Option | Default | Meaning |
| --- | --- | --- |
| `-q, --question` | required | The question to answer |
| `-k, --k` | 3 | How many chunks go into the prompt |
| `-c, --chunk-size` | 1500 | Maximum chunk size, in chars |

If the answer isn't in the notes, the model is told to say "I don't know".

## How it works

1. **Chunk**: each file is split on its Markdown structure. Headings start a new
   chunk, paragraphs are packed up to `--chunk-size`, and fenced code is never
   split. Chunks don't overlap.
2. **Embed**: every chunk is embedded with `text-embedding-3-small`, at most 4
   requests at a time, and normalised so cosine similarity is a dot product.
3. **Retrieve**: the question is embedded the same way and the `k` most similar
   chunks are picked.
4. **Answer**: the chunks, numbered and labelled with their file, go to
   `gpt-4o-mini` (temperature 0) with the question.

Rate limits, server errors and timeouts are retried up to 4 times with
exponential backoff. Nothing is cached, so every run embeds the notes again.

## Library

The same pipeline is available from the crate:

```rust
let rag = rag_cli::Rag::ingest(&["notes/backpressure.md".into()]).await?;
let hits = rag.retrieve("What is backpressure?", 3).await?;
let answer = rag.ask("What is backpressure?", 3).await?;
println!("{}\nfrom: {:?} ({} hits)", answer.text, answer.sources(), hits.len());
```

Use `Rag::ingest_with` to pick a chunk size other than `CHUNK_SIZE` (1500).

## Tests

```sh
cargo test                                                        # unit tests, offline
cargo test --test eval eval_retrieval -- --ignored --nocapture    # retrieval only (embeddings)
cargo test --test eval eval_e2e -- --ignored --nocapture          # full pipeline (embeddings + chat)
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

### What the eval still does not prove

Even with a larger corpus, chunk-level recall, MRR and separated abstention,
passing these tests is a regression guard rather than full proof of quality:

- **Keywords do not measure faithfulness or citation accuracy.** Substring
  checks still miss negations, extra unsupported claims tacked onto a matching
  word, and wrong or missing `[n]` citations, while valid paraphrases can fail.
  Catching groundedness and citation errors requires a semantic judge (e.g.
  LLM-as-a-judge) on top of the retrieved chunks.
- **Synthetic fixtures are cleaner than messy real notes.** Most of the added
  fixtures use tidy standard Markdown headings (`## ...`), whereas the two
  Logseq-style notes (`RAG and Agents.md` and `queues and workers.md`) use
  bulleted headings (`- ## ...`) that `chunk::chunks` does not split on yet,
  falling back to raw 1500-char mid-word cuts.
- **Scale, variance and operational bounds.** ~70 chunks and 28 questions are
  enough to see category-level trends, not to characterize behavior across
  thousands of documents. Hosted LLMs are not strictly deterministic even at
  temperature 0, each case runs once, and latency/cost are not asserted.

Other API-calling tests (latency measurements for embed and chat) are also
`#[ignore]`d and run with `cargo test -- --ignored`.
