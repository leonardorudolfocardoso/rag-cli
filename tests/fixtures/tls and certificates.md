# TLS and Certificates

## TLS 1.3 handshake

TLS 1.3 completes key negotiation in a single round trip (1-RTT) by sending client key shares in the initial `ClientHello`, cutting handshake latency compared to TLS 1.2's two round trips.

All TLS 1.3 key exchanges use ephemeral Diffie-Hellman (ECDHE), guaranteeing forward secrecy: compromising a server's long-term private key later does not allow decrypting previously recorded traffic.

## Certificate chains and trust stores

Clients verify a server's leaf certificate by walking its signature chain up through one or more intermediate CAs to a root CA pinned in the OS or runtime trust store.

When multiple domains share one IP address, the client sends the target hostname in the Server Name Indication (SNI) extension during `ClientHello` so the server presents the matching certificate.

## mTLS for service-to-service auth

In mutual TLS (mTLS), both the server and the client present X.509 certificates during the handshake, establishing cryptographic workload identity in both directions.

Production meshes pair mTLS with short-lived certificates (hours instead of years) rotated automatically by an identity controller (e.g. SPIFFE/SPIRE), limiting the blast radius of a leaked key.

## Certificate rotation incidents

Hardcoding or pinning a leaf certificate in mobile apps or partner clients causes outages as soon as the certificate renews; if pinning is unavoidable, pin an intermediate or backup key instead.

Always monitor certificate expiry with alerts firing at least 30 days ahead and verify that services actually reload the renewed file from disk without requiring a manual restart.
