# Search and Ranking

## Lexical search with BM25

BM25 scores documents by matching exact query terms, rewarding rare terms (inverse document frequency), saturating repeated term frequency so keyword stuffing has diminishing returns, and normalizing for document length.

Because it matches literal tokens, BM25 excels on exact identifiers, error codes, function names, and SKUs.

## Dense retrieval blind spots

Dense embedding models map semantically similar text close together, which handles synonyms and paraphrasing well.

However, bi-encoder embeddings frequently struggle with exact matches (rare acronyms, version numbers, SKUs, error codes), subtle negations, and out-of-domain jargon not well represented during training.

## Hybrid search and RRF

Hybrid retrieval runs both lexical search (BM25) and dense vector search, then merges the ranked lists.

Reciprocal Rank Fusion (RRF) scores each candidate as the sum of `1 / (k + rank)` across lists (commonly `k = 60`). Because RRF uses only ranks, it avoids the brittle work of normalizing incompatible BM25 and cosine score distributions.

## Cross-encoder reranking

Bi-encoders embed the query and each chunk independently, which makes indexing fast but prevents token-level interaction between query and passage.

A cross-encoder takes `(query, chunk)` concatenated as a single input and applies full cross-attention. It is too slow to scan the entire corpus, so it runs as a second stage over the top 20–50 candidates to pick the final top 3–5.

## Evaluation metrics

`Recall@k` checks whether the relevant chunk appears anywhere in the top `k`, ignoring whether it landed at position 1 or position `k`.

`MRR` (Mean Reciprocal Rank) scores `1 / rank` of the first relevant result, penalizing burying the answer lower in the list. `NDCG@k` extends ranking evaluation to graded relevance levels (e.g. 0, 1, 2) with logarithmic position discount.
