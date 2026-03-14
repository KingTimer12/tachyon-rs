/// Pre-formatted HTTP response headers. FaF keeps these as compile-time
/// constants to avoid any formatting at runtime.

pub const STATUS_200: &[u8] = b"HTTP/1.1 200 OK\r\n";
pub const STATUS_201: &[u8] = b"HTTP/1.1 201 Created\r\n";
pub const STATUS_204: &[u8] = b"HTTP/1.1 204 No Content\r\n";
pub const STATUS_400: &[u8] = b"HTTP/1.1 400 Bad Request\r\n";
pub const STATUS_404: &[u8] = b"HTTP/1.1 404 Not Found\r\n";
pub const STATUS_500: &[u8] = b"HTTP/1.1 500 Internal Server Error\r\n";

pub const CONTENT_JSON: &[u8] = b"Content-Type: application/json\r\n";
pub const CONTENT_HTML: &[u8] = b"Content-Type: text/html; charset=utf-8\r\n";
pub const CONTENT_TEXT: &[u8] = b"Content-Type: text/plain; charset=utf-8\r\n";
pub const CONNECTION_CLOSE: &[u8] = b"Connection: close\r\n";
pub const CONNECTION_KEEP: &[u8] = b"Connection: keep-alive\r\n";
pub const CRLF: &[u8] = b"\r\n";

/// Write a complete response into a buffer, returning bytes written.
/// This is the FaF callback model: you get a buffer, you fill it, you
/// return how many bytes you wrote.
#[inline]
pub fn write_response(buf: &mut [u8], status: &[u8], content_type: &[u8], body: &[u8]) -> usize {
    let mut pos = 0;

    macro_rules! write_bytes {
        ($src:expr) => {
            let src = $src;
            buf[pos..pos + src.len()].copy_from_slice(src);
            pos += src.len();
        };
    }

    write_bytes!(status);
    write_bytes!(content_type);

    // Content-Length header
    let cl_header = format!("Content-Length: {}\r\n", body.len());
    let cl_bytes = cl_header.as_bytes();
    write_bytes!(cl_bytes);

    write_bytes!(CONNECTION_KEEP);
    write_bytes!(CRLF);
    write_bytes!(body);

    pos
}
