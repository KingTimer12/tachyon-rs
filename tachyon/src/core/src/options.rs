use bytes::Bytes;
use serde_json::Value;

/// Response data returned by handlers
#[derive(Clone)]
pub struct ResponseData {
    pub data: Bytes,
    pub status_code: u16,
}

impl ResponseData {
    /// Create a new response with data and status code
    #[inline(always)]
    pub fn new(data: impl Into<Bytes>, status_code: u16) -> Self {
        Self {
            data: data.into(),
            status_code,
        }
    }

    /// Create a 200 OK response
    #[inline(always)]
    pub fn ok(data: impl Into<Bytes>) -> Self {
        Self::new(data, 200)
    }

    /// Create a 404 Not Found response
    #[inline(always)]
    pub fn not_found() -> Self {
        Self {
            data: Bytes::from_static(b"{\"error\":\"Not Found\"}"),
            status_code: 404,
        }
    }

    /// Create a 500 Internal Server Error response
    #[inline(always)]
    pub fn internal_error() -> Self {
        Self {
            data: Bytes::from_static(b"{\"error\":\"Internal Server Error\"}"),
            status_code: 500,
        }
    }

    /// Create a 400 Bad Request response
    #[inline(always)]
    pub fn bad_request() -> Self {
        Self {
            data: Bytes::from_static(b"{\"error\":\"Bad Request\"}"),
            status_code: 400,
        }
    }
}

/// Simplified request options passed to handlers
/// No Arc overhead - just the data needed
pub struct TachyonOptions {
    /// Parsed JSON body (only for POST/PUT/PATCH)
    pub body: Option<Value>,
    /// URL parameters extracted from path
    pub params: Option<Value>,
}

impl TachyonOptions {
    /// Create empty options (for GET/DELETE without params)
    #[inline(always)]
    pub fn empty() -> Self {
        Self {
            body: None,
            params: None,
        }
    }

    /// Create options with body only
    #[inline(always)]
    pub fn with_body(body: Value) -> Self {
        Self {
            body: Some(body),
            params: None,
        }
    }

    /// Create options with params only
    #[inline(always)]
    pub fn with_params(params: Value) -> Self {
        Self {
            body: None,
            params: Some(params),
        }
    }

    /// Create options with both body and params
    #[inline(always)]
    pub fn with_body_and_params(body: Value, params: Value) -> Self {
        Self {
            body: Some(body),
            params: Some(params),
        }
    }
}

impl Default for TachyonOptions {
    #[inline(always)]
    fn default() -> Self {
        Self::empty()
    }
}
