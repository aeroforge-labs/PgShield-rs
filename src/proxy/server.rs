// --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
//! Proxy Server Loop and Client Session Handler

use crate::config::Config;
use crate::firewall::QueryFirewall;
use crate::pool::PgBackendPool;
use crate::protocol::{PgMessage, PgWireCodec};
use futures::stream::StreamExt;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;
use tracing::{error, info, warn};

pub struct ProxyServer {
    config: Config,
    firewall: Arc<QueryFirewall>,
    pool: Arc<PgBackendPool>,
}

impl ProxyServer {
    pub fn new(config: Config) -> Self {
        let firewall = Arc::new(QueryFirewall::new(&config));
        let pool = Arc::new(PgBackendPool::new(&config));

        Self {
            config,
            firewall,
            pool,
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(self.config.listen_addr).await?;
        info!(addr = %self.config.listen_addr, "PgShield-rs Proxy listening for PostgreSQL connections");

        loop {
            let (stream, addr) = match listener.accept().await {
                Ok(val) => val,
                Err(e) => {
                    error!("Error accepting client connection: {}", e);
                    continue;
                }
            };

            info!(client = %addr, "Accepted incoming PostgreSQL client connection");

            let firewall = Arc::clone(&self.firewall);
            let pool = Arc::clone(&self.pool);

            tokio::spawn(async move {
                if let Err(e) = handle_client(stream, firewall, pool).await {
                    error!(client = %addr, error = %e, "Client session terminated with error");
                }
            });
        }
    }
}

async fn handle_client(
    stream: TcpStream,
    firewall: Arc<QueryFirewall>,
    pool: Arc<PgBackendPool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut framed = Framed::new(stream, PgWireCodec::new());

    while let Some(msg_result) = framed.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(e) => return Err(Box::new(e)),
        };

        match msg {
            PgMessage::SslRequest => {
                // Reject SSL upgrade request by sending 'N' to force plain text TCP
                framed.get_mut().write_u8(b'N').await?;
                framed.get_mut().flush().await?;
            }
            PgMessage::StartupMessage { version: _, params } => {
                info!(?params, "Received PostgreSQL StartupMessage");
                // Respond with AuthenticationOk and ReadyForQuery('I')
                let auth_ok = vec![b'R', 0, 0, 0, 8, 0, 0, 0, 0];
                framed.get_mut().write_all(&auth_ok).await?;

                let ready = PgMessage::build_ready_for_query(b'I');
                framed.get_mut().write_all(&ready).await?;
                framed.get_mut().flush().await?;
            }
            PgMessage::Query(raw_sql) => {
                info!(sql = %raw_sql, "Intercepted incoming SQL query");

                // Evaluate firewall rules
                if let Err(violation) = firewall.inspect_statement(&raw_sql) {
                    warn!(sql = %raw_sql, violation = %violation, "Query blocked by PgShield Firewall");

                    let err_msg = format!("PgShield Firewall Error: {}", violation);
                    let err_resp = PgMessage::build_error_response("42000", &err_msg);
                    let ready_resp = PgMessage::build_ready_for_query(b'I');

                    framed.get_mut().write_all(&err_resp).await?;
                    framed.get_mut().write_all(&ready_resp).await?;
                    framed.get_mut().flush().await?;
                    continue;
                }

                // Forward query to backend PostgreSQL database
                match pool.connect_backend().await {
                    Ok(mut backend_stream) => {
                        // Construct PostgreSQL wire frame for Query
                        let mut query_bytes = vec![b'Q'];
                        let len = (4 + raw_sql.len() + 1) as u32;
                        query_bytes.extend_from_slice(&len.to_be_bytes());
                        query_bytes.extend_from_slice(raw_sql.as_bytes());
                        query_bytes.push(0);

                        backend_stream.write_all(&query_bytes).await?;
                        backend_stream.flush().await?;

                        // Relay response from backend back to client
                        let mut buf = vec![0u8; 8192];
                        loop {
                            let n = backend_stream.read(&mut buf).await?;
                            if n == 0 {
                                break;
                            }
                            framed.get_mut().write_all(&buf[..n]).await?;
                            framed.get_mut().flush().await?;

                            // Check if ReadyForQuery ('Z') frame was returned
                            if buf[..n].iter().any(|&b| b == b'Z') {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to connect to backend Postgres: {}", e);
                        let err_msg = format!("PgShield Proxy Error: Unable to connect to backend database ({})", e);
                        let err_resp = PgMessage::build_error_response("08006", &err_msg);
                        let ready_resp = PgMessage::build_ready_for_query(b'I');

                        framed.get_mut().write_all(&err_resp).await?;
                        framed.get_mut().write_all(&ready_resp).await?;
                        framed.get_mut().flush().await?;
                    }
                }
            }
            PgMessage::Terminate => {
                info!("Client issued Terminate command");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
