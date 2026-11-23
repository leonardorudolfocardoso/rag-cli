# Distributed Retries and Resilience

## Retry storms

Retrying blindly at every layer of a call graph multiplies load during partial outages. If three services in a chain each retry a failing call 3 times, a single user request fans out into 27 downstream attempts (3^3), turning a brief slowdown into a sustained collapse.

Use retry budgets (for example, capping retries at 10% of total requests) and prefer retrying only at the edge closest to the caller unless a lower layer has a clear local recovery reason.

## Circuit breakers

A circuit breaker tracks recent failure rates to a dependency and transitions through three states: `closed` (normal traffic), `open` (fail fast immediately without calling downstream), and `half-open` (allow a small probe of requests to check recovery).

Failing fast when open protects thread pools and connection slots and gives an overloaded dependency breathing room to recover.

## Hedged requests

To cut tail latency without waiting for a full timeout, a client can send a duplicate read-only request (a hedge) if the first attempt has not returned by the p95 latency threshold, then cancel whichever finishes second.

Hedged requests only make sense for idempotent reads and should be gated by a budget so they do not add load during an outage.

## Idempotency keys

Retrying a non-idempotent mutation (like charging a card or creating an order) risks duplicate side effects when the server processed the request but the response was lost in transit.

Clients attach an `Idempotency-Key` header; the server stores the key alongside a request payload fingerprint and the resulting response inside a transaction, returning the saved response on replays and rejecting mismatched payloads with a conflict error.

## Deadlines and timeout budgets

Fixed per-hop timeouts waste work: if the caller gives up after 2 seconds, a downstream service with its own 5-second timeout keeps burning CPU for a response nobody will read.

Propagate an absolute deadline (or remaining timeout budget) across RPC hops so downstream services immediately skip or abort work once the caller's budget has expired.
