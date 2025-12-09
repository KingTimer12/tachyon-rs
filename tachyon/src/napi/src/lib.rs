#![deny(clippy::all)]

//! Ultra-optimized NAPI bindings for Tachyon
//!
//! Optimizations:
//! - Arc-swap for lock-free callback access
//! - Bounded crossbeam channels for fast sync
//! - Pre-allocated buffers where possible
//! - simd-json for fast serialization
//! - Minimal allocations in hot path
//! - Zero-copy where possible
//! - Inline hot paths

use arc_swap::ArcSwap;
use bytes::Bytes;
use crossbeam_channel::bounded;
use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi_derive::napi;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tachyon_core::options::{ResponseData, TachyonOptions};
use tachyon_core::Tachyon as CoreTachyon;

// Pre-allocated error responses - zero allocation on error path
static ERROR_CALLBACK_FAILED: Bytes = Bytes::from_static(b"{\"error\":\"Callback failed\"}");
static ERROR_ROUTE_NOT_FOUND: Bytes = Bytes::from_static(b"{\"error\":\"Route not found\"}");
static ERROR_TIMEOUT: Bytes = Bytes::from_static(b"{\"error\":\"Callback timeout\"}");

/// Data passed to JS callback - optimized structure
#[napi(object)]
#[derive(Clone)]
pub struct JsCallbackData {
    pub body: Option<String>,
    pub params: Option<String>,
}

/// Result from JS callback
#[napi(object)]
#[derive(Clone, Debug)]
pub struct JsCallbackResult {
    pub data: String,
    pub status: u16,
}

/// High-performance callback type with return value support
type CallbackFn =
    ThreadsafeFunction<JsCallbackData, JsCallbackResult, JsCallbackData, napi::Status, true>;

/// Callback entry with pre-computed hash and atomic stats
struct CallbackEntry {
    callback: Arc<CallbackFn>,
    /// Call counter for profiling
    call_count: AtomicU64,
}

impl CallbackEntry {
    fn new(callback: CallbackFn) -> Self {
        Self {
            callback: Arc::new(callback),
            call_count: AtomicU64::new(0),
        }
    }

    #[inline(always)]
    fn call(&self) -> &Arc<CallbackFn> {
        self.call_count.fetch_add(1, Ordering::Relaxed);
        &self.callback
    }
}

/// Lock-free callback storage using ArcSwap
/// Reads are completely lock-free after initial registration
struct CallbackStore {
    /// Primary storage - lock-free reads via ArcSwap
    snapshot: ArcSwap<FxHashMap<u64, Arc<CallbackEntry>>>,
    /// Write lock only used during registration
    write_lock: RwLock<()>,
}

impl CallbackStore {
    fn new() -> Self {
        Self {
            snapshot: ArcSwap::from_pointee(FxHashMap::default()),
            write_lock: RwLock::new(()),
        }
    }

    /// Lock-free get - no synchronization needed
    #[inline(always)]
    fn get(&self, hash: u64) -> Option<Arc<CallbackEntry>> {
        self.snapshot.load().get(&hash).cloned()
    }

    /// Insert with write lock (only during setup)
    fn insert(&self, hash: u64, entry: CallbackEntry) {
        let _guard = self.write_lock.write();
        let mut new_map = (**self.snapshot.load()).clone();
        new_map.insert(hash, Arc::new(entry));
        self.snapshot.store(Arc::new(new_map));
    }
}

/// FNV-1a hash for route keys
#[inline(always)]
fn fast_hash(key: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in key.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[napi]
pub struct Tachyon {
    inner: Arc<CoreTachyon>,
    callbacks: Arc<CallbackStore>,
}

#[napi]
impl Tachyon {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(CoreTachyon::new()),
            callbacks: Arc::new(CallbackStore::new()),
        }
    }

    /// Register a route with optimized callback handling
    #[napi(
        ts_args_type = "method: string, path: string, callback: (err: Error | null, data: JsCallbackData) => JsCallbackResult"
    )]
    pub fn register_route(&self, method: String, path: String, callback: CallbackFn) -> Result<()> {
        let method_upper = method.to_uppercase();

        // Pre-compute route key hash
        let route_key = format!("{}:{}", method_upper, path);
        let route_hash = fast_hash(&route_key);

        // Store callback with pre-computed hash
        self.callbacks
            .insert(route_hash, CallbackEntry::new(callback));

        // Create optimized handler closure
        let callbacks = Arc::clone(&self.callbacks);
        let handler =
            move |opts: TachyonOptions| invoke_callback_fast(&callbacks, route_hash, &opts);

        // Register with core
        match method_upper.as_str() {
            "GET" => self.inner.get(&path, handler),
            "POST" => self.inner.post(&path, handler),
            "PUT" => self.inner.put(&path, handler),
            "DELETE" => self.inner.delete(&path, handler),
            "PATCH" => self.inner.patch(&path, handler),
            _ => {
                return Err(Error::from_reason(format!(
                    "Unsupported method: {}",
                    method
                )));
            }
        }

        Ok(())
    }

    /// Start the server
    #[napi]
    pub async fn listen(&self, port: u16) -> Result<()> {
        self.inner
            .listen(port)
            .await
            .map_err(|e| Error::from_reason(format!("Server error: {}", e)))
    }

    /// Get callback statistics for profiling
    #[napi]
    pub fn get_stats(&self) -> String {
        let snapshot = self.callbacks.snapshot.load();
        let mut stats: Vec<(u64, u64)> = snapshot
            .iter()
            .map(|(k, v)| (*k, v.call_count.load(Ordering::Relaxed)))
            .collect();
        stats.sort_by(|a, b| b.1.cmp(&a.1));

        format!("{:?}", stats)
    }
}

