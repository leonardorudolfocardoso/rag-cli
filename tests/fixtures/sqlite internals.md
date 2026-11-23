# SQLite Internals

## WAL mode

By default SQLite uses a rollback journal that blocks readers while a writer commits. Enabling Write-Ahead Logging (`PRAGMA journal_mode = WAL`) appends changes to a separate `-wal` file instead, allowing concurrent readers alongside a single writer.

Checkpointing periodically copies committed pages from the `-wal` file back into the main database file; a long-lived reader prevents checkpoints from reclaiming older WAL frames, causing the `-wal` file to grow.

## Busy timeout and write contention

SQLite still permits only one writer at a time. If a second writer tries to upgrade a deferred transaction (`BEGIN`) after reading, SQLite immediately returns `SQLITE_BUSY` without waiting on `busy_timeout` to avoid a deadlock.

Use `BEGIN IMMEDIATE` for write transactions so SQLite acquires the write lock up front and properly honors `PRAGMA busy_timeout`.

## FTS5 full-text search

SQLite's FTS5 extension provides built-in inverted indexes via virtual tables (`CREATE VIRTUAL TABLE ... USING fts5(...)`) with fast phrase queries, prefix matching, and built-in `bm25()` ranking.

External content FTS5 tables avoid duplicating the full text column by referencing the primary table and syncing updates through SQL triggers.

## Pragmas for production

In WAL mode, `PRAGMA synchronous = NORMAL` avoids an `fsync` on every commit while still remaining crash-safe against power loss (worst case is losing the last few WAL commits, never corrupting the database).

Foreign key enforcement is off by default for backwards compatibility and must be enabled on every new connection with `PRAGMA foreign_keys = ON`.
