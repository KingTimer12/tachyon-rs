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
///
/// Three response paths (priority: `json` > `array` > `body`):
///
/// ```typescript
/// // 1. json — object fields, serialized as {...} by Rust's zero-alloc JsonWriter
/// return { status: 200, json: [{ key: "message", value: "sucesso" }] };
///
/// // 2. array — array elements, serialized as [...] by Rust's zero-alloc JsonWriter
/// return { status: 200, array: [{ value: "item1" }, { value: "42", valueType: "number" }] };
///
/// // 3. body — plain text or custom content type
/// return { status: 200, body: "Hello!", contentType: "text" };
/// ```
#[napi(object)]
pub struct TachyonRawResponse {
  pub status: Option<u32>, // default: 200
  /// Plain body string. Used with `contentType` to set the response type.
  /// Lowest priority — ignored if `json` or `array` is present.
  pub body: Option<String>,
  /// "json" | "text" | "html" (default: "json"). Only applies to `body`.
  pub content_type: Option<String>,
  /// Custom HTTP headers as key-value pairs (e.g., CORS, Cache-Control).
  pub headers: Option<Vec<TachyonRawHeader>>,
  /// JSON object — fields serialized as `{...}` by Rust's zero-alloc JsonWriter.
  /// Bypasses `JSON.stringify()` entirely.
  pub json: Option<Vec<TachyonRawJsonField>>,
  /// JSON array — elements serialized as `[...]` by Rust's zero-alloc JsonWriter.
  pub array: Option<Vec<TachyonRawJsonField>>,
}

/// A typed JSON node for zero-alloc serialization via Rust's JsonWriter.
/// Supports any JSON shape: flat objects, nested objects, arrays, primitives.
///
/// ```typescript
/// return {
///   status: 200,
///   json: [
///     { key: "message", value: "Hello, World!" },
///     { key: "id", value: "42", valueType: "number" },
///     { key: "user", valueType: "object", children: [
///       { key: "name", value: "Aaron" },
///       { key: "age", value: "25", valueType: "number" },
///     ]},
///     { key: "tags", valueType: "array", children: [
///       { value: "fast" },
///       { value: "42", valueType: "number" },
///     ]},
///   ],
/// };
/// ```
#[napi(object)]
pub struct TachyonRawJsonField {
  /// Object key. Omit for array elements.
  pub key: Option<String>,
  /// Scalar value as string. Ignored for "object"/"array" types.
  pub value: Option<String>,
  /// "string" (default) | "number" | "bool" | "null" | "raw" | "object" | "array"
  pub value_type: Option<String>,
  /// Child fields for "object" or "array" types.
  pub children: Option<Vec<TachyonRawJsonField>>,
}

/// A single HTTP header key-value pair.
#[napi(object)]
pub struct TachyonRawHeader {
  pub name: String,
  pub value: String,
}