/// Ultra-fast callback invocation
/// - Lock-free callback lookup
/// - Minimal allocations
/// - Bounded channel for fast sync
/// - Inline everything in hot path
#[inline(always)]
fn invoke_callback_fast(
    callbacks: &CallbackStore,
    route_hash: u64,
    opts: &TachyonOptions,
) -> ResponseData {
    // Lock-free callback lookup - no synchronization
    let entry = match callbacks.get(route_hash) {
        Some(e) => e,
        None => {
            return ResponseData {
                data: ERROR_ROUTE_NOT_FOUND.clone(),
                status_code: 404,
            };
        }
    };

    let tsfn = entry.call();

    // Prepare callback data - use fast serialization
    let js_data = JsCallbackData {
        body: opts.body.as_ref().map(|v| fast_serialize(v)),
        params: opts.params.as_ref().map(|v| fast_serialize(v)),
    };

    // Use bounded(1) channel - zero allocation after first use
    let (tx, rx) = bounded::<JsCallbackResult>(1);

    // Call JS callback with blocking mode for synchronous response
    tsfn.call_with_return_value(
        Ok(js_data),
        ThreadsafeFunctionCallMode::Blocking,
        move |result: std::result::Result<JsCallbackResult, napi::Error<napi::Status>>, _env| {
            if let Ok(res) = result {
                let _ = tx.send(res);
            }
            Ok(())
        },
    );

    // Wait for result - crossbeam recv is extremely fast
    match rx.recv_timeout(std::time::Duration::from_secs(30)) {
        Ok(result) => ResponseData {
            data: Bytes::from(result.data),
            status_code: result.status,
        },
        Err(crossbeam_channel::RecvTimeoutError::Timeout) => ResponseData {
            data: ERROR_TIMEOUT.clone(),
            status_code: 504,
        },
        Err(_) => ResponseData {
            data: ERROR_CALLBACK_FAILED.clone(),
            status_code: 500,
        },
    }
}

/// Fast JSON serialization using simd-json with fallback
#[inline(always)]
fn fast_serialize(v: &serde_json::Value) -> String {
    // Pre-allocate reasonable buffer size based on value type
    let capacity = match v {
        serde_json::Value::Null => 4,
        serde_json::Value::Bool(_) => 5,
        serde_json::Value::Number(_) => 16,
        serde_json::Value::String(s) => s.len() + 2,
        serde_json::Value::Array(a) => a.len() * 16 + 2,
        serde_json::Value::Object(o) => o.len() * 32 + 2,
    };

    let mut buf = Vec::with_capacity(capacity);
    if simd_json::to_writer(&mut buf, v).is_ok() {
        // SAFETY: simd-json produces valid UTF-8
        unsafe { String::from_utf8_unchecked(buf) }
    } else {
        v.to_string()
    }
}

/// Optimized callback for routes that don't need body/params
/// Even faster path for simple GET requests - fully inlined
#[allow(dead_code)]
#[inline(always)]
fn invoke_callback_simple(callbacks: &CallbackStore, route_hash: u64) -> ResponseData {
    let entry = match callbacks.get(route_hash) {
        Some(e) => e,
        None => {
            return ResponseData {
                data: ERROR_ROUTE_NOT_FOUND.clone(),
                status_code: 404,
            };
        }
    };

    let tsfn = entry.call();

    // Empty data - no allocations
    let js_data = JsCallbackData {
        body: None,
        params: None,
    };

    let (tx, rx) = bounded::<JsCallbackResult>(1);

    tsfn.call_with_return_value(
        Ok(js_data),
        ThreadsafeFunctionCallMode::Blocking,
        move |result: std::result::Result<JsCallbackResult, napi::Error<napi::Status>>, _env| {
            if let Ok(res) = result {
                let _ = tx.send(res);
            }
            Ok(())
        },
    );

    match rx.recv_timeout(std::time::Duration::from_secs(30)) {
        Ok(result) => ResponseData {
            data: Bytes::from(result.data),
            status_code: result.status,
        },
        Err(_) => ResponseData {
            data: ERROR_CALLBACK_FAILED.clone(),
            status_code: 500,
        },
    }
}
