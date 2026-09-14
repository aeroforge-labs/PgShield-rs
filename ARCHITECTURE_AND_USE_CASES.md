# PgShield-rs: Architecture, Real-World Use Cases & Infrastructure Integration Spec

---

## Executive Architectural Summary

**PgShield-rs** is an asynchronous PostgreSQL wire protocol proxy, multiplexer, and real-time query inspection engine written in **Rust** using the **Tokio** runtime ecosystem. 

Positioned between application workloads (microservices, serverless functions, background workers) and backend PostgreSQL databases, PgShield-rs provides:
1. **Layer 7 PostgreSQL Wire Protocol v3.0 Interception**
2. **High-Density Statement Connection Multiplexing**
3. **Sub-Millisecond Abstract Syntax Tree (AST) Query Inspection**
4. **Runtime Database Firewalling & Automatic Outage Prevention**

---

## 1. Problem Statement: Database Resiliency at Cloud Scale

### The Cloud-Native Database Bottleneck
Modern cloud-native architectures utilize auto-scaling container clusters (Kubernetes HPA, Nomad) or ephemeral serverless runtimes (AWS Lambda, Google Cloud Functions, Cloudflare Workers, Next.js Edge APIs). During traffic spikes, compute nodes scale horizontally from tens to thousands of concurrent execution contexts in seconds.

Each compute instance typically opens one or more database connections. In PostgreSQL, each client connection is backed by a dedicated OS worker process. This architecture creates two fatal vulnerabilities at scale:

#### Failure Mode 1: Connection & RAM Exhaustion
- **PostgreSQL Resource Cost:** ~2 MB – 10 MB RAM per native client connection + process context-switching overhead.
- **Consequence:** A surge of 1,000+ concurrent serverless functions quickly exceeds PostgreSQL's `max_connections` limit, causing `FATAL: sorry, too many clients already` errors or triggering OS Out-Of-Memory (OOM) kernel kills on the database primary.

#### Failure Mode 2: The "Rogue Query" Cascading Outage
Traditional connection poolers (e.g., PgBouncer, AWS RDS Proxy) function as **transparent Layer 4 / basic Layer 7 byte forwarders**. They pass SQL strings to PostgreSQL without inspecting their semantic intent.

When an application deployment introduces a bad query:
- A `SELECT * FROM audit_events` missing a `LIMIT` clause on a 500M-row table.
- An `UPDATE users SET verified = true` missing a `WHERE` clause due to a developer bug.
- An unindexed `JOIN` that triggers a sequential scan on millions of rows.

PostgreSQL locks CPU cores, disk I/O, and shared buffers. Subsequent legitimate transactions queue up, pool connections exhaust, and the entire database cluster locks up—causing a cascading outage across all microservices.

---

## 2. System Architecture & Protocol Interception

PgShield-rs intercepts native PostgreSQL Frontend/Backend Protocol v3.0 messages without converting them to heavy string representations or relying on prone-to-bypass regular expressions.

### Pipeline Architecture

```
┌──────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                       PGSHIELD-RS PROXY                                          │
│                                                                                                  │
│  [ Client Stream ] ──► [ PgWire Codec ] ──► [ AST Firewall ] ──► [ Conn Pool ] ──► [ DB Stream ]│
│    (Thousands)           (Tokio-util)      (sqlparser-rs)      (Deadpool/bb8)      (Fixed Pool)│
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Component Breakdown

| Pipeline Stage | Core Mechanism | Rust Ecosystem Components | Operational Objective |
| :--- | :--- | :--- | :--- |
| **Protocol Demuxer** | Postgres v3.0 Wire Parser | `tokio-util` codec / `bytes` crate | Decodes startup packets, SSL handshakes, auth frames (`MD5`/`SCRAM-SHA-256`), and query messages. |
| **Backend Connection Pool** | Multiplexed Async Pooler | `deadpool` / `bb8` async pooling | Maintains a fixed, warm pool of backend connections (e.g., 50–100) to PostgreSQL in transaction mode. |
| **Query AST Firewall** | Zero-Copy AST Parsing & Rule Matching | `sqlparser-rs` crate | Tokenizes SQL into AST nodes in micro-seconds, enforcing safety constraints before query execution. |
| **Metrics & Telemetry** | Fingerprinting & Observability | `tracing`, `OpenTelemetry`, `prometheus` | Emits query fingerprints, rule violation alerts, AST parse latencies, and p99 pool queue wait times. |

### Wire Protocol Interception Code Pattern

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
                // 1. Inspect SQL statement via AST rule engine
                firewall.inspect_statement(&raw_sql)?;
                                
                // 2. Acquire multiplexed connection slot from warm pool
                let mut conn = backend_pool.acquire().await?;

                // 3. Forward statement to backend PostgreSQL instance
                conn.forward_query(&raw_sql, &mut client_stream).await?;
            }
            PgMessage::Terminate => break,
            _ => { /* Handle parameter bind, execute, sync frames */ }
        }
    }
    Ok(())
}
```

