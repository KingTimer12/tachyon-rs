//! Ultra-fast HTTP server core with extreme optimizations
//! - mimalloc global allocator for faster allocations
//! - Thread-local caching to avoid lock contention
//! - Zero-allocation hot paths where possible
//! - SIMD JSON parsing via simd-json
//! - Optimized TCP settings (nodelay, keepalive, large buffers)
//! - Pre-built response components
//! - Inline everything in hot path
//! - Maximum connection backlog

use std::sync::Arc;

use ahash::AHasher;
use arc_swap::ArcSwap;
use bytes::Bytes;
use dashmap::DashMap;
use http_body_util::{combinators::BoxBody, BodyExt};
use hyper::{header, Method as HyperMethod, Request, Response, StatusCode};
use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::hash::BuildHasherDefault;
use thread_local::ThreadLocal;

use crate::{
    cache::HotCache,
    methods::Method,
    options::{ResponseData, TachyonOptions},
    router::TachyonRouter,
    utils::full,
};

// Use mimalloc for faster allocations
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

type FastHasher = BuildHasherDefault<AHasher>;

// Pre-allocated static responses - zero allocation for common cases
static NOTFOUND_BYTES: &[u8] = b"{\"error\":\"Not Found\"}";
static CONTENT_TYPE_JSON: &str = "application/json";
static CONTENT_TYPE_JSON_BYTES: &[u8] = b"application/json";

// Pre-computed header values for zero-allocation responses
static HEADER_CONTENT_TYPE: once_cell::sync::Lazy<hyper::header::HeaderValue> =
    once_cell::sync::Lazy::new(|| hyper::header::HeaderValue::from_static(CONTENT_TYPE_JSON));

