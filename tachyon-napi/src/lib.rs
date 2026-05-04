#![deny(clippy::all)]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use napi::threadsafe_function::ThreadsafeFunctionCallMode;
use napi::{Result, Status, bindgen_prelude::Function};
use napi_derive::napi;

use crate::handle::{TachyonRawJsonField, TachyonRawRequest, TachyonRawResponse};

mod handle;

/// Per-route async handler: receives owned request, returns a WriteFn future.
type AsyncRouteFn = Arc<
  dyn Fn(TachyonRawRequest) -> Pin<Box<dyn Future<Output = tachyon_core::server::WriteFn> + Send>>
    + Send
    + Sync,
>;

/// Map: method_id → path_bytes → handler
type RouteMap = Arc<HashMap<u8, HashMap<Box<[u8]>, AsyncRouteFn>>>;

/// Zero-alloc method → u8 id for HashMap key.
#[inline(always)]
fn method_to_id(m: tachyon_http::methods::Method) -> u8 {
  match m {
    tachyon_http::methods::Method::Get => 0,
    tachyon_http::methods::Method::Post => 1,
    tachyon_http::methods::Method::Put => 2,
    tachyon_http::methods::Method::Delete => 3,
    tachyon_http::methods::Method::Patch => 4,
    tachyon_http::methods::Method::Head => 5,
    tachyon_http::methods::Method::Options => 6,
    tachyon_http::methods::Method::Other => 7,
  }
}

/// Zero-alloc method → static str. For passing to JS handlers.
#[inline(always)]
fn method_str(m: tachyon_http::methods::Method) -> &'static str {
  match m {
    tachyon_http::methods::Method::Get => "GET",
    tachyon_http::methods::Method::Post => "POST",
    tachyon_http::methods::Method::Put => "PUT",
    tachyon_http::methods::Method::Delete => "DELETE",
    tachyon_http::methods::Method::Patch => "PATCH",
    tachyon_http::methods::Method::Head => "HEAD",
    tachyon_http::methods::Method::Options => "OPTIONS",
    tachyon_http::methods::Method::Other => "OTHER",
  }
}

/// Parse string method name → u8 id (used when registering routes from JS).
fn str_to_method_id(s: &str) -> u8 {
  match s {
    "GET" => 0,
    "POST" => 1,
    "PUT" => 2,
    "DELETE" => 3,
    "PATCH" => 4,
    "HEAD" => 5,
    "OPTIONS" => 6,
    _ => 7,
  }
}

/// Build flat headers string: "name\tvalue\n..." — 1 allocation for the whole header block.
fn build_flat_headers(req: &tachyon_http::http::Request<'_>) -> String {
  let cap = req.headers[..req.header_count]
    .iter()
    .filter_map(|h| h.as_ref())
    .map(|h| h.name.len() + h.value.len() + 2)
    .sum();
  let mut s = String::with_capacity(cap);
  for h in &req.headers[..req.header_count] {
    if let Some(h) = h.as_ref() {
      let name = unsafe { std::str::from_utf8_unchecked(h.name) };
      let value = unsafe { std::str::from_utf8_unchecked(h.value) };
      s.push_str(name);
      s.push('\t');
      s.push_str(value);
      s.push('\n');
    }
  }
  s
}

/// Recursively serialize a `TachyonRawJsonField` into the `JsonWriter`.
fn write_json_field(w: &mut tachyon_http::json::JsonWriter, f: &TachyonRawJsonField) {
  if let Some(ref key) = f.key {
    w.key(key);
  }
  match f.value_type.as_deref().unwrap_or("string") {
    "object" => {
      w.object(|w| {
        if let Some(ref children) = f.children {
          for child in children {
            write_json_field(w, child);
          }
        }
      });
    }
    "array" => {
      w.array(|w| {
        if let Some(ref children) = f.children {
          for child in children {
            write_json_field(w, child);
          }
        }
      });
    }
    "number" | "bool" | "null" | "raw" => {
      w.raw(f.value.as_deref().unwrap_or("null").as_bytes());
    }
    _ => {
      w.string(f.value.as_deref().unwrap_or(""));
    }
  }
}

