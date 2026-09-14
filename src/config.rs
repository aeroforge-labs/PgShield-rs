// --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
//! Configuration module for PgShield-rs proxy server.

use clap::Parser;
use std::net::SocketAddr;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "pgshield",
    author = "eddiere",
    version = "0.1.0",
    about = "PostgreSQL Wire Protocol Proxy & Query Firewall"
)]
pub struct Config {
    /// Socket address for PgShield to listen on (e.g., 0.0.0.0:6432)
    #[arg(
        short = 'l',
        long,
        env = "PGSHIELD_LISTEN_ADDR",
        default_value = "0.0.0.0:6432"
    )]
    pub listen_addr: SocketAddr,

    /// Backend PostgreSQL server host
    #[arg(
        short = 'H',
        long,
        env = "POSTGRES_BACKEND_HOST",
        default_value = "127.0.0.1"
    )]
    pub backend_host: String,

    /// Backend PostgreSQL server port
    #[arg(
        short = 'P',
        long,
        env = "POSTGRES_BACKEND_PORT",
        default_value = "5432"
    )]
    pub backend_port: u16,

    /// Maximum backend pool size
    #[arg(short = 's', long, env = "POOL_MAX_SIZE", default_value = "50")]
    pub pool_max_size: usize,

    /// Enforce strict firewall rules (reject bad queries at proxy level)
    #[arg(
        short = 'f',
        long,
        env = "FIREWALL_STRICT_MODE",
        default_value_t = true
    )]
    pub strict_firewall: bool,

    /// Require WHERE clause on UPDATE and DELETE statements
    #[arg(long, env = "FIREWALL_REQUIRE_WHERE", default_value_t = true)]
    pub require_where: bool,

    /// Require LIMIT clause on SELECT statements
    #[arg(long, env = "FIREWALL_REQUIRE_LIMIT", default_value_t = false)]
    pub require_limit: bool,

    /// Block DDL statements (DROP, TRUNCATE, ALTER)
    #[arg(long, env = "FIREWALL_BLOCK_DDL", default_value_t = true)]
    pub block_ddl: bool,

    /// Enable TLS for backend PostgreSQL connections (required for RDS, Supabase, Neon, Cloud SQL)
    #[arg(long, env = "PGSHIELD_TLS_BACKEND", default_value_t = false)]
    pub tls_backend: bool,

    /// Verify backend TLS certificate against system trust roots (set false for self-signed certs)
    #[arg(long, env = "PGSHIELD_TLS_VERIFY", default_value_t = true)]
    pub tls_verify: bool,

    /// Minimum idle backend connections kept warm in the connection pool
    #[arg(long, env = "POOL_MIN_IDLE", default_value = "2")]
    pub pool_min_idle: usize,
}

impl Config {
    pub fn parse_env() -> Self {
        Self::parse()
    }
}
