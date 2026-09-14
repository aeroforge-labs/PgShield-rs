# Benchmark Results

This document records performance benchmark results comparing PgShield-rs proxy overhead
against a direct PostgreSQL connection using `pgbench`.

## Methodology

### Test Environment

| Parameter         | Value                              |
|-------------------|------------------------------------|
| Tool              | pgbench (PostgreSQL 16)            |
| Workload          | TPC-B-like (pgbench default)       |
| Scale Factor      | -s 10 (~1.4 GB dataset)            |
| Test Duration     | 30 seconds per concurrency level   |
| Worker Threads    | 8                                  |
| Concurrency       | 1, 10, 50, 100, 250, 500, 1000     |

### System Configuration

> Populate with your hardware specs when running benchmarks.

| Parameter         | Value                              |
|-------------------|------------------------------------|
| CPU               | _to be filled_                     |
| Memory            | _to be filled_                     |
| OS                | _to be filled_                     |
| PostgreSQL        | _to be filled_                     |
| PgShield-rs       | v0.2.0 (warm pool + TLS)           |

### Benchmark Script

Run using the provided automation harness:

```bash
chmod +x benchmarks/run_benchmarks.sh
./benchmarks/run_benchmarks.sh
```

---

## Results

> Results will be populated after running the benchmark harness.
> The table below shows the format for recording measurements.

### Transactions Per Second (TPS)

| Concurrency | Direct Postgres TPS | PgShield-rs TPS | Overhead |
|-------------|---------------------|-----------------|----------|
| 1           | _pending_           | _pending_       | _pending_ |
| 10          | _pending_           | _pending_       | _pending_ |
| 50          | _pending_           | _pending_       | _pending_ |
| 100         | _pending_           | _pending_       | _pending_ |
| 250         | _pending_           | _pending_       | _pending_ |
| 500         | _pending_           | _pending_       | _pending_ |
| 1000        | _pending_           | _pending_       | _pending_ |

### Latency Percentiles (100 concurrent clients)

| Percentile | Direct Postgres | PgShield-rs |
|------------|-----------------|-------------|
| p50        | _pending_       | _pending_   |
| p95        | _pending_       | _pending_   |
| p99        | _pending_       | _pending_   |
| p99.9      | _pending_       | _pending_   |

---

## Expected Overhead Profile

Based on the architecture:

- **Firewall-only path (no blocked query)**: Expected < 0.5ms added latency per query due to
  sqlparser-rs AST parsing at query time. AST parsing is CPU-bound and executes inline on
  the Tokio task handling the client session.

- **Connection pool warm-hit path**: Near-zero overhead beyond TCP send/recv when a warm
  backend socket is reused from the idle channel.

- **Connection pool cold-miss path**: Full TCP + optional TLS handshake overhead (~5-15ms
  depending on TLS and network).

---

## Comparison with pgcat

[pgcat](https://github.com/levkkt/pgcat) is a production-grade Rust PostgreSQL pooler focused
on session-mode and transaction-mode connection multiplexing with sharding.

**PgShield-rs differentiates** as an SQL AST security layer that adds query firewall rules
and dynamic query analysis on top of the connection pool — not available in pgcat.
Running both together (pgcat for pooling, PgShield-rs for security) is a valid production
architecture.

---

## Running Your Own Benchmarks

```bash
# Start the full Docker Compose stack
docker-compose up -d

# Wait for PostgreSQL to be healthy
sleep 5

# Run benchmarks
./benchmarks/run_benchmarks.sh

# Review raw results
ls benchmarks/results/
```