/// Build the WriteFn closure from a JS response (or None on handler error).
fn make_write_fn(ts_res_opt: Option<TachyonRawResponse>) -> tachyon_core::server::WriteFn {
  Box::new(
    move |res: &mut tachyon_core::response::Response<'_>| match ts_res_opt {
      None => res.json(500, b"{\"error\":\"handler error\"}"),
      Some(ts_res) => {
        let status_code = ts_res.status.unwrap_or(200) as u16;
        if let Some(headers) = &ts_res.headers {
          for h in headers {
            res.header(h.name.as_bytes(), h.value.as_bytes());
          }
        }
        if let Some(fields) = &ts_res.json {
          res.json_writer(status_code, |w| {
            w.object(|w| {
              for f in fields {
                write_json_field(w, f);
              }
            });
          })
        } else if let Some(elements) = &ts_res.array {
          res.json_writer(status_code, |w| {
            w.array(|w| {
              for f in elements {
                write_json_field(w, f);
              }
            });
          })
        } else {
          let body = ts_res.body.as_deref().unwrap_or("");
          match ts_res.content_type.as_deref().unwrap_or("json") {
            "text" | "plain" => res.text(status_code, body.as_bytes()),
            _ => res.json(status_code, body.as_bytes()),
          }
        }
      }
    },
  )
}

/// Server configuration exposed to TypeScript.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct TachyonRawConfig {
  pub bind_addr: Option<String>,
  pub buffer_size: Option<u32>,
  pub timeout_secs: Option<u32>,
  pub tcp_nodelay: Option<bool>,
  pub reuse_port: Option<bool>,
  pub tcp_fastopen: Option<bool>,
  pub busy_poll_us: Option<i32>,
  pub recv_buf_size: Option<i32>,
  pub send_buf_size: Option<i32>,
  pub security: Option<String>,
  pub compression_threshold: Option<i32>,
  pub catch_panics: Option<bool>,
}

impl From<TachyonRawConfig> for tachyon_core::config::ServerConfig {
  fn from(ts: TachyonRawConfig) -> Self {
    let mut config = tachyon_core::config::ServerConfig::new();
    if let Some(addr) = ts.bind_addr {
      config = config.bind(&addr);
    }
    if let Some(t) = ts.timeout_secs {
      config = config.timeout(std::time::Duration::from_secs(t as u64));
    }
    if let Some(v) = ts.tcp_nodelay {
      config = config.tcp_nodelay(v);
    }
    if let Some(v) = ts.reuse_port {
      config = config.reuse_port(v);
    }
    if let Some(v) = ts.tcp_fastopen {
      config = config.tcp_fastopen(v);
    }
    if let Some(v) = ts.busy_poll_us {
      config = config.busy_poll(v);
    }
    if let Some(v) = ts.recv_buf_size {
      config = config.recv_buffer(v);
    }
    if let Some(v) = ts.send_buf_size {
      config = config.send_buffer(v);
    }
    if let Some(ref s) = ts.security {
      let preset = match s.as_str() {
        "none" => tachyon_http::response::SecurityPreset::None,
        "strict" => tachyon_http::response::SecurityPreset::Strict,
        _ => tachyon_http::response::SecurityPreset::Basic,
      };
      config = config.security(preset);
    }
    if let Some(v) = ts.compression_threshold {
      if v < 0 {
        config = config.compression(usize::MAX);
      } else {
        config = config.compression(v as usize);
      }
    }
    if let Some(v) = ts.catch_panics {
      config = config.catch_panics(v);
    }
    config
  }
}

/// The Tachyon server instance. Routes are registered in Rust for zero-overhead dispatch.
#[napi]
pub struct TachyonRawServer {
  config: tachyon_core::config::ServerConfig,
  /// Registered routes: (method_id, path_bytes, handler)
  routes: Vec<(u8, Box<[u8]>, AsyncRouteFn)>,
}

