# Changelog

All notable changes to **PgShield-rs** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased] - v0.2.0

### Added
- **True Warm Connection Pool:** Replaced the Semaphore-only concurrency gate with a channel-backed warm pool (`PgBackendPool`) that pre-opens idle backend TCP connections at startup and recycles them after each query, eliminating per-query connection overhead.
- **PostgreSQL Auth Relay Passthrough:** Implemented full bi-directional authentication handshake relay between the client and the backend PostgreSQL server, supporting SCRAM-SHA-256 (SASL), MD5, and clear-text auth methods. PgShield-rs no longer responds `AuthenticationOk` unconditionally.
- **Backend TLS Support:** Added optional TLS backend connection support via `tokio-rustls` and `webpki-roots`. Configurable via `--tls-backend` flag or `PGSHIELD_TLS_BACKEND=true` environment variable. Supports cloud PostgreSQL providers (AWS RDS, Supabase, Neon, GCP Cloud SQL).
- **Reproducible Builds via Cargo.lock:** Committed `Cargo.lock` to version control to guarantee bit-exact reproducible CI builds.
- **Benchmarking Harness:** Added `benchmarks/run_benchmarks.sh` pgbench automation script and `BENCHMARKS.md` performance documentation template with methodology and measurement tables.
- **New Config Flags:** Added `--tls-backend`, `--tls-verify`, and `--pool-min-idle` CLI and environment variable options.

### Changed
- `PgMessage` enum extended with SCRAM/SASL auth variants: `AuthenticationMD5Password`, `AuthenticationSASL`, `AuthenticationSASLContinue`, `AuthenticationSASLFinal`, `BackendKeyData`, `ParameterStatus`, `PasswordBytes`.
- `ProxyServer::new()` is now `async` to support warm pool pre-connection at startup.

---

## [0.1.0] - 2026-09-14

### Added
- **PostgreSQL v3.0 Protocol Engine:** Implemented Tokio async decoder/encoder for PostgreSQL wire protocol frames (`StartupMessage`, `SslRequest`, `Query`, `ReadyForQuery`, `ErrorResponse`).
- **Runtime SQL AST Query Firewall:** Integrated `sqlparser-rs` with rule engines:
  - `RequireWhereRule`: Rejects `UPDATE` or `DELETE` statements lacking a `WHERE` selection.
  - `RequireLimitRule`: Rejects `SELECT` queries lacking a `LIMIT` clause when strict limit mode is active.
  - `BlockDDLRule`: Rejects destructive DDL statements (`DROP`, `TRUNCATE`, `ALTER TABLE`) in production mode.
- **Connection Multiplexing Pool:** Built `PgBackendPool` manager managing async connection pools to backend PostgreSQL databases.
- **Configuration & CLI Engine:** Built `clap` CLI parser and environment variable configuration engine.
- **Docker & Deployment Artifacts:** Created multi-stage `Dockerfile` and local `docker-compose.yml` integration stack with Postgres 16 and Nginx.
- **Documentation & Open Source Standards:** Created `README.md`, `ARCHITECTURE_AND_USE_CASES.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, and `LICENSE`.
