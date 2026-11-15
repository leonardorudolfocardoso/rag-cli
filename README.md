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
| `-o, --overlap` | 200 | Longest paragraph repeated across a chunk boundary, in chars |

If the answer isn't in the notes, the model is told to say "I don't know".

## How it works

1. **Chunk**: each file is split on its Markdown structure. Headings start a new
   chunk, paragraphs are packed up to `--chunk-size`, and fenced code is never
   split. A short paragraph at a cut is repeated in the next chunk as overlap.
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

Use `ingest_with` and `ChunkOpts` to change the chunk size and overlap.

## Tests

```sh
cargo test                                          # unit tests, offline
cargo test --test eval -- --ignored --nocapture     # eval, calls OpenAI
```

The eval ingests the notes in `tests/fixtures` and asks five questions, one of
them deliberately not covered by the notes. It reports:

- **recall@3**: whether the file holding the answer was among the top 3 chunks
- **accuracy**: whether the answer contains the expected keywords

It fails unless recall is 5/5 and accuracy at least 4/5. To compare settings,
change one thing (chunk size, `k`, the prompt), re-run and compare the scores.

Other API-calling tests (latency measurements for embed and chat) are also
`#[ignore]`d and run with `cargo test -- --ignored`.
