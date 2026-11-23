# Observability

## Logs, metrics, and traces

Structured logs capture discrete high-cardinality events (request IDs, user IDs, error details) and are expensive to index at huge volume.

Metrics are pre-aggregated numeric time series (counters, gauges, histograms) ideal for alerting and dashboards, but high-cardinality labels like user IDs blow up series counts. Distributed traces link timed spans across services to show where a single request spent its latency budget.

## Percentiles vs averages

Averages hide long tails: a service with 99% of requests at 10ms and 1% at 5 seconds averages 60ms while 1 in 100 users suffers.

Never average p99 values reported by individual hosts to get a fleet-wide p99 — averaging percentiles is mathematically wrong because hosts receive different traffic volumes and have different tail shapes. Aggregate mergeable histograms (such as HDR histograms or DDSketch) instead.

## Context propagation

Distributed tracing passes trace and span IDs across process boundaries using the W3C `traceparent` HTTP header.

Baggage headers let you attach key-value pairs that ride along the entire call tree, which is convenient for tenancy or feature flags, but unbounded baggage inflates every downstream header and can leak sensitive attributes to third-party services.

## Tail-based sampling

Head-based sampling decides whether to record a trace at the entry point before knowing whether the request will fail or be slow, so rare errors are easily dropped.

Tail-based sampling buffers spans at a collector until a trace finishes, then keeps 100% of error and high-latency traces while sampling down healthy fast traffic — at the cost of extra collector memory and stateful routing by trace ID.
