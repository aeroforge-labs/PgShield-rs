// --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
//! PostgreSQL Wire Protocol v3.0 Message Types

use bytes::{BufMut, Bytes, BytesMut};

pub const SSL_REQUEST_CODE: i32 = 80877103;

#[derive(Debug, PartialEq, Clone)]
pub enum PgMessage {
    /// SSL Request from client during connection initialization
    SslRequest,

    /// Startup message containing connection parameters (user, database, options)
    StartupMessage {
        version: i32,
        params: Vec<(String, String)>,
    },

    /// Simple Query ('Q')
    Query(String),

    /// Password response ('p') — used for MD5 and SCRAM client-final messages
    Password(String),

    /// Raw password bytes ('p') — for relaying SCRAM binary messages verbatim
    PasswordBytes(Vec<u8>),

    /// Terminate connection ('X')
    Terminate,

    /// Sync frame ('S')
    Sync,

    /// AuthenticationOk from backend ('R' with int32 = 0)
    AuthenticationOk,

    /// AuthenticationMD5Password from backend ('R' with int32 = 5, 4-byte salt)
    AuthenticationMD5Password { salt: [u8; 4] },

    /// AuthenticationSASL from backend ('R' with int32 = 10), lists SASL mechanisms
    AuthenticationSASL { mechanisms: Vec<String> },

    /// AuthenticationSASLContinue from backend ('R' with int32 = 11), SCRAM server-first
    AuthenticationSASLContinue { data: Vec<u8> },

    /// AuthenticationSASLFinal from backend ('R' with int32 = 12), SCRAM server-final
    AuthenticationSASLFinal { data: Vec<u8> },

    /// BackendKeyData from backend ('K') — cancellation key
    BackendKeyData { pid: u32, secret: u32 },

    /// ParameterStatus from backend ('S') — key=value server params
    ParameterStatus { name: String, value: String },

    /// ReadyForQuery response ('Z' with status e.g. b'I')
    ReadyForQuery(u8),

    /// CommandComplete response ('C' with tag string)
    CommandComplete(String),

    /// ErrorResponse ('E')
    ErrorResponse {
        severity: String,
        code: String,
        message: String,
    },

    /// Generic raw byte message for transparent forwarding
    Raw { tag: u8, payload: bytes::Bytes },
}

impl PgMessage {
    /// Encode ErrorResponse message into wire protocol bytes format
    pub fn build_error_response(code: &str, message: &str) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_u8(b'E');

        // Reserve length header (i32)
        let len_pos = buf.len();
        buf.put_i32(0);

        // 'S' = Severity (ERROR)
        buf.put_u8(b'S');
        buf.put_slice(b"ERROR\0");

        // 'C' = Error Code (e.g. 42000)
        buf.put_u8(b'C');
        buf.put_slice(code.as_bytes());
        buf.put_u8(0);

        // 'M' = Message
        buf.put_u8(b'M');
        buf.put_slice(message.as_bytes());
        buf.put_u8(0);

        // Null terminator for key-value fields
        buf.put_u8(0);

        // Calculate and patch message length
        let len = (buf.len() - len_pos) as i32;
        buf[len_pos..len_pos + 4].copy_from_slice(&len.to_be_bytes());

        buf.freeze()
    }

    /// Encode ReadyForQuery message into wire protocol bytes format
    pub fn build_ready_for_query(status: u8) -> Bytes {
        let mut buf = BytesMut::with_capacity(6);
        buf.put_u8(b'Z');
        buf.put_i32(5); // length = 4 (length header) + 1 (status byte)
        buf.put_u8(status);
        buf.freeze()
    }
}
