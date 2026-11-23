# HTTP Caching

## Cache-Control directives

`max-age=N` sets how many seconds a response stays fresh in both shared and browser caches. `s-maxage=N` overrides `max-age` specifically for shared caches like CDNs.

`no-cache` does not forbid storing the response; it requires revalidating with the origin before reusing it. `no-store` forbids writing the response to any cache at all, which is what sensitive data needs.

`stale-while-revalidate=N` lets a cache serve an expired response immediately while refreshing it asynchronously in the background.

## Conditional requests

Origins attach an `ETag` (content fingerprint) or `Last-Modified` timestamp to cacheable responses. Once a cached entry goes stale, the client sends `If-None-Match` or `If-Modified-Since`.

If the resource has not changed, the origin returns `304 Not Modified` with an empty body, saving bandwidth and serialization work. Weak ETags (`W/"..."`) indicate semantic equivalence rather than byte-for-byte identity.

## Vary and cache keys

Caches key entries by method and URL, plus any request headers listed in `Vary`. `Vary: Accept-Encoding` keeps gzip and brotli variants separate so a client never receives an encoding it cannot decode.

Avoid listing high-cardinality headers like `Vary: User-Agent` or `Vary: Cookie`: they fragment the cache into thousands of near-empty buckets and collapse the hit ratio.

## Cache stampede

When a hot key expires, dozens or hundreds of concurrent misses can hit the origin or database at once (a thundering herd / cache stampede).

Mitigations include request coalescing (singleflight) so only one in-flight fill queries the origin while peers wait for its result, serving stale responses during refresh, or probabilistic early expiration before the TTL hits zero.

## Shared vs private caches

Responses containing user-specific data must carry `Cache-Control: private` so a CDN or reverse proxy never serves one user's payload to another. Public shared caches only store responses without `private` and generally ignore responses with `Authorization` headers unless explicitly opted in.
