# Changelog

All notable changes to **PgShield-rs** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
