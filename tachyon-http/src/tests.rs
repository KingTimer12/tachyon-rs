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
    let n = response::write_response(&mut buf, response::STATUS_200, response::CONTENT_JSON, body);
    let resp = std::str::from_utf8(&buf[..n]).unwrap();
    assert!(resp.starts_with("HTTP/1.1 200 OK"));
    assert!(resp.contains("application/json"));
    assert!(resp.ends_with("{\"ok\":true}"));
}
