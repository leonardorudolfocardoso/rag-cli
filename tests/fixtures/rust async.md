# Rust Async and Tokio

## Futures and polling

Rust futures are lazy: creating a future does no work until an executor polls it via `Future::poll`. When a future cannot make progress, it registers a `Waker` and returns `Poll::Pending`.

Async multitasking in Tokio is cooperative. A task only yields back to the runtime at `.await` points, so progress depends on tasks returning `Pending` promptly instead of monopolizing a worker thread.

## Blocking the executor

Running CPU-heavy loops or synchronous filesystem and network I/O directly inside an async task blocks the Tokio worker thread, stalling every other task scheduled on that thread.

Offload synchronous or CPU-bound work to `tokio::task::spawn_blocking` (or a dedicated Rayon pool for heavy parallelism) so the async worker threads stay responsive.

## Cancellation and select!

Dropping a future cancels it at its last `.await` point. `tokio::select!` polls multiple branches concurrently and drops the remaining branches as soon as one completes.

A branch is cancel-safe only if dropping it mid-flight loses no data: `mpsc::Receiver::recv` is cancel-safe, whereas a helper that buffers partial reads across multiple polls (like `AsyncBufReadExt::read_line` if reused carelessly or multi-step protocols) can lose partial state unless the future is pinned and resumed across iterations.

## Bounded channels and backpressure

`tokio::sync::mpsc::channel(capacity)` creates a bounded channel: once `capacity` messages are buffered, `send().await` suspends the producer until a consumer drains space, applying backpressure naturally.

Unbounded channels (`unbounded_channel`) never suspend producers; if producers outpace consumers, memory grows without bound until the process OOMs.

## Structured concurrency with JoinSet

`tokio::task::JoinSet` tracks a group of spawned tasks and yields their results in completion order via `join_next().await`.

When a `JoinSet` is dropped — for example because one task returned an error and `?` exited the function early — every task still running in the set is aborted automatically, preventing leaked background work.
