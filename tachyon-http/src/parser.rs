use crate::{
    http::{MAX_HEADERS, Request},
    utils::{find_header_end, parse_header, parse_method, parse_path, parse_version},
};

/// Parse result: either a complete request or an indication of how many
/// more bytes we need.
#[derive(Debug)]
pub enum ParseResult<'a> {
    Complete(Box<Request<'a>>),
    Incomplete,
    Error(ParseError),
}

#[derive(Debug)]
pub enum ParseError {
    InvalidMethod,
    InvalidPath,
    InvalidVersion,
    HeadersTooLong,
    MalformedHeader,
}

/// Parse a complete HTTP request from a byte buffer.
///
/// # Zero-copy guarantee
/// The returned `Request` borrows directly from `buf`. No data is copied
/// or allocated during parsing. This is the key FaF optimization.
///
/// # Pipelining
/// The request's `body` is bounded by `Content-Length` (if present).
/// Use `Request::consumed()` to find where the next pipelined request starts.
#[inline]
pub fn parse(buf: &[u8]) -> ParseResult<'_> {
    // Find end of headers (double CRLF)
    let header_end = match find_header_end(buf) {
        Some(pos) => pos,
        None => return ParseResult::Incomplete,
    };

    let header_section = &buf[..header_end + 2];

    // Parse request line: METHOD SP PATH SP VERSION CRLF
    let (method, rest) = match parse_method(header_section) {
        Some(r) => r,
        None => return ParseResult::Error(ParseError::InvalidMethod),
    };

    let (path, rest) = match parse_path(rest) {
        Some(r) => r,
        None => return ParseResult::Error(ParseError::InvalidPath),
    };

    let (version_minor, rest) = match parse_version(rest) {
        Some(r) => r,
        None => return ParseResult::Error(ParseError::InvalidVersion),
    };

    // Parse headers
    let mut headers = [None; MAX_HEADERS];
    let mut header_count = 0;
    let mut remaining = rest;

    while !remaining.is_empty() && remaining != b"\r\n" {
        if header_count >= MAX_HEADERS {
            return ParseResult::Error(ParseError::HeadersTooLong);
        }

        match parse_header(remaining) {
            Some((header, rest)) => {
                headers[header_count] = Some(header);
                header_count += 1;
                remaining = rest;
            }
            None => {
                if remaining.starts_with(b"\r\n") {
                    break;
                }
                return ParseResult::Error(ParseError::MalformedHeader);
            }
        }
    }

    // Body starts after the double CRLF
    let body_offset = header_end + 4; // +4 for \r\n\r\n

    // Build a temporary request to extract Content-Length via the header() helper
    let mut req = Request {
        method,
        path,
        version_minor,
        headers,
        header_count,
        body: &[],
        body_offset,
    };

    // Determine body length from Content-Length header (for pipelining support).
    // Without Content-Length, GET/HEAD/DELETE have no body; others consume all remaining bytes.
    let content_length = req.content_length().unwrap_or(0);
    let body_end = body_offset + content_length;

    if body_end > buf.len() {
        // Haven't received the full body yet
        return ParseResult::Incomplete;
    }

    req.body = &buf[body_offset..body_end];

    ParseResult::Complete(Box::new(req))
}
