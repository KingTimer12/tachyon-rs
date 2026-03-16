#![deny(clippy::all)]

use std::sync::Arc;

use napi::{Result, Status, bindgen_prelude::Function};
use napi::threadsafe_function::ThreadsafeFunctionCallMode;
use napi_derive::napi;

use crate::handle::{TachyonRawHeader, TachyonRawRequest, TachyonRawResponse};

mod handle;

/// Zero-alloc method → static str. Avoids Debug formatting per request.
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

/// Server configuration exposed to TypeScript.
///
/// ```typescript
/// import { TachyonRawConfig } from 'tachyon';
///
/// const config = new TachyonRawConfig();
/// config.bindAddr = '0.0.0.0:8080';
/// config.workers = 4;
/// ```
#[napi(object)]
#[derive(Debug, Clone)]
pub struct TachyonRawConfig {
    /// Address to bind (default: "0.0.0.0:3000")
    pub bind_addr: Option<String>,
    /// Number of worker threads (default: CPU count)
    pub workers: Option<u32>,
    /// Coroutine stack size in KB (default: 64)
    pub stack_size_kb: Option<u32>,
    /// Buffer pool size per worker (default: 512)
    pub buffers_per_worker: Option<u32>,
    /// Buffer size in bytes (default: 8192)
    pub buffer_size: Option<u32>,
    /// Handler timeout in seconds (default: 30)
    pub timeout_secs: Option<u32>,
    /// Enable TCP_NODELAY (default: true)
    pub tcp_nodelay: Option<bool>,
    /// Enable SO_REUSEPORT (default: true, Linux/BSD only)
    pub reuse_port: Option<bool>,
    /// Enable TCP Fast Open (default: true, Linux only)
    pub tcp_fastopen: Option<bool>,
    /// SO_BUSY_POLL microseconds (default: 0 = disabled, requires root)
    pub busy_poll_us: Option<i32>,
    /// SO_RCVBUF in bytes (default: 0 = OS default)
    pub recv_buf_size: Option<i32>,
    /// SO_SNDBUF in bytes (default: 0 = OS default)
    pub send_buf_size: Option<i32>,
    /// Security header preset: "none" | "basic" | "strict" (default: "basic")
    pub security: Option<String>,
}
 
impl From<TachyonRawConfig> for tachyon_core::config::ServerConfig {
    fn from(ts: TachyonRawConfig) -> Self {
        let mut config = tachyon_core::config::ServerConfig::new();
        if let Some(addr) = ts.bind_addr {
            config = config.bind(&addr);
        }
        if let Some(w) = ts.workers {
            config = config.workers(w as usize);
        }
        if let Some(s) = ts.stack_size_kb {
            config = config.stack_size((s as usize) * 1024);
        }
        if let Some(count) = ts.buffers_per_worker {
            let size = ts.buffer_size.unwrap_or(8192) as usize;
            config = config.buffer_pool(count as usize, size);
        }
        if let Some(t) = ts.timeout_secs {
            config = config.timeout(std::time::Duration::from_secs(t as u64));
        }
        // Socket tuning
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
        config
    }
}

/// The Tachyon server instance.
///
/// ```typescript
/// import { TachyonRawServer } from 'tachyon';
///
/// const server = new TachyonRawServer({ bindAddr: '0.0.0.0:8080' });
///
/// // The handler runs in Rust coroutines — fast, safe, cross-platform
/// server.start((req) => ({
///     status: 200,
///     body: JSON.stringify({ message: 'Hello from Tachyon!' }),
/// }));
/// ```
#[napi]
pub struct TachyonRawServer {
    config: tachyon_core::config::ServerConfig,
}
 
#[napi]
impl TachyonRawServer {
    #[napi(constructor)]
    pub fn new(config: Option<TachyonRawConfig>) -> Self {
        let config = config
            .map(|c| c.into())
            .unwrap_or_else(tachyon_core::config::ServerConfig::new);
        Self { config }
    }
 
    /// Start the server with a JavaScript handler function.
    ///
    /// The handler is called for each HTTP request from a Rust coroutine.
    /// It receives a `TachyonRequest` and must return a `TachyonResponse`.
    ///
    /// The server runs on background threads — Node's event loop stays free.
    #[napi]
    pub fn start(&self, handler: Function<TachyonRawRequest, TachyonRawResponse>) -> Result<()> {
        // Create a thread-safe reference to the JS function.
        // napi-rs handles the prevent-GC / prevent-drop dance internally.
        let handler_ref = handler.build_threadsafe_function()
            .build()?;

        let rust_handler: tachyon_core::server::Handler =
            Arc::new(move |req: &tachyon_http::http::Request, res: &mut tachyon_core::response::Response| {
                // Convert Rust request → JS-friendly struct.
                // method_str() returns &'static str — no allocation.
                // path and body require owned Strings for the FFI boundary.
                let ts_req = TachyonRawRequest {
                    method: method_str(req.method).to_string(),
                    path: req.path_str().to_string(),
                    body: if req.body.is_empty() {
                        None
                    } else {
                        String::from_utf8(req.body.to_vec()).ok()
                    },
                    headers: req.headers[..req.header_count]
                        .iter()
                        .filter_map(|h| h.as_ref())
                        .map(|h| TachyonRawHeader {
                            name: String::from_utf8_lossy(h.name).into_owned(),
                            value: String::from_utf8_lossy(h.value).into_owned(),
                        })
                        .collect(),
                };

                // Bridge sync coroutine ↔ async Node event loop:
                // 1. Create a rendezvous channel (1 slot, minimal overhead)
                // 2. call_with_return_value schedules JS on Node's event loop
                // 3. Callback sends the result through the channel
                // 4. Coroutine blocks on recv() until JS handler completes
                //
                // ThreadsafeFunctionCallMode::Blocking ensures the call
                // is enqueued even if Node's loop is saturated.
                let (tx, rx) = std::sync::mpsc::sync_channel::<Option<TachyonRawResponse>>(1);

                let status = handler_ref.call_with_return_value(
                    ts_req,
                    ThreadsafeFunctionCallMode::Blocking,
                    move |result: napi::Result<TachyonRawResponse>, _env| {
                        let _ = tx.send(result.ok());
                        Ok(())
                    },
                );

                if status != Status::Ok {
                    return res.json(500, b"{\"error\":\"handler call failed\"}");
                }

                match rx.recv() {
                    Ok(Some(ts_res)) => {
                        let status_code = ts_res.status.unwrap_or(200) as u16;
                        let body = ts_res.body.as_deref().unwrap_or("");
                        // Apply custom headers from the JS handler
                        if let Some(headers) = &ts_res.headers {
                            for h in headers {
                                res.header(h.name.as_bytes(), h.value.as_bytes());
                            }
                        }
                        match ts_res.content_type.as_deref().unwrap_or("json") {
                            "text" | "plain" => res.text(status_code, body.as_bytes()),
                            _ => res.json(status_code, body.as_bytes()),
                        }
                    }
                    // JS handler threw or channel disconnected
                    Ok(None) | Err(_) => {
                        res.json(500, b"{\"error\":\"handler error\"}")
                    }
                }
            });

        let server = tachyon_core::server::Server::new(self.config.clone());

        // Run server on a background thread so Node's event loop isn't blocked
        std::thread::spawn(move || {
            if let Err(e) = server.run(rust_handler) {
                eprintln!("[tachyon] Server error: {}", e);
            }
        });

        Ok(())
    }
}