#[napi]
impl TachyonRawServer {
  #[napi(constructor)]
  pub fn new(config: Option<TachyonRawConfig>) -> Self {
    let config = config.map(|c| c.into()).unwrap_or_default();
    Self {
      config,
      routes: Vec::new(),
    }
  }

  /// Register a route handler. Called once per route at startup from TypeScript.
  ///
  /// The handler receives a `TachyonRawRequest` and returns a `TachyonRawResponse`.
  /// Routing and 404 handling happen entirely in Rust — no JS call for unmatched paths.
  #[napi]
  pub fn route(
    &mut self,
    method: String,
    path: String,
    handler: Function<TachyonRawRequest, TachyonRawResponse>,
  ) -> Result<()> {
    // Let Rust infer the full ThreadsafeFunction type from the Function parameter.
    let ts_fn = Arc::new(handler.build_threadsafe_function().build()?);

    let route_fn: AsyncRouteFn = Arc::new(move |req: TachyonRawRequest| {
      let ts_fn = ts_fn.clone();
      Box::pin(async move {
        let (tx, rx) = tokio::sync::oneshot::channel::<Option<TachyonRawResponse>>();
        let status = ts_fn.call_with_return_value(
          req,
          ThreadsafeFunctionCallMode::NonBlocking,
          move |result: napi::Result<TachyonRawResponse>, _env| {
            let _ = tx.send(result.ok());
            Ok(())
          },
        );
        let ts_res_opt = if status != Status::Ok {
          None
        } else {
          rx.await.ok().flatten()
        };
        make_write_fn(ts_res_opt)
      }) as Pin<Box<dyn Future<Output = tachyon_core::server::WriteFn> + Send>>
    });

    let method_id = str_to_method_id(&method);
    let path_bytes: Box<[u8]> = path.into_bytes().into_boxed_slice();
    self.routes.push((method_id, path_bytes, route_fn));
    Ok(())
  }

  /// Start the server. Must be called after all routes are registered.
  ///
  /// Builds an O(1) route map and starts the Tokio runtime on a background thread.
  #[napi]
  pub fn listen(&self) -> Result<()> {
    // Build O(1) lookup maps
    let mut route_map: HashMap<u8, HashMap<Box<[u8]>, AsyncRouteFn>> = HashMap::new();
    for (method_id, path, handler) in &self.routes {
      route_map
        .entry(*method_id)
        .or_default()
        .insert(path.clone(), handler.clone());
    }
    let route_map: RouteMap = Arc::new(route_map);

    let rust_handler: tachyon_core::server::Handler =
      Arc::new(move |req: &tachyon_http::http::Request<'_>| {
        let method_id = method_to_id(req.method);

        // Strip query string for routing lookup
        let full_path = req.path_str();
        let route_path = full_path.split('?').next().unwrap_or(full_path);

        let handler = route_map
          .get(&method_id)
          .and_then(|m| m.get(route_path.as_bytes()));

        if let Some(handler) = handler {
          // Extract all request data synchronously — owned, so the future is 'static
          let method = method_str(req.method).to_string();
          let path = full_path.to_string();
          let body = if req.body.is_empty() {
            None
          } else {
            Some(match std::str::from_utf8(req.body) {
              Ok(s) => s.to_string(),
              Err(_) => String::from_utf8_lossy(req.body).into_owned(),
            })
          };
          let headers = build_flat_headers(req);
          let ts_req = TachyonRawRequest {
            method,
            path,
            body,
            headers,
          };
          let handler = handler.clone();
          Box::pin(async move { handler(ts_req).await })
        } else {
          // 404 handled entirely in Rust — zero JS overhead
          Box::pin(async move {
            Box::new(|res: &mut tachyon_core::response::Response<'_>| {
              res.json(404, b"{\"error\":\"not found\"}")
            }) as tachyon_core::server::WriteFn
          })
        }
      });

    let server = tachyon_core::server::Server::new(self.config.clone());
    std::thread::spawn(move || {
      if let Err(e) = server.run(rust_handler) {
        eprintln!("[tachyon] Server error: {}", e);
      }
    });

    Ok(())
  }
}
