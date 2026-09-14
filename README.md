# PgShield-rs
### Asynchronous PostgreSQL Wire Protocol Proxy, Multiplexer & Query Firewall in Rust

[![Rust](https://img.shields.io/badge/Language-Rust-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-v3.0_Protocol-blue)](https://www.postgresql.org/)

**PgShield-rs** is a high-performance, asynchronous PostgreSQL wire protocol proxy and real-time security inspection engine built with **Tokio** and **Rust**. It sits between your microservices/serverless workloads and backend PostgreSQL instances to deliver **statement-level connection multiplexing**, **SQL AST query parsing**, **runtime query firewall rules**, and **instant mitigation of database-crashing queries**.

---

## Quick Navigation

- [Overview & Problem Statement](#overview--problem-statement)
- [Key Features](#key-features)
- [System Architecture](#system-architecture)
- [Real-World Use Cases](#real-world-use-cases)
- [Nginx & Infrastructure Topologies](#nginx--infrastructure-topologies)
- [Detailed Documentation](#detailed-documentation)
- [Roadmap & Benchmarking](#roadmap--benchmarking)
- [Contributing & Governance](#contributing--governance)

---

## Overview & Problem Statement

Modern cloud applications relying on auto-scaling compute pods or serverless execution models (AWS Lambda, Cloudflare Workers, Next.js Edge APIs) frequently exhaust PostgreSQL connection limits (`max_connections`). 

While traditional poolers like **PgBouncer** or **AWS RDS Proxy** handle connection reuse, they act as **opaque byte forwarders** without application-layer query intelligence. When a rogue deployment releases an unindexed query, an unbounded `SELECT *` without a `LIMIT`, or an accidental `UPDATE`/`DELETE` without a `WHERE` clause, the primary database locks up—bringing down all connected services.

**PgShield-rs** merges lightweight connection multiplexing with real-time SQL AST query filtering and rate protection.

---

## Key Features

- **Layer 7 PostgreSQL Wire Protocol v3.0 Demuxer**: Direct decoding of startup, authentication, parameter bind, and query frames using `tokio-util` and `bytes`.
- **Async Statement Multiplexing**: Manages thousands of incoming client TCP connections while maintaining a conservative pool of warm backend connections (e.g., 50–100 connections).
- **AST Query Firewall Engine**: Uses `sqlparser-rs` to parse SQL queries into Abstract Syntax Trees in sub-milliseconds, blocking dangerous queries *before* they reach PostgreSQL.
- **Zero-Code Application Integration**: Drop-in compatible with standard PostgreSQL connection strings—no SDKs or app code changes required.
- **OpenTelemetry & Prometheus Telemetry**: Exposes slow query signatures, rule rejection rates, and p99 connection pool wait times.

---

## System Architecture

PgShield-rs operates at Layer 7 by intercepting PostgreSQL v3.0 protocol traffic:

```
[ App / Lambda / Pods ] ──(Postgres TCP 6432)──► [ PgShield-rs Proxy ] ──(Multiplexed 5432)──► [ PostgreSQL DB ]
                                                        │
                                                        ├── 1. Decode PgWire Frames
                                                        ├── 2. AST Inspection & Firewall
                                                        └── 3. Pool Connection Forwarding
```

### Protocol Interception Pipeline

```rust
// --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
// PostgreSQL protocol message handling in Tokio
pub async fn handle_query_frame(
    mut client_stream: Framed<TcpStream, PgWireCodec>,
    backend_pool: Arc<PgBackendPool>,
    firewall: Arc<QueryFirewall>,
) -> Result<(), ProxyError> {
    while let Some(msg) = client_stream.next().await {
        match msg? {
            PgMessage::Query(raw_sql) => {
                // Parse AST and validate firewall rules
                firewall.inspect_statement(&raw_sql)?;
                                
                // Acquire pool slot and forward query to Postgres backend
                let mut conn = backend_pool.acquire().await?;
                conn.forward_query(&raw_sql, &mut client_stream).await?;
            }
            PgMessage::Terminate => break,
            _ => { /* Handle parameter bind, prepare, sync */ }
        }
    }
    Ok(())
}
```

---

## Real-World Use Cases

1. **Serverless & Kubernetes Connection Burst Protection**: Prevents connection exhaustion when thousands of short-lived lambdas or containers spin up simultaneously.
2. **Rogue Query Mitigation**: Instantly drops queries missing `WHERE` clauses on `UPDATE`/`DELETE`, rejecting unindexed full-table scans or `SELECT *` without `LIMIT`.
3. **Multi-Tenant SaaS Guardrails**: Enforces runtime query limits and isolation rules for customer-facing or internal developer SQL platforms.
4. **Zero-Trust Blast Radius Protection**: Restricts compromised microservice database accounts from executing destructive DDL/DML (`DROP TABLE`, `TRUNCATE`).

---

## Nginx & Infrastructure Topologies

### Standard Web Architecture
Nginx manages web traffic (HTTP/HTTPS), while PgShield-rs manages database traffic (PostgreSQL protocol):

```
[ Clients ] ──(HTTPS)──► [ Nginx Web Proxy ] ──(HTTP)──► [ App Services ] ──(PgWire)──► [ PgShield-rs ] ──► [ PostgreSQL ]
```

### High-Availability Nginx Load Balancing for PgShield-rs
Nginx (`stream` module) can load balance TCP traffic across multiple PgShield-rs nodes:

```nginx
stream {
    upstream pgshield_cluster {
        server pgshield-node1.internal:6432;
        server pgshield-node2.internal:6432;
    }

    server {
        listen 5432;
        proxy_pass pgshield_cluster;
    }
}
```

---

## Detailed Documentation

For exhaustive deep-dives on architectural design, failure modes, comparative analysis (PgBouncer vs RDS Proxy vs PgShield-rs), and deployment guides, see:
**[ARCHITECTURE_AND_USE_CASES.md](file:///c:/Users/USER/Desktop/DEV-EDDIERE/OPEN-SOURCE/PgShield/ARCHITECTURE_AND_USE_CASES.md)**

---

## Roadmap & Benchmarking

- **Phase 1: Wire Protocol State Machine** (SSL negotiation, MD5/SCRAM-SHA-256 auth relay, `StartupMessage`, `ReadyForQuery`).
- **Phase 2: Statement Multiplexing** (Transaction-level connection pooler with `deadpool`/`bb8`).
- **Phase 3: SQL AST Guard & Rule Engine** (Integrated `sqlparser-rs` rule configuration).
- **Phase 4: Benchmarks & Chaos Testing** (`pgbench -c 1000 -j 16 -T 60` showing < 0.5ms proxy overhead).

---

## Contributing & Governance

Contributions are welcome! Please see our open-source governance guidelines:
- **[CONTRIBUTING.md](file:///c:/Users/USER/Desktop/DEV-EDDIERE/OPEN-SOURCE/PgShield/CONTRIBUTING.md)**: Development setup & guide for adding custom firewall rules.
- **[SECURITY.md](file:///c:/Users/USER/Desktop/DEV-EDDIERE/OPEN-SOURCE/PgShield/SECURITY.md)**: Vulnerability reporting and security response process.
- **[CODE_OF_CONDUCT.md](file:///c:/Users/USER/Desktop/DEV-EDDIERE/OPEN-SOURCE/PgShield/CODE_OF_CONDUCT.md)**: Community standards and pledge.
- **[CHANGELOG.md](file:///c:/Users/USER/Desktop/DEV-EDDIERE/OPEN-SOURCE/PgShield/CHANGELOG.md)**: Version history and release notes.

---

## License
MIT License. See [LICENSE](file:///c:/Users/USER/Desktop/DEV-EDDIERE/OPEN-SOURCE/PgShield/LICENSE) for details. Developed as an open-source high-resiliency PostgreSQL infrastructure proxy.
