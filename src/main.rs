// --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
//! PgShield-rs CLI Binary Entry Point

use pgshield::{Config, ProxyServer};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing / logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("Starting PgShield-rs PostgreSQL Wire Protocol Proxy & Query Firewall");

    let config = Config::parse_env();
    info!(
        listen = %config.listen_addr,
        backend = %format!("{}:{}", config.backend_host, config.backend_port),
        pool_size = config.pool_max_size,
        strict_firewall = config.strict_firewall,
        "Configuration loaded"
    );

    let server = ProxyServer::new(config).await?;
    server.run().await?;

    Ok(())
}
