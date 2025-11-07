use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};

#[inline(always)]
pub fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
  Full::new(chunk.into())
    .map_err(|never| match never {})
    .boxed()
}

#[inline(always)]
pub fn empty() -> BoxBody<Bytes, hyper::Error> {
  Empty::<Bytes>::new()
    .map_err(|never| match never {})
    .boxed()
}

#[inline(always)]
pub fn route_matches(route_pattern: &str, actual_route: &str) -> bool {
  let pattern_bytes = route_pattern.as_bytes();
  let actual_bytes = actual_route.as_bytes();

  // Fast path: if lengths are very different, can't match
  let len_diff = pattern_bytes.len().abs_diff(actual_bytes.len());
  if len_diff > 50 {
    return false;
  }

  // Fast method separator search using memchr-style approach
  let pattern_colon = match pattern_bytes.iter().position(|&b| b == b':') {
    Some(pos) => pos,
    None => return false,
  };

  let actual_colon = match actual_bytes.iter().position(|&b| b == b':') {
    Some(pos) => pos,
    None => return false,
  };

  // Fast method comparison (usually just 1 byte: 0-4)
  if pattern_colon != actual_colon || pattern_bytes[..pattern_colon] != actual_bytes[..actual_colon]
  {
    return false;
  }

  // Get paths after method (skip the ':')
  let pattern_path = unsafe { pattern_bytes.get_unchecked(pattern_colon + 1..) };
  let actual_path = unsafe { actual_bytes.get_unchecked(actual_colon + 1..) };

  // Fast path: exact byte match (no parameters)
  if pattern_path == actual_path {
    return true;
  }

  // Fast path: if no colon in pattern path, must be exact match
  if !pattern_path.contains(&b':') {
    return false;
  }

  // Zero-allocation segment matching using byte slices
  let mut pattern_iter = pattern_path.split(|&b| b == b'/');
  let mut actual_iter = actual_path.split(|&b| b == b'/');

  loop {
    match (pattern_iter.next(), actual_iter.next()) {
      (Some(pattern_seg), Some(actual_seg)) => {
        // Parameter segment (starts with ':')
        if !pattern_seg.is_empty() && pattern_seg[0] == b':' {
          // Parameters can't be empty
          if actual_seg.is_empty() {
            return false;
          }
          continue;
        }

        // Exact match required
        if pattern_seg != actual_seg {
          return false;
        }
      }
      (None, None) => return true, // Both exhausted = match
      _ => return false,           // Length mismatch
    }
  }
}