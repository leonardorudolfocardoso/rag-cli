# Project Caipora

## Runtime and endpoints

Caipora is an internal gateway daemon written in Rust that proxies tool calls to downstream servers over stdio and HTTP.

The daemon accepts local requests on Unix domain socket `/run/caipora/gw.sock` and exposes Prometheus-format admin metrics on TCP port `9471` (bound to loopback only).

## Policy and approvals

Every registered tool is classified in `policy.toml` as `read`, `write`, or `destructive`. `read` tools execute automatically; `write` tools require a matching caller scope.

Calls to `destructive` tools are blocked unless accompanied by a signed approval token issued by the `sentinel` daemon. Approval tokens expire after 90 seconds and are single-use.

## Quotas and circuit breaking

Caipora enforces a per-server concurrency cap that defaults to 6 in-flight `tools/call` requests, rejecting excess traffic with an overload error rather than queueing unboundedly.

If a downstream server hits 5 consecutive timeouts, Caipora opens that server's circuit breaker for 30 seconds before sending a single health probe.

## Audit log format

Every tool invocation is appended as one JSON object per line to `/var/log/caipora/audit.jsonl`.

Before writing an audit entry, Caipora strips any JSON keys listed under `redact_keys` in config and hashes the sanitized arguments and response payloads with BLAKE3 so operators can correlate calls without storing secrets in plaintext.