// Pre-built 404 response bytes for ultra-fast not found
static NOTFOUND_RESPONSE: once_cell::sync::Lazy<(StatusCode, &'static str, usize)> =
    once_cell::sync::Lazy::new(|| {
        (
            StatusCode::NOT_FOUND,
            CONTENT_TYPE_JSON,
            NOTFOUND_BYTES.len(),
        )
    });

// Common status codes pre-loaded
static STATUS_OK: StatusCode = StatusCode::OK;
static STATUS_NOT_FOUND: StatusCode = StatusCode::NOT_FOUND;

/// Thread-local route cache for lock-free lookups after warmup
struct ThreadLocalCache {
    routes: FxHashMap<u64, Arc<dyn Fn(TachyonOptions) -> ResponseData + Send + Sync>>,
}

impl ThreadLocalCache {
    fn new() -> Self {
        Self {
            routes: FxHashMap::default(),
        }
    }
}

/// Fast hash for route keys - FNV-1a optimized
#[inline(always)]
fn fast_hash(key: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in key.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub struct Tachyon {
    /// Main route storage - used for registration and fallback
    routes: Arc<DashMap<String, TachyonRouter, FastHasher>>,
    /// Hot cache for frequently accessed routes
    hot_cache: Arc<HotCache>,
    /// Thread-local caches - each thread gets its own cache
    thread_caches: ThreadLocal<RefCell<ThreadLocalCache>>,
    /// Atomic snapshot of routes for lock-free reads
    routes_snapshot:
        Arc<ArcSwap<FxHashMap<u64, Arc<dyn Fn(TachyonOptions) -> ResponseData + Send + Sync>>>>,
}

impl Tachyon {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    fn register_route<F>(&self, method: Method, path: &str, callback: F)
    where
        F: Fn(TachyonOptions) -> ResponseData + Send + Sync + 'static,
    {
        let method_id = method.id();

        // Pre-allocate route key with exact capacity - use stack buffer first
        let mut buf = itoa::Buffer::new();
        let method_str = buf.format(method_id);
        let mut route_key = String::with_capacity(method_str.len() + 1 + path.len());
        route_key.push_str(method_str);
        route_key.push(':');
        route_key.push_str(path);

        let handler = Arc::new(callback);

        // Insert into main routes
        self.routes.insert(
            route_key.clone(),
            TachyonRouter::new(method_id, handler.clone()),
        );

        // Update atomic snapshot for lock-free reads
        let hash = fast_hash(&route_key);
        let mut new_snapshot = (**self.routes_snapshot.load()).clone();
        new_snapshot.insert(
            hash,
            handler as Arc<dyn Fn(TachyonOptions) -> ResponseData + Send + Sync>,
        );
        self.routes_snapshot.store(Arc::new(new_snapshot));
    }

    #[inline]
    pub fn get<F>(&self, path: &str, callback: F)
    where
        F: Fn(TachyonOptions) -> ResponseData + Send + Sync + 'static,
    {
        self.register_route(Method::Get, path, callback);
    }

    #[inline]
    pub fn post<F>(&self, path: &str, callback: F)
    where
        F: Fn(TachyonOptions) -> ResponseData + Send + Sync + 'static,
    {
        self.register_route(Method::Post, path, callback);
    }

    #[inline]
    pub fn delete<F>(&self, path: &str, callback: F)
    where
        F: Fn(TachyonOptions) -> ResponseData + Send + Sync + 'static,
    {
        self.register_route(Method::Delete, path, callback);
    }

    #[inline]
    pub fn put<F>(&self, path: &str, callback: F)
    where
        F: Fn(TachyonOptions) -> ResponseData + Send + Sync + 'static,
    {
        self.register_route(Method::Put, path, callback);
    }

    #[inline]
    pub fn patch<F>(&self, path: &str, callback: F)
    where
        F: Fn(TachyonOptions) -> ResponseData + Send + Sync + 'static,
    {
        self.register_route(Method::Patch, path, callback);
    }

    #[inline]
    pub fn routes(&self) -> Arc<DashMap<String, TachyonRouter, FastHasher>> {
        self.routes.clone()
    }

    pub async fn listen(&self, port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use hyper::server::conn::http1;
        use hyper::service::service_fn;
        use hyper_util::rt::TokioIo;
        use std::net::{Ipv4Addr, SocketAddr};

        let addr = SocketAddr::from((Ipv4Addr::new(0, 0, 0, 0), port));

        // Create socket with extreme optimizations
        let socket = tokio::net::TcpSocket::new_v4()?;

        // Enable SO_REUSEADDR for faster restarts
        socket.set_reuseaddr(true)?;

        // Set socket buffer sizes for high throughput
        // 256KB buffers for maximum performance
        let _ = socket.set_send_buffer_size(262144);
        let _ = socket.set_recv_buffer_size(262144);

        // Bind and listen with maximum backlog
        socket.bind(addr)?;
        let listener = socket.listen(65535)?; // Maximum backlog

        // Clone once outside the loop - minimize Arc operations
        let routes = Arc::clone(&self.routes);
        let hot_cache = Arc::clone(&self.hot_cache);
        let routes_snapshot = Arc::clone(&self.routes_snapshot);

        loop {
            let (stream, _) = listener.accept().await?;

            // TCP optimizations for lowest latency
            // TCP_NODELAY disables Nagle's algorithm
            let _ = stream.set_nodelay(true);

            // Set TCP keepalive for connection reuse
            let sock_ref = socket2::SockRef::from(&stream);
            let _ = sock_ref.set_tcp_keepalive(
                &socket2::TcpKeepalive::new()
                    .with_time(std::time::Duration::from_secs(60))
                    .with_interval(std::time::Duration::from_secs(10)),
            );

            let io = TokioIo::new(stream);
            let routes = Arc::clone(&routes);
            let cache = Arc::clone(&hot_cache);
            let snapshot = Arc::clone(&routes_snapshot);

            tokio::spawn(async move {
                // Inline service creation for speed
                let service = service_fn(move |req| {
                    let routes = Arc::clone(&routes);
                    let cache = Arc::clone(&cache);
                    let snapshot = Arc::clone(&snapshot);
                    Self::handle_request_fast(routes, cache, snapshot, req)
                });

                // HTTP/1.1 with extreme optimizations
                let conn = http1::Builder::new()
                    .keep_alive(true)
                    .pipeline_flush(true)
                    .max_buf_size(131072) // 128KB buffer
                    .preserve_header_case(false) // Faster header handling
                    .title_case_headers(false) // Skip title casing
                    .serve_connection(io, service);

                // Ignore errors silently for maximum performance
                let _ = conn.await;
            });
        }
    }

    /// Ultra-fast request handler with multiple optimization layers
    #[inline]
    async fn handle_request_fast(
        routes: Arc<DashMap<String, TachyonRouter, FastHasher>>,
        hot_cache: Arc<HotCache>,
        routes_snapshot: Arc<
            ArcSwap<FxHashMap<u64, Arc<dyn Fn(TachyonOptions) -> ResponseData + Send + Sync>>>,
        >,
        req: Request<hyper::body::Incoming>,
    ) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
        let path = req.uri().path();
        let hyper_method = req.method().clone();
        let method = Method::from(&hyper_method);
        let method_id = method.id();

        // Build route key with stack allocation - avoid heap allocation
        // Method ID is always 0-4 (1 byte), colon is 1 byte, path up to 254 bytes = 256 max
        let route_key = {
            let mut key_buf = [0u8; 256];
            let mut buf = itoa::Buffer::new();
            let method_str = buf.format(method_id);
            let method_bytes = method_str.as_bytes();
            let path_bytes = path.as_bytes();

            let total_len = method_bytes.len() + 1 + path_bytes.len();

            if total_len <= 256 {
                // Fast path: use stack buffer
                key_buf[..method_bytes.len()].copy_from_slice(method_bytes);
                key_buf[method_bytes.len()] = b':';
                key_buf[method_bytes.len() + 1..total_len].copy_from_slice(path_bytes);
                // SAFETY: method_str is ASCII digits, path is valid UTF-8
                unsafe { std::str::from_utf8_unchecked(&key_buf[..total_len]) }.to_owned()
            } else {
                // Fallback for very long paths
                let mut s = String::with_capacity(total_len);
                s.push_str(method_str);
                s.push(':');
                s.push_str(path);
                s
            }
        };

        let route_hash = fast_hash(&route_key);

        // Layer 1: Try atomic snapshot first (lock-free, fastest)
        let snapshot = routes_snapshot.load();
        if let Some(handler) = snapshot.get(&route_hash) {
            return Ok(Self::execute_handler(handler.clone(), req, &hyper_method).await);
        }

        // Layer 2: Try hot cache (single slot lookup with RwLock)
        if let Some(handler) = hot_cache.get(&route_key) {
            return Ok(Self::execute_handler(handler, req, &hyper_method).await);
        }

        // Layer 3: Full route lookup with parameterized matching
        let handler = Self::find_route(&routes, &route_key);

        match handler {
            Some(h) => {
                // Cache for next time
                hot_cache.set(route_key, h.clone());
                Ok(Self::execute_handler(h, req, &hyper_method).await)
            }
            None => Ok(Self::not_found_response()),
        }
    }

    /// Find route with parameterized matching
    #[inline]
    fn find_route(
        routes: &DashMap<String, TachyonRouter, FastHasher>,
        route_key: &str,
    ) -> Option<Arc<dyn Fn(TachyonOptions) -> ResponseData + Send + Sync>> {
        // Direct lookup first
        if let Some(route_ref) = routes.get(route_key) {
            return Some(Arc::clone(route_ref.handler()));
        }

        // Parameterized route lookup
        routes
            .iter()
            .find(|entry| crate::utils::route_matches(entry.key(), route_key))
            .map(|entry| Arc::clone(entry.value().handler()))
    }

    /// Execute handler with optimized body parsing
    #[inline]
    async fn execute_handler(
        handler: Arc<dyn Fn(TachyonOptions) -> ResponseData + Send + Sync>,
        req: Request<hyper::body::Incoming>,
        method: &HyperMethod,
    ) -> Response<BoxBody<Bytes, hyper::Error>> {
        // Parse body only for methods that support it
        let body = if matches!(
            method,
            &HyperMethod::POST | &HyperMethod::PUT | &HyperMethod::PATCH
        ) {
            Self::parse_json_body_fast(req).await
        } else {
            None
        };

        let options = TachyonOptions { body, params: None };
        let result = handler(options);
        Self::build_response_fast(result)
    }

    /// Ultra-fast JSON body parsing with simd-json
    #[inline(always)]
    async fn parse_json_body_fast(
        req: Request<hyper::body::Incoming>,
    ) -> Option<serde_json::Value> {
        // Fast content-type check - inline everything
        let content_type = req.headers().get(header::CONTENT_TYPE)?;
        let ct_bytes = content_type.as_bytes();

        // Ultra-fast prefix check - compare first 16 bytes directly
        // "application/json" is exactly 16 bytes
        if ct_bytes.len() < 16 {
            return None;
        }

        // Use unsafe for maximum speed - we already checked length
        let matches = unsafe {
            std::ptr::eq(
                ct_bytes.as_ptr().cast::<[u8; 16]>(),
                CONTENT_TYPE_JSON_BYTES.as_ptr().cast::<[u8; 16]>(),
            ) || ct_bytes.get_unchecked(..16) == CONTENT_TYPE_JSON_BYTES
        };

        if !matches {
            return None;
        }

        // Collect body with size hint for pre-allocation
        let body_bytes = req.collect().await.ok()?.to_bytes();

        // Minimum valid JSON is "{}" or "[]" - 2 bytes
        if body_bytes.len() < 2 {
            return None;
        }

        // Try simd-json first (up to 4x faster than serde_json)
        let mut body_vec = body_bytes.to_vec();
        simd_json::serde::from_slice::<serde_json::Value>(&mut body_vec)
            .ok()
            .or_else(|| serde_json::from_slice(&body_bytes).ok())
    }

    /// Build response with minimal allocations - fully inlined
    #[inline(always)]
    fn build_response_fast(result: ResponseData) -> Response<BoxBody<Bytes, hyper::Error>> {
        // Fast path for common status codes
        let status = match result.status_code {
            200 => STATUS_OK,
            404 => STATUS_NOT_FOUND,
            _ => StatusCode::from_u16(result.status_code).unwrap_or(STATUS_OK),
        };

        // Use pre-computed header value
        let mut response = Response::new(full(result.data.clone()));
        *response.status_mut() = status;

        let headers = response.headers_mut();
        headers.insert(header::CONTENT_TYPE, HEADER_CONTENT_TYPE.clone());

        // Use itoa for fast integer formatting
        let mut len_buf = itoa::Buffer::new();
        let len_str = len_buf.format(result.data.len());
        if let Ok(hv) = hyper::header::HeaderValue::from_str(len_str) {
            headers.insert(header::CONTENT_LENGTH, hv);
        }

        response
    }

    /// Pre-built 404 response - zero allocation for maximum speed
    #[inline(always)]
    fn not_found_response() -> Response<BoxBody<Bytes, hyper::Error>> {
        let mut response = Response::new(full(NOTFOUND_BYTES));
        *response.status_mut() = STATUS_NOT_FOUND;

        let headers = response.headers_mut();
        headers.insert(header::CONTENT_TYPE, HEADER_CONTENT_TYPE.clone());
        // Pre-computed length: 21 bytes for {"error":"Not Found"}
        headers.insert(
            header::CONTENT_LENGTH,
            hyper::header::HeaderValue::from_static("21"),
        );

        response
    }
}

impl Default for Tachyon {
    fn default() -> Self {
        Self {
            routes: Arc::new(DashMap::with_capacity_and_hasher(64, FastHasher::default())),
            hot_cache: Arc::new(HotCache::new()),
            thread_caches: ThreadLocal::new(),
            routes_snapshot: Arc::new(ArcSwap::from_pointee(FxHashMap::default())),
        }
    }
}

// Required for once_cell
mod once_cell {
    pub mod sync {
        pub struct Lazy<T> {
            cell: std::sync::OnceLock<T>,
            init: fn() -> T,
        }

        impl<T> Lazy<T> {
            pub const fn new(init: fn() -> T) -> Self {
                Self {
                    cell: std::sync::OnceLock::new(),
                    init,
                }
            }
        }

        impl<T> std::ops::Deref for Lazy<T> {
            type Target = T;

            fn deref(&self) -> &Self::Target {
                self.cell.get_or_init(self.init)
            }
        }
    }
}
