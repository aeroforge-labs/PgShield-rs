// --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
use bytes::BytesMut;
use pgshield::protocol::{PgMessage, PgWireCodec};
use tokio_util::codec::Decoder;

#[test]
fn test_decode_ssl_request() {
    let mut codec = PgWireCodec::new();
    let mut buf = BytesMut::new();

    // 8-byte SSLRequest: length=8, code=80877103 (0x04D2162F)
    buf.extend_from_slice(&(8i32).to_be_bytes());
    buf.extend_from_slice(&(80877103i32).to_be_bytes());

    let msg = codec.decode(&mut buf).unwrap();
    assert_eq!(msg, Some(PgMessage::SslRequest));
}

#[test]
fn test_decode_query_message() {
    let mut codec = PgWireCodec::new();
    codec.set_startup_complete(true);

    let mut buf = BytesMut::new();
    let sql = "SELECT 1;";
    let len = (4 + sql.len() + 1) as i32;

    buf.extend_from_slice(&[b'Q']);
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(sql.as_bytes());
    buf.extend_from_slice(&[0]);

    let msg = codec.decode(&mut buf).unwrap();
    assert_eq!(msg, Some(PgMessage::Query("SELECT 1;".to_string())));
}

#[test]
fn test_build_error_response() {
    let err_bytes = PgMessage::build_error_response("42000", "PgShield Firewall Violation");
    assert_eq!(err_bytes[0], b'E');
    assert!(err_bytes.len() > 10);
}
