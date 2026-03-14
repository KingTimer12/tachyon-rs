use crate::{methods::Method, utils::eq_ignore_ascii_case};

/// Maximum number of headers we'll parse. FaF uses a similar fixed limit.
pub const MAX_HEADERS: usize = 32;

/// A parsed header: name and value as byte slices into the request buffer.
/// Zero allocation — just pointer + length pairs.
#[derive(Debug, Clone, Copy)]
pub struct Header<'a> {
    pub name: &'a [u8],
    pub value: &'a [u8],
}
 
/// A parsed HTTP request. All fields borrow from the original buffer.
///
/// This is the core design from FaF: the parser never allocates.
/// The request struct is a view into the pre-allocated slab buffer.
#[derive(Debug)]
pub struct Request<'a> {
    pub method: Method,
    pub path: &'a [u8],
    pub version_minor: u8, // 0 = HTTP/1.0, 1 = HTTP/1.1
    pub headers: [Option<Header<'a>>; MAX_HEADERS],
    pub header_count: usize,
    pub body: &'a [u8],
    /// Offset where the body starts (for content-length validation)
    pub body_offset: usize,
}

impl<'a> Request<'a> {
    /// Find a header by name (case-insensitive). Returns the first match.
    pub fn header(&self, name: &[u8]) -> Option<&'a [u8]> {
        self.headers[..self.header_count]
            .iter()
            .filter_map(|h| h.as_ref())
            .find(|h| eq_ignore_ascii_case(h.name, name))
            .map(|h| h.value)
    }
 
    /// Get Content-Length if present and valid.
    pub fn content_length(&self) -> Option<usize> {
        self.header(b"content-length")
            .and_then(|v| std::str::from_utf8(v).ok())
            .and_then(|s| s.trim().parse().ok())
    }
 
    /// Path as UTF-8 string (most paths are ASCII, so this is cheap).
    pub fn path_str(&self) -> &str {
        // Safety: HTTP paths are required to be valid ASCII, and
        // ASCII is valid UTF-8. Non-ASCII bytes would be percent-encoded.
        // We still use from_utf8 (not unchecked) for safety.
        std::str::from_utf8(self.path).unwrap_or("/")
    }
}