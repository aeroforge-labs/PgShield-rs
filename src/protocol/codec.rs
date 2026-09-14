// --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
//! PostgreSQL Wire Protocol Codec implementation for Tokio Framed Streams

use crate::protocol::messages::{PgMessage, SSL_REQUEST_CODE};
use bytes::{Buf, BufMut, BytesMut};
use std::io;
use tokio_util::codec::{Decoder, Encoder};

pub struct PgWireCodec {
    startup_complete: bool,
}

impl PgWireCodec {
    pub fn new() -> Self {
        Self {
            startup_complete: false,
        }
    }

    pub fn set_startup_complete(&mut self, complete: bool) {
        self.startup_complete = complete;
    }
}

impl Default for PgWireCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder for PgWireCodec {
    type Item = PgMessage;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.is_empty() {
            return Ok(None);
        }

        // Handle startup phase (SSLRequest or StartupMessage)
        if !self.startup_complete {
            if src.len() < 4 {
                return Ok(None);
            }

            let len = i32::from_be_bytes([src[0], src[1], src[2], src[3]]) as usize;
            if src.len() < len {
                return Ok(None);
            }

            let mut msg_buf = src.split_to(len);
            msg_buf.advance(4); // Consume 4-byte length

            if len == 8 {
                let code = msg_buf.get_i32();
                if code == SSL_REQUEST_CODE {
                    return Ok(Some(PgMessage::SslRequest));
                }
            }

            let version = msg_buf.get_i32();
            let mut params = Vec::new();

            while msg_buf.has_remaining() {
                let key = read_null_string(&mut msg_buf)?;
                if key.is_empty() {
                    break;
                }
                let val = read_null_string(&mut msg_buf)?;
                params.push((key, val));
            }

            self.startup_complete = true;
            return Ok(Some(PgMessage::StartupMessage { version, params }));
        }

        // Standard message frame phase (1-byte tag + 4-byte length)
        if src.len() < 5 {
            return Ok(None);
        }

        let tag = src[0];
        let len = i32::from_be_bytes([src[1], src[2], src[3], src[4]]) as usize;
        
        let total_frame_len = 1 + len;
        if src.len() < total_frame_len {
            return Ok(None);
        }

        let mut frame_buf = src.split_to(total_frame_len);
        frame_buf.advance(5); // Consume tag (1 byte) and length (4 bytes)

        let payload = frame_buf.freeze();

        match tag {
            b'Q' => {
                let raw_sql = String::from_utf8_lossy(&payload).trim_matches('\0').to_string();
                Ok(Some(PgMessage::Query(raw_sql)))
            }
            b'p' => {
                let password = String::from_utf8_lossy(&payload).trim_matches('\0').to_string();
                Ok(Some(PgMessage::Password(password)))
            }
            b'X' => Ok(Some(PgMessage::Terminate)),
            b'S' => Ok(Some(PgMessage::Sync)),
            _ => Ok(Some(PgMessage::Raw { tag, payload })),
        }
    }
}

impl Encoder<PgMessage> for PgWireCodec {
    type Error = io::Error;

    fn encode(&mut self, item: PgMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match item {
            PgMessage::Query(sql) => {
                dst.put_u8(b'Q');
                let len = (4 + sql.len() + 1) as i32;
                dst.put_i32(len);
                dst.put_slice(sql.as_bytes());
                dst.put_u8(0); // Null terminator
            }
            PgMessage::ReadyForQuery(status) => {
                dst.put_u8(b'Z');
                dst.put_i32(5);
                dst.put_u8(status);
            }
            PgMessage::ErrorResponse { severity: _, code, message } => {
                let err_bytes = PgMessage::build_error_response(&code, &message);
                dst.extend_from_slice(&err_bytes);
            }
            PgMessage::Raw { tag, payload } => {
                dst.put_u8(tag);
                let len = (4 + payload.len()) as i32;
                dst.put_i32(len);
                dst.extend_from_slice(&payload);
            }
            _ => {}
        }
        Ok(())
    }
}

fn read_null_string(buf: &mut BytesMut) -> Result<String, io::Error> {
    if let Some(null_pos) = buf.iter().position(|&b| b == 0) {
        let str_bytes = buf.split_to(null_pos);
        buf.advance(1); // Consume null byte
        Ok(String::from_utf8_lossy(&str_bytes).to_string())
    } else {
        Ok(String::new())
    }
}
