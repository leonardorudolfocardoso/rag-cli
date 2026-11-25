# Plaintext RFC Retrieval Experiments

Empirical retrieval measurements on 3 complete IETF RFCs ([rfc6585.txt](../tests/fixtures_plain/rfc6585.txt), [rfc6750.txt](../tests/fixtures_plain/rfc6750.txt), [rfc7636.txt](../tests/fixtures_plain/rfc7636.txt) — **95,595 chars total**), with 20 questions ([sweep_cases.json](../tests/sweep_cases.json): 16 single-chunk + 4 cross-chunk), embedded with `text-embedding-3-small`. Runner: [tests/sweep.rs](../tests/sweep.rs).

Each experiment has its own directory with its data (CSV), chart (PNG), gnuplot script and write-up:

| # | Experiment | Varies | Chunker | Headline |
| ---: | --- | --- | --- | --- |
| 1 | [Chunk size curve](01-chunk-size/README.md) | `chunk_size`, `k = 3` | `Plain` | Inverted U: `1100–1500` chars best, dilution from `2200` |
| 2 | [Top-`k` curve](02-top-k/README.md) | `k`, `chunk_size = 1100` | `Plain` | Knee at `k = 3`; `k = 7` reaches `20/20` |
| 3 | [`Plain` vs `Rfc` chunker](03-chunker/README.md) | chunker × `chunk_size` × `k` | `Plain`, `Rfc` | Section-aware chunks rank better (`MRR 0.767` vs `0.684`), weaker on cross-chunk |

`Plain` packs paragraphs up to `chunk_size` with no notion of structure (`Markdown` produces identical chunks on these files, checked by `plain_chunker_matches_markdown_on_rfcs`). `Rfc` packs the same way but starts a new chunk at every section heading, found from the page layout.

Run a chart from the repo root, e.g. `gnuplot experiments/03-chunker/plot.gp`.

## Metrics

Each question lists one or more evidence snippets, each tied to a file. A retrieved chunk *contains* a snippet when it comes from that file and includes the snippet (case-insensitive). A question is *found* in the top `k` when every one of its snippets is in some chunk among the first `k` hits.

| Metric | Meaning |
| --- | --- |
| `single@k` | Found questions among the 16 whose evidence fits in one chunk |
| `cross@k` | Found questions among the 4 whose evidence spans two or more chunks (all pieces needed) |
| `overall@k` | Found questions among all 20 (`single@k` + `cross@k`) |
| `MRR` | Mean Reciprocal Rank: how high the evidence ranks, not just whether it's there (below) |
| `avg_prompt_chars` | Average chars of the `k` retrieved chunks per question: the context the chat model would read |

**MRR.** For each question, take the 1-based rank of its evidence in the top `k` and score `1 / rank`: first place scores `1.0`, second `0.5`, third `0.33`. A question with several snippets uses the rank of the *last* one found, since it isn't answerable until all are present; a question with any snippet missing from the top `k` scores `0`. `MRR` is the average over all 20 questions.

For example, three questions found at ranks 1, 3 and not at all give `(1 + 1/3 + 0) / 3 = 0.44`. Two chunkers with the same `overall@k` can differ in `MRR` when one puts the evidence first more often; and `MRR` can never exceed `overall@k` as a fraction (`17/20` found caps it at `0.85`), since misses score `0`. The [Markdown eval](../README.md#tests) computes `recall@k` and `MRR` the same way, over its answerable questions only.
