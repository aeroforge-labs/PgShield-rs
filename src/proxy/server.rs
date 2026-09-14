// --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
//! Proxy Server Loop and Client Session Handler
//!
//! Implements the full PostgreSQL v3.0 wire protocol session lifecycle:
//!   1. SSL negotiation (upgrade or reject with fallback)
//!   2. Startup / authentication relay between client and backend PostgreSQL
//!   3. Query interception, firewall evaluation, and forwarding
//!   4. Connection recycling back to the warm pool after session completes

use crate::config::Config;
use crate::firewall::QueryFirewall;
use crate::pool::{BackendStream, PgBackendPool};
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
    pub async fn new(config: Config) -> Result<Self, std::io::Error> {
        let firewall = Arc::new(QueryFirewall::new(&config));
        let pool = PgBackendPool::new(&config).await?;

        Ok(Self {
            config,
            firewall,
            pool,
        })
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

// --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
// Auth relay helpers

/// Forward client startup credentials to the backend and relay the full auth challenge back.
/// Supports SCRAM-SHA-256 (SASL), MD5, and clear-text password relay.
async fn relay_auth_handshake(
    client: &mut Framed<TcpStream, PgWireCodec>,
    backend: &mut BackendStream,
    startup_bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Forward the original StartupMessage to the backend PostgreSQL server
    backend.write_all(startup_bytes).await?;
    backend.flush().await?;

    // Read backend response — either AuthRequest or ErrorResponse
    let mut buf = vec![0u8; 8192];
    loop {
        let n = backend.read(&mut buf).await?;
        if n == 0 {
            return Err(
                std::io::Error::other("Backend closed connection during auth handshake").into(),
            );
        }

        let msg_type = buf[0];

        match msg_type {
            // 'R' — Authentication message from backend
            b'R' => {
                if n < 9 {
                    // Forward partial — may be AuthOk (9 bytes)
                    client.get_mut().write_all(&buf[..n]).await?;
                    client.get_mut().flush().await?;
                }

                let auth_type = i32::from_be_bytes([buf[5], buf[6], buf[7], buf[8]]);

                match auth_type {
                    // AuthenticationOk — backend accepted credentials without a challenge
                    0 => {
                        client.get_mut().write_all(&buf[..n]).await?;
                        client.get_mut().flush().await?;
                        return Ok(());
                    }
                    // AuthenticationMD5Password — relay salt to client, wait for hashed password
                    5 => {
                        client.get_mut().write_all(&buf[..n]).await?;
                        client.get_mut().flush().await?;

                        // Read client MD5 password response ('p')
                        let mut client_resp = vec![0u8; 4096];
                        let m = client.get_mut().read(&mut client_resp).await?;

                        // Forward MD5 password to backend
                        backend.write_all(&client_resp[..m]).await?;
                        backend.flush().await?;
                    }
                    // AuthenticationSASL (10) — SCRAM-SHA-256 mechanism list
                    10 => {
                        client.get_mut().write_all(&buf[..n]).await?;
                        client.get_mut().flush().await?;

                        // Read client SASL initial response ('p')
                        let mut client_resp = vec![0u8; 4096];
                        let m = client.get_mut().read(&mut client_resp).await?;

                        // Forward to backend
                        backend.write_all(&client_resp[..m]).await?;
                        backend.flush().await?;
                    }
                    // AuthenticationSASLContinue (11) — SCRAM server-first-message
                    11 => {
                        client.get_mut().write_all(&buf[..n]).await?;
                        client.get_mut().flush().await?;

                        // Read client SASL continue response ('p')
                        let mut client_resp = vec![0u8; 4096];
                        let m = client.get_mut().read(&mut client_resp).await?;

                        backend.write_all(&client_resp[..m]).await?;
                        backend.flush().await?;
                    }
                    // AuthenticationSASLFinal (12) — SCRAM server-final-message
                    12 => {
                        client.get_mut().write_all(&buf[..n]).await?;
                        client.get_mut().flush().await?;
                        // No client response expected after SASLFinal
                    }
                    unknown => {
                        warn!(
                            auth_type = unknown,
                            "Unrecognized backend auth type — forwarding raw bytes"
                        );
                        client.get_mut().write_all(&buf[..n]).await?;
                        client.get_mut().flush().await?;
                    }
                }
            }
            // 'E' — Backend rejected credentials with ErrorResponse
            b'E' => {
                client.get_mut().write_all(&buf[..n]).await?;
                client.get_mut().flush().await?;
                return Err(std::io::Error::other("Backend rejected client credentials").into());
            }
            // 'K' — BackendKeyData (after auth): relay and signal auth complete
            b'K' | b'Z' => {
                client.get_mut().write_all(&buf[..n]).await?;
                client.get_mut().flush().await?;
                if msg_type == b'Z' {
                    return Ok(());
                }
            }
            // 'S' — ParameterStatus: relay to client
            b'S' => {
                client.get_mut().write_all(&buf[..n]).await?;
                client.get_mut().flush().await?;
            }
            other => {
                warn!(
                    byte = other,
                    "Unexpected backend message during auth — forwarding"
                );
                client.get_mut().write_all(&buf[..n]).await?;
                client.get_mut().flush().await?;
            }
        }
    }
}

// --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
// Client session handler

async fn handle_client(
    stream: TcpStream,
    firewall: Arc<QueryFirewall>,
    pool: Arc<PgBackendPool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut framed = Framed::new(stream, PgWireCodec::new());
    let mut backend_opt: Option<BackendStream> = None;

    while let Some(msg_result) = framed.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(e) => return Err(Box::new(e)),
        };

        match msg {
            // ----------------------------------------------------------------
            // SSL Negotiation: reject TLS upgrade from client side
            // Backend TLS is handled independently in the pool connection
            PgMessage::SslRequest => {
                framed.get_mut().write_u8(b'N').await?;
                framed.get_mut().flush().await?;
            }

            // ----------------------------------------------------------------
            // Startup: open backend connection, relay full auth handshake
            PgMessage::StartupMessage {
                version,
                ref params,
            } => {
                info!(
                    version = version,
                    ?params,
                    "Received PostgreSQL StartupMessage"
                );

                // Reconstruct the raw startup wire frame to forward to backend
                let mut raw = Vec::new();
                // Reserve 4 bytes for length prefix
                raw.extend_from_slice(&[0u8; 4]);
                raw.extend_from_slice(&version.to_be_bytes());
                for (k, v) in params {
                    raw.extend_from_slice(k.as_bytes());
                    raw.push(0);
                    raw.extend_from_slice(v.as_bytes());
                    raw.push(0);
                }
                raw.push(0); // Null terminator
                let total_len = raw.len() as u32;
                raw[0..4].copy_from_slice(&total_len.to_be_bytes());

                // Acquire a warm backend connection
                let mut backend = match pool.acquire().await {
                    Ok(b) => b,
                    Err(e) => {
                        error!(error = %e, "Failed to acquire backend connection from pool");
                        let err_msg = format!("PgShield Pool Error: {e}");
                        let err_resp = PgMessage::build_error_response("08006", &err_msg);
                        framed.get_mut().write_all(&err_resp).await?;
                        framed.get_mut().flush().await?;
                        return Err(e.into());
                    }
                };

                // Relay auth handshake: client <-> backend
                match relay_auth_handshake(&mut framed, &mut backend, &raw).await {
                    Ok(_) => {
                        info!("Authentication relay completed successfully");
                        // Continue draining any remaining ParameterStatus / BackendKeyData / ReadyForQuery
                        let mut buf = vec![0u8; 8192];
                        loop {
                            let n = match backend.read(&mut buf).await {
                                Ok(0) => break,
                                Ok(n) => n,
                                Err(_) => break,
                            };
                            framed.get_mut().write_all(&buf[..n]).await?;
                            framed.get_mut().flush().await?;
                            // 'Z' signals ReadyForQuery — auth startup complete
                            if buf[..n].contains(&b'Z') {
                                break;
                            }
                        }
                        backend_opt = Some(backend);
                    }
                    Err(e) => {
                        warn!(error = %e, "Auth handshake failed — closing backend connection");
                        return Err(e);
                    }
                }
            }

            // ----------------------------------------------------------------
            // Query: firewall check then forward to backend via warm connection
            PgMessage::Query(raw_sql) => {
                info!(sql = %raw_sql, "Intercepted incoming SQL query");

                // Evaluate firewall rules
                if let Err(violation) = firewall.inspect_statement(&raw_sql) {
                    warn!(sql = %raw_sql, violation = %violation, "Query blocked by PgShield Firewall");
                    let err_msg = format!("PgShield Firewall Error: {violation}");
                    let err_resp = PgMessage::build_error_response("42000", &err_msg);
                    let ready_resp = PgMessage::build_ready_for_query(b'I');
                    framed.get_mut().write_all(&err_resp).await?;
                    framed.get_mut().write_all(&ready_resp).await?;
                    framed.get_mut().flush().await?;
                    continue;
                }

                // Acquire backend (reuse warm connection if available)
                let mut backend = match backend_opt.take() {
                    Some(b) => b,
                    None => match pool.acquire().await {
                        Ok(b) => b,
                        Err(e) => {
                            error!(error = %e, "Failed to acquire backend connection from pool");
                            let err_msg = format!("PgShield Pool Error: {e}");
                            let err_resp = PgMessage::build_error_response("08006", &err_msg);
                            let ready_resp = PgMessage::build_ready_for_query(b'I');
                            framed.get_mut().write_all(&err_resp).await?;
                            framed.get_mut().write_all(&ready_resp).await?;
                            framed.get_mut().flush().await?;
                            continue;
                        }
                    },
                };

                // Build PostgreSQL wire frame for Query message
                let mut query_bytes = vec![b'Q'];
                let len = (4 + raw_sql.len() + 1) as u32;
                query_bytes.extend_from_slice(&len.to_be_bytes());
                query_bytes.extend_from_slice(raw_sql.as_bytes());
                query_bytes.push(0);

                backend.write_all(&query_bytes).await?;
                backend.flush().await?;

                // Relay response from backend to client
                let mut buf = vec![0u8; 65536];
                loop {
                    let n = match backend.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(e) => {
                            error!(error = %e, "Backend stream read error during query relay");
                            break;
                        }
                    };
                    framed.get_mut().write_all(&buf[..n]).await?;
                    framed.get_mut().flush().await?;

                    // 'Z' ReadyForQuery indicates response is fully delivered
                    if buf[..n].contains(&b'Z') {
                        break;
                    }
                }

                // Recycle the backend connection to the warm pool
                pool.recycle(backend);
            }

            // ----------------------------------------------------------------
            // Terminate: client closed session gracefully
            PgMessage::Terminate => {
                info!("Client issued Terminate command — closing session");
                // Return backend connection to pool if we have one
                if let Some(backend) = backend_opt.take() {
                    pool.recycle(backend);
                }
                break;
            }

            _ => {}
        }
    }

    Ok(())
}
