// --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
//! Asynchronous PostgreSQL Connection Pool Manager

use crate::config::Config;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tracing::info;

pub struct PgBackendPool {
    backend_addr: String,
    semaphore: Arc<Semaphore>,
}

impl PgBackendPool {
    pub fn new(config: &Config) -> Self {
        let backend_addr = format!("{}:{}", config.backend_host, config.backend_port);
        info!(
            backend = %backend_addr,
            max_size = config.pool_max_size,
            "Initializing PgShield Connection Pool"
        );

        Self {
            backend_addr,
            semaphore: Arc::new(Semaphore::new(config.pool_max_size)),
        }
    }

    pub async fn connect_backend(&self) -> Result<TcpStream, std::io::Error> {
        let _permit = self.semaphore.acquire().await.map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Other, "Pool semaphore closed")
        })?;

        TcpStream::connect(&self.backend_addr).await
    }
}
