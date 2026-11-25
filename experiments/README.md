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
