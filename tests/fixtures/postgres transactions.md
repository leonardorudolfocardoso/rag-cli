# PostgreSQL Transactions

## MVCC and snapshots

PostgreSQL uses Multi-Version Concurrency Control (MVCC): updates write a new tuple version instead of overwriting in place, tagging rows with `xmin` and `xmax` transaction IDs.

Each query or transaction reads a consistent snapshot, so readers never block writers and writers never block readers.

## Isolation levels

The default isolation level in PostgreSQL is `Read Committed`: every statement inside a transaction sees a fresh snapshot including commits made by other transactions up to that statement.

`Repeatable Read` freezes the snapshot at the first non-transaction-control statement and aborts on concurrent write conflicts. `Serializable` uses Serializable Snapshot Isolation (SSI) to detect read-write anomalies without full two-phase locking, at the cost of serialization failures that clients must retry.

## Vacuum and bloat

Because MVCC keeps old row versions until no active snapshot can see them, updates and deletes leave dead tuples behind. `autovacuum` reclaims that space for reuse and updates visibility maps for index-only scans.

Long-running idle-in-transaction sessions hold back the horizon, causing table bloat and eventually risking transaction ID wraparound if vacuum cannot freeze old tuples.

## Locks and deadlocks

Row-level locks (`FOR UPDATE`, `FOR SHARE`) protect concurrent modifications to the same row, while DDL operations such as `ALTER TABLE` can request `ACCESS EXCLUSIVE` locks that queue behind and block all traffic.

For database-backed job queues, `SELECT ... FOR UPDATE SKIP LOCKED` lets workers claim disjoint rows without waiting on each other. PostgreSQL detects deadlocks after `deadlock_timeout` (default 1 second) and aborts one transaction so the other can proceed.

## Connection pooling

Every PostgreSQL connection spawns a backend process with its own memory overhead, so high-concurrency apps sit behind a pooler like PgBouncer.

In transaction pooling mode, a server connection is returned to the pool as soon as a transaction ends. Features tied to session state — such as `SET` parameters, `LISTEN/NOTIFY`, and protocol-level prepared statements without pooler support — can leak across clients or fail in transaction mode.
