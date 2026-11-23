# Project Marlin

## Overview

Marlin is our internal batch embedding indexer for engineering documentation. It runs nightly as a scheduled job at 02:30 UTC and publishes immutable index snapshots to the S3 bucket `marlin-index-prod`.

## Ingestion pipeline

During each run, Marlin splits Markdown documents into 1200-character chunks and hashes each chunk with SHA-256 to look up existing vectors from the previous snapshot, skipping re-embedding for unchanged chunks.

New or modified chunks are sent to the OpenAI embeddings API with a cap of 8 concurrent worker tasks and exponential backoff on 429 responses.

## Rollback procedure

Every published snapshot has an immutable manifest ID, and consumers discover the active index by reading the `CURRENT` pointer file at the bucket root.

To roll back a bad index release, an operator updates `CURRENT` to point at the previous manifest ID; readers pick up the rollback on their next poll with zero re-embedding required.

## Known limitations

Marlin intentionally skips any file larger than 2 MiB and ignores symbolic links to prevent directory cycles during recursive scans.

Only UTF-8 Markdown and plain text files are supported today; PDF extraction was explicitly deferred to v2.
