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
let rag = rag_cli::ingest(&["notes/backpressure.md".into()]).await?;
let answer = rag_cli::ask(&rag, "What is backpressure?", 3).await?;
println!("{}\nfrom: {:?}", answer.text, answer.sources);
```

Use `ingest_with` to pick a chunk size other than `CHUNK_SIZE` (1500).

## Tests

```sh
cargo test                                          # unit tests, offline
cargo test --test eval -- --ignored --nocapture     # eval, calls OpenAI
```

The eval ingests the notes in `tests/fixtures` and asks six questions: four
answered by a single note (`RAG and Agents.md` or `backpressure.md`), one whose
answer spans two chunks (`backpressure.md` and `node streams.md`), and one
deliberately not covered by the notes. The other fixtures act as distractors
that share vocabulary with the questions (streams, vectors, protocols,
PostgreSQL) so retrieval has to rank the right files above near misses. It
reports:

- **recall@3**: whether every expected file was among the top 3 chunks
- **accuracy**: whether the answer contains the expected keywords

It fails unless recall is 6/6 and accuracy at least 5/6. To compare settings,
change one thing (chunk size, `k`, the prompt), re-run and compare the scores.

### What the eval does not prove

Passing 6/6 here is a sanity check against regressions, not proof that the
pipeline works well in general:

- **Corpus and sample size are tiny.** The seven fixtures split into only 11
  chunks at the default chunk size, so `k = 3` hands the model ~27% of the
  entire index on every question. Six hand-picked questions have high variance
  and say nothing about retrieval quality across hundreds or thousands of
  chunks.
- **Recall is file-level and generous.** `Answer.sources` only tracks file
  paths, not which chunk inside a file was returned — retrieving the wrong
  chunk of `RAG and Agents.md` (even its two-line `tags::` header) still counts
  as `R✓`. The unanswerable case also gets an automatic `R✓` because it checks
  no sources, and `recall@3` ignores ranking order (rank 1 and rank 3 score the
  same).
- **Keywords do not measure faithfulness or correctness.** Substring checks miss
  negations, hallucinations tacked onto a matching word, and wrong or missing
  `[n]` citations, while valid paraphrases fail. Except for private notes like
  `caipora`, `gpt-4o-mini` already knows most of these topics from pretraining,
  so a keyword hit does not prove the answer came from the retrieved context.
- **Heading-based chunking is barely exercised by the fixtures.** All fixture
  notes use Logseq-style bulleted headings (`- ## ...`) with no blank lines
  between sections, which `chunk::chunks` does not treat as headings. As a
  result, six fixtures are a single chunk each and `RAG and Agents.md` falls
  back to raw 1500-char cuts mid-word. Passing the eval does not validate
  Markdown-structure chunking on real long notes or intra-document boundaries.
- **No generalization or operational bounds.** The questions were written
  looking at the fixtures, run once (hosted LLMs are not strictly deterministic
  even at temperature 0), and check nothing about latency, token cost, or
  vocabulary mismatch between a user's phrasing and their notes.

Other API-calling tests (latency measurements for embed and chat) are also
`#[ignore]`d and run with `cargo test -- --ignored`.
