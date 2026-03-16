use crate::{
    methods::Method,
    parser::{ParseResult, parse},
    response,
};

#[test]
fn parse_simple_get() {
    let raw = b"GET /api/users HTTP/1.1\r\nHost: localhost\r\nAccept: */*\r\n\r\n";
    match parse(raw) {
        ParseResult::Complete(req) => {
            assert_eq!(req.method, Method::Get);
            assert_eq!(req.path_str(), "/api/users");
            assert_eq!(req.version_minor, 1);
            assert_eq!(req.header_count, 2);
            assert_eq!(req.header(b"host"), Some(b"localhost".as_ref()));
            assert!(req.body.is_empty());
        }
        other => panic!("Expected Complete, got {:?}", other),
    }
}

#[test]
fn parse_post_with_body() {
    let raw = b"POST /data HTTP/1.1\r\nContent-Length: 13\r\n\r\n{\"key\":\"val\"}";
    match parse(raw) {
        ParseResult::Complete(req) => {
            assert_eq!(req.method, Method::Post);
            assert_eq!(req.content_length(), Some(13));
            assert_eq!(req.body, b"{\"key\":\"val\"}");
        }
        other => panic!("Expected Complete, got {:?}", other),
    }
}

#[test]
fn incomplete_request() {
    let raw = b"GET /api HTTP/1.1\r\nHost: local";
    assert!(matches!(parse(raw), ParseResult::Incomplete));
}

#[test]
fn write_json_response() {
    let mut buf = [0u8; 4096];
    let body = b"{\"ok\":true}";
    let n = response::write_response(
        &mut buf,
        response::STATUS_200,
        response::CONTENT_JSON,
        body,
        response::SECURITY_BASIC,
        b"",
        b"",
    );
    let resp = std::str::from_utf8(&buf[..n]).unwrap();
    assert!(resp.starts_with("HTTP/1.1 200 OK"));
    assert!(resp.contains("application/json"));
    assert!(resp.contains("X-Content-Type-Options: nosniff"));
    assert!(resp.contains("X-Frame-Options: SAMEORIGIN"));
    assert!(resp.ends_with("{\"ok\":true}"));
}

#[test]
fn write_response_security_none() {
    let mut buf = [0u8; 4096];
    let body = b"{}";
    let n = response::write_response(
        &mut buf,
        response::STATUS_200,
        response::CONTENT_JSON,
        body,
        response::SECURITY_NONE,
        b"",
        b"",
    );
    let resp = std::str::from_utf8(&buf[..n]).unwrap();
    assert!(!resp.contains("X-Content-Type-Options"));
    assert!(!resp.contains("X-Frame-Options"));
}

#[test]
fn write_response_security_strict() {
    let mut buf = [0u8; 4096];
    let body = b"{}";
    let n = response::write_response(
        &mut buf,
        response::STATUS_200,
        response::CONTENT_JSON,
        body,
        response::SECURITY_STRICT,
        b"",
        b"",
    );
    let resp = std::str::from_utf8(&buf[..n]).unwrap();
    assert!(resp.contains("X-Content-Type-Options: nosniff"));
    assert!(resp.contains("X-Frame-Options: DENY"));
    assert!(resp.contains("Referrer-Policy: strict-origin-when-cross-origin"));
    assert!(resp.contains("Permissions-Policy: camera=(), microphone=(), geolocation=()"));
    assert!(resp.contains("Cross-Origin-Opener-Policy: same-origin"));
    assert!(resp.contains("Cross-Origin-Resource-Policy: same-origin"));
}

#[test]
fn write_response_custom_headers() {
    let mut buf = [0u8; 4096];
    let body = b"{}";
    let custom = b"Access-Control-Allow-Origin: *\r\nCache-Control: no-cache\r\n";
    let n = response::write_response(
        &mut buf,
        response::STATUS_200,
        response::CONTENT_JSON,
        body,
        response::SECURITY_NONE,
        custom,
        b"",
    );
    let resp = std::str::from_utf8(&buf[..n]).unwrap();
    assert!(resp.contains("Access-Control-Allow-Origin: *"));
    assert!(resp.contains("Cache-Control: no-cache"));
    assert!(resp.ends_with("{}"));
}

#[test]
fn write_response_with_date_header() {
    let mut buf = [0u8; 4096];
    let body = b"{}";
    let date = b"Date: Mon, 16 Mar 2026 12:00:00 GMT\r\n";
    let n = response::write_response(
        &mut buf,
        response::STATUS_200,
        response::CONTENT_JSON,
        body,
        response::SECURITY_NONE,
        b"",
        date,
    );
    let resp = std::str::from_utf8(&buf[..n]).unwrap();
    assert!(resp.contains("Date: Mon, 16 Mar 2026 12:00:00 GMT"));
    assert!(resp.ends_with("{}"));
}