---

## 3. Deep-Dive Real-World Use Cases

### Use Case 1: Serverless & Container Auto-Scaling Burst Protection
* **Scenario:** An e-commerce platform running on Kubernetes with AWS Lambda edge functions experiences a flash sale. Traffic surges 50x in 10 seconds.
* **Impact without PgShield-rs:** 3,000 serverless functions spin up and attempt to connect directly to Postgres. Postgres hits `max_connections = 500` and crashes.
* **Impact with PgShield-rs:** PgShield-rs holds all 3,000 incoming client TCP connections in lightweight Tokio async tasks. It multiplexes the actual query execution across a fixed pool of 75 Postgres connections. The database operates smoothly at optimal throughput without dropped connections.

### Use Case 2: Outage Mitigation via AST Query Guardrails
* **Scenario:** A continuous deployment pipeline releases a broken ORM migration or flawed API endpoint that executes an `UPDATE orders SET status = 'Archived'` without a `WHERE` clause.
* **Impact without PgShield-rs:** Postgres locks the `orders` table, modifies millions of rows accidentally, and brings down order processing.
* **Impact with PgShield-rs:** PgShield-rs parses the SQL into an AST statement: `Statement::Update { selection: None, .. }`. The rule engine instantly identifies `selection: None` and rejects the statement at the proxy level in **< 0.2ms** with a custom Postgres protocol error:
  `ERROR: 42000: PgShield Firewall: UPDATE statements without a WHERE clause are prohibited in production.`

### Use Case 3: Forced Pagination on Public / Analytics APIs
* **Scenario:** An internal reporting dashboard or customer API allows users to request exported data. A user requests an unfiltered `SELECT * FROM telemetry_events`.
* **Impact with PgShield-rs:** The AST firewall checks for the presence of `Limit` nodes in `Statement::Query`. If missing, PgShield-rs can either:
  1. Reject the query immediately, OR
  2. Mutate the AST on the fly to inject a safe maximum default (`LIMIT 1000`) before forwarding to Postgres.

### Use Case 4: Zero-Trust Security & Blast Radius Reduction
* **Scenario:** A web microservice suffers a SQL injection vulnerability or remote code execution (RCE). The attacker attempts to run `DROP TABLE users;` or `TRUNCATE TABLE payments;`.
* **Impact with PgShield-rs:** Even if the database user configured in the microservice connection string has administrative privileges, PgShield-rs evaluates the AST against a strict rule policy blocking DDL statements (`Statement::Drop`, `Statement::AlterTable`, `Statement::Truncate`) during normal application runtime.

---

## 4. Comprehensive Architectural Comparison

| Feature / Dimension | Standard PgBouncer | AWS RDS Proxy | **PgShield-rs** |
| :--- | :---: | :---: | :---: |
| **Primary Focus** | Connection Pooling | AWS-managed Connection Pooling | Connection Multiplexing + Real-Time SQL AST Security |
| **Protocol Support** | Postgres v3.0 | Postgres & MySQL | Postgres v3.0 |
| **SQL Intelligence** | ❌ Opaque Forwarder | ❌ Opaque Forwarder | ✅ **Full AST Tokenization (`sqlparser-rs`)** |
| **Query Firewalling** | ❌ None | ❌ None | ✅ **AST-based Rule Enforcement** |
| **Unbounded Query Mitigation** | ❌ No | ❌ No | ✅ **Rejects missing `WHERE` / `LIMIT` clauses** |
| **Proxy Overhead Latency** | ~0.1 – 0.3 ms | ~1.0 – 3.0 ms (Managed Cloud) | ✅ **< 0.5 ms (Native Async Rust)** |
| **Deployment Flexibility** | Open Source / Self-hosted | AWS Only | ✅ **Open Source / Container / Sidecar / Gateway** |

---

## 5. Nginx & Infrastructure Integration Topologies

PgShield-rs is designed to integrate into existing production infrastructure alongside web proxies like Nginx.

### Topology A: Multi-Tier Web & Database Architecture (Standard Setup)

In this setup, **Nginx** handles web-facing traffic (HTTP/HTTPS, SSL termination, rate limiting), while **PgShield-rs** handles internal database traffic (PostgreSQL wire protocol):

```
                                [ Internet Clients ]
                                         │
                                         │ HTTPS (Port 443)
                                         ▼
                            ┌──────────────────────────┐
                            │    Nginx Web Gateway     │  <-- Layer 7 Web Proxy & SSL
                            └──────────────────────────┘
                                         │
                                         │ Internal HTTP / gRPC
                                         ▼
                            ┌──────────────────────────┐
                            │ Application Microservices│  (Node.js, Go, Python, Java)
                            └──────────────────────────┘
                                         │
                                         │ Postgres Protocol (Port 6432)
                                         ▼
                            ┌──────────────────────────┐
                            │       PgShield-rs        │  <-- Layer 7 DB Proxy & AST Firewall
                            └──────────────────────────┘
                                         │
                                         │ Multiplexed DB Traffic (Port 5432)
                                         ▼
                            ┌──────────────────────────┐
                            │    PostgreSQL Cluster    │
                            └──────────────────────────┘
```

