#!/usr/bin/env bash
# --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
# PgShield-rs pgbench Benchmark Harness
#
# Runs pgbench against:
#   1. Direct PostgreSQL (baseline)
#   2. PgShield-rs proxy in front of PostgreSQL
#
# Results are saved to ./results/ for comparison and documentation in BENCHMARKS.md
#
# Requirements:
#   - PostgreSQL client tools (psql, pgbench)
#   - A running PostgreSQL instance (or docker-compose stack)
#   - PgShield-rs binary compiled in release mode (cargo build --release)
#
# Usage:
#   chmod +x benchmarks/run_benchmarks.sh
#   ./benchmarks/run_benchmarks.sh

set -euo pipefail

PGHOST="${PGHOST:-127.0.0.1}"
PGPORT_DIRECT="${PGPORT_DIRECT:-5432}"
PGPORT_PGSHIELD="${PGPORT_PGSHIELD:-6432}"
PGUSER="${PGUSER:-postgres}"
PGPASSWORD="${PGPASSWORD:-postgres}"
PGDATABASE="${PGDATABASE:-postgres}"

RESULTS_DIR="$(dirname "$0")/results"
mkdir -p "$RESULTS_DIR"

TIMESTAMP=$(date -u +"%Y%m%dT%H%M%SZ")

# Concurrency levels to test
CLIENTS=(1 10 50 100 250 500 1000)
THREADS=8
DURATION=30  # seconds per benchmark run

PGBENCH="pgbench"
export PGPASSWORD

echo "PgShield-rs Benchmark Harness"
echo "=============================="
echo "Direct backend:  ${PGHOST}:${PGPORT_DIRECT}"
echo "PgShield proxy:  ${PGHOST}:${PGPORT_PGSHIELD}"
echo "Test database:   ${PGDATABASE}"
echo "Duration:        ${DURATION}s per run"
echo ""

# Initialize pgbench schema on the direct backend
echo "Initializing pgbench schema..."
PGPORT="$PGPORT_DIRECT" pgbench -h "$PGHOST" -U "$PGUSER" -d "$PGDATABASE" -i -s 10 2>&1 | tail -5

echo ""
echo "Running benchmarks..."
echo ""

DIRECT_RESULTS="${RESULTS_DIR}/direct_${TIMESTAMP}.txt"
PGSHIELD_RESULTS="${RESULTS_DIR}/pgshield_${TIMESTAMP}.txt"

echo "# Direct PostgreSQL Benchmark Results - ${TIMESTAMP}" > "$DIRECT_RESULTS"
echo "# PgShield-rs Proxy Benchmark Results - ${TIMESTAMP}" > "$PGSHIELD_RESULTS"

for C in "${CLIENTS[@]}"; do
    echo "--- Concurrency: ${C} clients, ${THREADS} threads ---"

    echo "" >> "$DIRECT_RESULTS"
    echo "## Clients=${C}" >> "$DIRECT_RESULTS"
    PGPORT="$PGPORT_DIRECT" pgbench \
        -h "$PGHOST" \
        -U "$PGUSER" \
        -d "$PGDATABASE" \
        -c "$C" \
        -j "$THREADS" \
        -T "$DURATION" \
        --report-per-command \
        2>&1 | tee -a "$DIRECT_RESULTS" | grep -E "tps|latency|TPS"

    echo "" >> "$PGSHIELD_RESULTS"
    echo "## Clients=${C}" >> "$PGSHIELD_RESULTS"
    PGPORT="$PGPORT_PGSHIELD" pgbench \
        -h "$PGHOST" \
        -U "$PGUSER" \
        -d "$PGDATABASE" \
        -c "$C" \
        -j "$THREADS" \
        -T "$DURATION" \
        --report-per-command \
        2>&1 | tee -a "$PGSHIELD_RESULTS" | grep -E "tps|latency|TPS"

    echo ""
done

echo ""
echo "Benchmark complete."
echo "Direct results:   ${DIRECT_RESULTS}"
echo "PgShield results: ${PGSHIELD_RESULTS}"
echo ""
echo "Summarize these results in BENCHMARKS.md"
