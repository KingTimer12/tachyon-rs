// Pre-formatted HTTP response headers. FaF keeps these as compile-time
// constants to avoid any formatting at runtime.

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
pub const ENCODING_GZIP: &[u8] = b"Content-Encoding: gzip\r\n";
pub const VARY_ACCEPT_ENCODING: &[u8] = b"Vary: Accept-Encoding\r\n";
pub const CRLF: &[u8] = b"\r\n";

/// Security header presets — pre-concatenated for a single copy_from_slice per response.
///
/// - `NONE`: no security headers (maximum throughput, user handles security externally)
/// - `BASIC`: essential protection with minimal overhead (default)
/// - `STRICT`: full hardening for production-facing services
pub const SECURITY_NONE: &[u8] = b"";

pub const SECURITY_BASIC: &[u8] = b"\
X-Content-Type-Options: nosniff\r\n\
X-Frame-Options: SAMEORIGIN\r\n";

pub const SECURITY_STRICT: &[u8] = b"\
X-Content-Type-Options: nosniff\r\n\
X-Frame-Options: DENY\r\n\
X-XSS-Protection: 0\r\n\
Referrer-Policy: strict-origin-when-cross-origin\r\n\
Permissions-Policy: camera=(), microphone=(), geolocation=()\r\n\
Cross-Origin-Opener-Policy: same-origin\r\n\
Cross-Origin-Resource-Policy: same-origin\r\n";

/// Security preset levels for HTTP response headers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SecurityPreset {
    /// No security headers. Maximum speed, user handles security externally
    /// (e.g., behind a reverse proxy that adds its own headers).
    None,
    /// Essential security headers with minimal overhead (default).
    /// Adds: X-Content-Type-Options, X-Frame-Options.
    #[default]
    Basic,
    /// Full security hardening for production-facing services.
    /// Adds: all Basic headers + X-XSS-Protection, Referrer-Policy,
    /// Permissions-Policy, COOP, CORP.
    Strict,
}

impl SecurityPreset {
    /// Returns the pre-concatenated header bytes for this preset.
    pub fn as_bytes(self) -> &'static [u8] {
        match self {
            Self::None => SECURITY_NONE,
            Self::Basic => SECURITY_BASIC,
            Self::Strict => SECURITY_STRICT,
        }
    }
}

/// Calculate total response size without writing.
#[inline]
pub fn response_size(
    status: &[u8],
    content_type: &[u8],
    body: &[u8],
    security_headers: &[u8],
    custom_headers: &[u8],
    date_header: &[u8],
) -> usize {
    let cl_header = format!("Content-Length: {}\r\n", body.len());
    status.len()
        + content_type.len()
        + cl_header.len()
        + CONNECTION_KEEP.len()
        + date_header.len()
        + security_headers.len()
        + custom_headers.len()
        + CRLF.len()
        + body.len()
}

/// Write a complete response into a buffer, returning bytes written.
/// This is the FaF callback model: you get a buffer, you fill it, you
/// return how many bytes you wrote.
///
/// **Caller must ensure `buf` is large enough** — use `response_size()` to check.
#[inline]
pub fn write_response(
    buf: &mut [u8],
    status: &[u8],
    content_type: &[u8],
    body: &[u8],
    security_headers: &[u8],
    custom_headers: &[u8],
    date_header: &[u8],
) -> usize {
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

    // Date header (cached, updated once per second)
    if !date_header.is_empty() {
        write_bytes!(date_header);
    }

    // Security headers (pre-concatenated, single memcpy)
    if !security_headers.is_empty() {
        write_bytes!(security_headers);
    }

    // Custom headers from the handler (CORS, Cache-Control, etc.)
    if !custom_headers.is_empty() {
        write_bytes!(custom_headers);
    }

    write_bytes!(CRLF);
    write_bytes!(body);

    pos
}

/// Write a complete response into a Vec (heap-allocated), for bodies that
/// exceed the pool buffer size.
#[inline]
pub fn write_response_vec(
    status: &[u8],
    content_type: &[u8],
    body: &[u8],
    security_headers: &[u8],
    custom_headers: &[u8],
    date_header: &[u8],
) -> Vec<u8> {
    let size = response_size(status, content_type, body, security_headers, custom_headers, date_header);
    let mut buf = Vec::with_capacity(size);
    buf.extend_from_slice(status);
    buf.extend_from_slice(content_type);
    let cl_header = format!("Content-Length: {}\r\n", body.len());
    buf.extend_from_slice(cl_header.as_bytes());
    buf.extend_from_slice(CONNECTION_KEEP);
    if !date_header.is_empty() {
        buf.extend_from_slice(date_header);
    }
    if !security_headers.is_empty() {
        buf.extend_from_slice(security_headers);
    }
    if !custom_headers.is_empty() {
        buf.extend_from_slice(custom_headers);
    }
    buf.extend_from_slice(CRLF);
    buf.extend_from_slice(body);
    buf
}