### Topology B: High-Availability Nginx Load Balancing for PgShield-rs

For high availability (HA), multiple PgShield-rs instances can be deployed in a cluster. **Nginx** uses its `stream` module (Layer 4 TCP proxying) to load-balance database connections across the PgShield-rs pool:

```nginx
# /etc/nginx/nginx.conf (Nginx Layer 4 Stream Load Balancing)
user nginx;
worker_processes auto;

events {
    worker_connections 10240;
}

stream {
    upstream pgshield_backend_cluster {
        least_conn;
        server pgshield-node-1.internal:6432 max_fails=3 fail_timeout=10s;
        server pgshield-node-2.internal:6432 max_fails=3 fail_timeout=10s;
        server pgshield-node-3.internal:6432 max_fails=3 fail_timeout=10s;
    }

    server {
        listen 5432; # Applications connect to Nginx on port 5432
        proxy_pass pgshield_backend_cluster;
        proxy_connect_timeout 2s;
        proxy_timeout 1h;
    }
}
```

### Topology C: Docker Compose Deployment Blueprint

Below is a complete `docker-compose.yml` demonstrating a production-ready stack containing PostgreSQL 16, PgShield-rs, Nginx, and Grafana monitoring:

```yaml
version: '3.8'

services:
  postgres-db:
    image: postgres:16-alpine
    container_name: postgres_primary
    environment:
      POSTGRES_DB: app_db
      POSTGRES_USER: pguser
      POSTGRES_PASSWORD: secretpassword
    ports:
      - "5432:5432"
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U pguser -d app_db"]
      interval: 5s
      timeout: 5s
      retries: 5

  pgshield-proxy:
    image: pgshield/pgshield-rs:latest
    container_name: pgshield_proxy
    environment:
      PGSHIELD_LISTEN_ADDR: "0.0.0.0:6432"
      POSTGRES_BACKEND_HOST: "postgres-db"
      POSTGRES_BACKEND_PORT: "5432"
      POOL_MAX_SIZE: "50"
      FIREWALL_STRICT_MODE: "true"
    ports:
      - "6432:6432"
    depends_on:
      postgres-db:
        condition: service_healthy

  nginx-lb:
    image: nginx:alpine
    container_name: nginx_lb
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf:ro
    ports:
      - "80:80"
      - "443:443"
    depends_on:
      - pgshield-proxy
```

---

## 6. Implementation Roadmap & Benchmarking Specs

### Phase Roadmap

- **Phase 1: Wire Protocol State Machine**
  - Implement full PostgreSQL v3.0 handshake state machine (SSL negotiation, `StartupMessage`, `AuthenticationOk`, `MD5`/`SCRAM-SHA-256` password exchange relay, `ReadyForQuery`).
- **Phase 2: High-Density Statement Multiplexing**
  - Implement transaction-mode pooler allowing thousands of client sessions to share a small pool of warm PostgreSQL backend connections safely.
- **Phase 3: SQL AST Guard & Rule Engine**
  - Integrate `sqlparser-rs`. Develop configurable rule engine supporting:
    - Enforce mandatory `WHERE` on `UPDATE`/`DELETE`.
    - Enforce mandatory `LIMIT` on `SELECT`.
    - Block destructive DDL (`DROP`, `TRUNCATE`, `ALTER`) in production profiles.
- **Phase 4: Real-World Benchmarking & Chaos Testing**
  - Execute `pgbench` stress tests comparing direct Postgres connections vs. PgBouncer vs. PgShield-rs under heavy connection contention.

### Benchmarking with `pgbench`

Stress testing is performed using standard `pgbench` tooling to measure transaction throughput (TPS) and added latency percentiles:

```bash
# Run pgbench through PgShield-rs proxy (1,000 client connections over 16 threads)
pgbench -h localhost -p 6432 -U pguser -c 1000 -j 16 -T 60 app_db
```

#### Expected Benchmark Performance Metrics:
- **Added Proxy Latency:** `< 0.5 ms` p99 added latency overhead.
- **Max Client Capacity:** `10,000+` concurrent client streams.
- **AST Parsing Speed:** `< 150 microseconds` per SQL statement.

---

## 7. Open-Source Publishing & Portfolio Impact

### Repository Deliverables
- Fully reproducible Docker Compose environment with Postgres 16, PgShield-rs, Nginx, and Grafana.
- Comprehensive documentation and rule configuration examples.

### Resume / Portfolio Impact Statement
> *"Designed and developed **PgShield-rs**, an open-source PostgreSQL wire protocol proxy and security inspection engine in Rust. Implemented statement-level connection multiplexing and real-time SQL AST parsing using Tokio and `sqlparser-rs`, protecting database clusters from connection exhaustion and unindexed query spikes with sub-millisecond inspection latency."*
