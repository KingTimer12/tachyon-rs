#![deny(clippy::all)]
use napi_derive::napi;

/// Parsed HTTP request exposed to TypeScript callbacks.
///
/// ```typescript
/// server.handle(({ method, path, body }) => {
///     console.log(req.method, req.path);
///     return { status: 200, body: '{"ok": true}' };
/// });
/// ```
#[napi(object)]
pub struct TachyonRawRequest {
  pub method: String,
  pub path: String,
  pub body: Option<String>,
  pub headers: Vec<TachyonRawHeader>,
}

/// Response from TypeScript handler.
#[napi(object)]
pub struct TachyonRawResponse {
  pub status: Option<u32>,  // default: 200
  pub body: Option<String>, // default: empty ('cause of status 204)
  /// "json" | "text" | "html" (default: "json")
  pub content_type: Option<String>,
  /// Custom HTTP headers as key-value pairs (e.g., CORS, Cache-Control).
  pub headers: Option<Vec<TachyonRawHeader>>,
}

/// A single HTTP header key-value pair.
#[napi(object)]
pub struct TachyonRawHeader {
  pub name: String,
  pub value: String,
}
