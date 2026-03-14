use crate::{http::Header, methods::Method};

#[inline(always)]
pub fn eq_ignore_ascii_case(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .all(|(&x, &y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

#[inline(always)]
pub fn find_header_end(buf: &[u8]) -> Option<usize> {
    // SIMD path: delegates to C++ SSE4.2/AVX2/NEON scanner via cxx bridge.
    // 68-90% faster than scalar on typical HTTP headers.
    #[cfg(feature = "simd")]
    {
        tachyon_simd::find_headers_end(buf)
    }
 
    // Scalar fallback when compiled without SIMD feature.
    #[cfg(not(feature = "simd"))]
    {
        if buf.len() < 4 {
            return None;
        }
        for i in 0..buf.len() - 3 {
            if buf[i] == b'\r' && buf[i + 1] == b'\n' && buf[i + 2] == b'\r' && buf[i + 3] == b'\n' {
                return Some(i);
            }
        }
        None
    }
}
 
#[inline(always)]
pub fn memchr_byte(needle: u8, haystack: &[u8]) -> Option<usize> {
    // SIMD path: 16-32 bytes per cycle instead of 1.
    #[cfg(feature = "simd")]
    {
        tachyon_simd::memchr(needle, haystack)
    }
 
    #[cfg(not(feature = "simd"))]
    {
        haystack.iter().position(|&b| b == needle)
    }
}
 
#[inline(always)]
pub fn parse_method(buf: &[u8]) -> Option<(Method, &[u8])> {
    // Check common methods by first byte for fast dispatch
    let (method, len) = if buf.starts_with(b"GET ") {
        (Method::Get, 4)
    } else if buf.starts_with(b"POST ") {
        (Method::Post, 5)
    } else if buf.starts_with(b"PUT ") {
        (Method::Put, 4)
    } else if buf.starts_with(b"DELETE ") {
        (Method::Delete, 7)
    } else if buf.starts_with(b"PATCH ") {
        (Method::Patch, 6)
    } else if buf.starts_with(b"HEAD ") {
        (Method::Head, 5)
    } else if buf.starts_with(b"OPTIONS ") {
        (Method::Options, 8)
    } else {
        // Skip unknown method
        let sp = memchr_byte(b' ', buf)?;
        (Method::Other, sp + 1)
    };
    Some((method, &buf[len..]))
}
 
#[inline(always)]
pub fn parse_path(buf: &[u8]) -> Option<(&[u8], &[u8])> {
    let sp = memchr_byte(b' ', buf)?;
    let path = &buf[..sp];
    if path.is_empty() {
        return None;
    }
    Some((path, &buf[sp + 1..]))
}
 
#[inline(always)]
pub fn parse_version(buf: &[u8]) -> Option<(u8, &[u8])> {
    // Expect "HTTP/1.X\r\n"
    if buf.len() < 10 || !buf.starts_with(b"HTTP/1.") {
        return None;
    }
    let minor = match buf[7] {
        b'0' => 0,
        b'1' => 1,
        _ => return None,
    };
    if buf[8] != b'\r' || buf[9] != b'\n' {
        return None;
    }
    Some((minor, &buf[10..]))
}
 
#[inline(always)]
pub fn parse_header(buf: &[u8]) -> Option<(Header<'_>, &[u8])> {
    let colon = memchr_byte(b':', buf)?;
    let crlf = find_crlf(&buf[colon..])?;
    let crlf_abs = colon + crlf;
 
    let name = &buf[..colon];
    // Skip ": " (colon + optional whitespace)
    let value_start = colon + 1;
    let value = trim_ascii_start(&buf[value_start..crlf_abs]);
 
    Some((Header { name, value }, &buf[crlf_abs + 2..]))
}
 
#[inline(always)]
pub fn find_crlf(buf: &[u8]) -> Option<usize> {
    for i in 0..buf.len().saturating_sub(1) {
        if buf[i] == b'\r' && buf[i + 1] == b'\n' {
            return Some(i);
        }
    }
    None
}
 
#[inline(always)]
pub fn trim_ascii_start(buf: &[u8]) -> &[u8] {
    let start = buf.iter().position(|&b| b != b' ' && b != b'\t').unwrap_or(buf.len());
    &buf[start..]
}