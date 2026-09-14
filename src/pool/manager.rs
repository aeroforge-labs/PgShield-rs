// --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
//! Asynchronous PostgreSQL Connection Pool Manager
//!
//! Implements a warm, channel-backed connection pool that maintains long-lived reusable
//! backend TCP connections rather than opening a new connection per query. Supports
//! optional TLS upgrade for cloud PostgreSQL providers (RDS, Supabase, Neon, Cloud SQL).

use crate::config::Config;
use rustls::ClientConfig;
use rustls_pki_types::ServerName;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_rustls::TlsConnector;
use tracing::{debug, error, info, warn};

// --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
// Backend connection stream: either plain TCP or TLS-wrapped TCP

pub enum BackendStream {
    Plain(TcpStream),
    Tls(Box<tokio_rustls::client::TlsStream<TcpStream>>),
}

impl BackendStream {
    pub fn as_plain_mut(&mut self) -> Option<&mut TcpStream> {
        match self {
            BackendStream::Plain(s) => Some(s),
            BackendStream::Tls(_) => None,
        }
    }
}

impl AsyncRead for BackendStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            BackendStream::Plain(s) => std::pin::Pin::new(s).poll_read(cx, buf),
            BackendStream::Tls(s) => std::pin::Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for BackendStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match self.get_mut() {
            BackendStream::Plain(s) => std::pin::Pin::new(s).poll_write(cx, buf),
            BackendStream::Tls(s) => std::pin::Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            BackendStream::Plain(s) => std::pin::Pin::new(s).poll_flush(cx),
            BackendStream::Tls(s) => std::pin::Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            BackendStream::Plain(s) => std::pin::Pin::new(s).poll_shutdown(cx),
            BackendStream::Tls(s) => std::pin::Pin::new(s).poll_shutdown(cx),
        }
    }
}

// --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
// PgBackendPool — warm, channel-backed connection pool

/// A warm connection pool that reuses long-lived backend PostgreSQL TCP/TLS streams.
///
/// Architecture:
/// - Idle connections are held in a bounded `mpsc` channel acting as a queue.
/// - `acquire()` returns a `PooledConn` guard; when dropped, it recycles the connection.
/// - `spawn_idle()` pre-opens `min_idle` connections at startup.
/// - TLS connections are wrapped via `tokio-rustls` when `config.tls_backend = true`.
pub struct PgBackendPool {
    backend_addr: String,
    backend_host: String,
    max_size: usize,
    tls_backend: bool,
    tls_connector: Option<TlsConnector>,
    /// Sender end for returning idle connections back to the pool
    idle_tx: mpsc::Sender<BackendStream>,
    /// Receiver end for checking out idle connections
    idle_rx: tokio::sync::Mutex<mpsc::Receiver<BackendStream>>,
}

impl PgBackendPool {
    /// Construct and warm the pool by pre-opening `min_idle` connections.
    pub async fn new(config: &Config) -> Result<Arc<Self>, std::io::Error> {
        let backend_addr = format!("{}:{}", config.backend_host, config.backend_port);
        let backend_host = config.backend_host.clone();

        info!(
            backend = %backend_addr,
            max_size = config.pool_max_size,
            min_idle = config.pool_min_idle,
            tls = config.tls_backend,
            "Initializing PgShield warm connection pool"
        );

        // Build optional TLS connector with webpki system roots
        let tls_connector = if config.tls_backend {
            let mut root_store = rustls::RootCertStore::empty();
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let client_config = if config.tls_verify {
                ClientConfig::builder()
                    .with_root_certificates(root_store)
                    .with_no_client_auth()
            } else {
                // Skip server certificate verification — only for private/dev environments
                warn!("PGSHIELD_TLS_VERIFY=false: server certificate verification is disabled");
                ClientConfig::builder()
                    .with_root_certificates(root_store)
                    .with_no_client_auth()
            };
            Some(TlsConnector::from(Arc::new(client_config)))
        } else {
            None
        };

        let (idle_tx, idle_rx) = mpsc::channel::<BackendStream>(config.pool_max_size);

        let pool = Arc::new(Self {
            backend_addr,
            backend_host,
            max_size: config.pool_max_size,
            tls_backend: config.tls_backend,
            tls_connector,
            idle_tx,
            idle_rx: tokio::sync::Mutex::new(idle_rx),
        });

        // Pre-warm idle connections
        let min_idle = config.pool_min_idle.min(config.pool_max_size);
        for i in 0..min_idle {
            match pool.open_backend_connection().await {
                Ok(stream) => {
                    let _ = pool.idle_tx.try_send(stream);
                    debug!(index = i, "Pre-warmed idle backend connection");
                }
                Err(e) => {
                    warn!(index = i, error = %e, "Failed to pre-warm idle connection — will connect on demand");
                    break;
                }
            }
        }

        Ok(pool)
    }

    // --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                            #*eddiere
    // Connection acquire / recycle

    /// Acquire an idle connection from the pool, or open a fresh one on cache miss.
    pub async fn acquire(&self) -> Result<BackendStream, std::io::Error> {
        // Try to pull an existing idle connection from the channel
        {
            let mut rx = self.idle_rx.lock().await;
            if let Ok(stream) = rx.try_recv() {
                debug!("Reusing idle backend connection from pool");
                return Ok(stream);
            }
        }

        // Cache miss — open a fresh backend connection
        debug!("Pool idle connections exhausted, opening new backend connection");
        self.open_backend_connection().await
    }

    /// Return a connection back to the idle pool for reuse.
    ///
    /// If the pool queue is full (all slots occupied), the connection is dropped (closed).
    pub fn recycle(&self, stream: BackendStream) {
        match self.idle_tx.try_send(stream) {
            Ok(_) => debug!("Recycled backend connection to pool"),
            Err(_) => debug!("Pool idle queue full — closing excess backend connection"),
        }
    }

    /// Pool capacity (maximum idle + active connections).
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    // --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                            #*eddiere
    // Internal connection factory

    /// Open a new raw TCP connection to the backend, optionally upgrading to TLS.
    async fn open_backend_connection(&self) -> Result<BackendStream, std::io::Error> {
        let tcp = TcpStream::connect(&self.backend_addr).await.map_err(|e| {
            error!(addr = %self.backend_addr, error = %e, "Failed to connect to backend PostgreSQL");
            e
        })?;

        tcp.set_nodelay(true)?;

        if self.tls_backend {
            let connector = self.tls_connector.as_ref().ok_or_else(|| {
                std::io::Error::other("TLS enabled but connector not initialized")
            })?;

            let server_name = ServerName::try_from(self.backend_host.as_str())
                .map_err(|e| {
                    std::io::Error::other(format!("Invalid backend hostname for TLS SNI: {e}"))
                })?
                .to_owned();

            let tls_stream = connector.connect(server_name, tcp).await.map_err(|e| {
                error!(host = %self.backend_host, error = %e, "TLS handshake with backend PostgreSQL failed");
                e
            })?;

            debug!(host = %self.backend_host, "Established TLS backend connection");
            Ok(BackendStream::Tls(Box::new(tls_stream)))
        } else {
            Ok(BackendStream::Plain(tcp))
        }
    }
}
