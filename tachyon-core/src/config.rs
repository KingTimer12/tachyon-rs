//! Server configuration with sane defaults.
//!
//! Inspired by FaF's approach: few knobs, all performance-relevant.

use std::time::Duration;

/// Socket-level tuning options.
///
/// Applied to the listener socket via `setsockopt`. When the `simd` feature
/// is enabled, these are passed through the cxx bridge to C for clean
/// cross-platform header handling. Without `simd`, a pure-Rust fallback
/// sets the subset available through `std::net`.
#[derive(Debug, Clone)]
pub struct SocketConfig {
    /// Enable SO_REUSEPORT — allows multiple listeners on the same port.
    /// Linux 3.9+, most BSDs. Ignored on Windows.
    pub reuse_port: bool,

    /// Disable Nagle's algorithm (TCP_NODELAY). Almost always wanted for
    /// low-latency servers. Default: true.
    pub tcp_nodelay: bool,

    /// Enable TCP Fast Open (TFO). Saves a round-trip on repeat connections.
    /// Linux 3.7+. Silently ignored on other platforms.
    pub tcp_fastopen: bool,

    /// SO_BUSY_POLL microseconds. The kernel busy-polls the socket for this
    /// long before sleeping. Reduces latency at the cost of CPU.
    /// 0 = disabled (default). Requires root on Linux.
    pub busy_poll_us: i32,

    /// SO_RCVBUF override in bytes. 0 = OS default.
    pub recv_buf_size: i32,

    /// SO_SNDBUF override in bytes. 0 = OS default.
    pub send_buf_size: i32,
}

impl Default for SocketConfig {
    fn default() -> Self {
        Self {
            reuse_port: true,
            tcp_nodelay: true,
            tcp_fastopen: true,
            busy_poll_us: 0,
            recv_buf_size: 0,
            send_buf_size: 0,
        }
    }
}

impl SocketConfig {
    /// Disable all tuning — use OS defaults for everything.
    pub fn none() -> Self {
        Self {
            reuse_port: false,
            tcp_nodelay: false,
            tcp_fastopen: false,
            busy_poll_us: 0,
            recv_buf_size: 0,
            send_buf_size: 0,
        }
    }
}

/// Configuration for a Tachyon server instance.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Address to bind (default: "0.0.0.0:3000")
    pub bind_addr: String,

    /// Number of worker threads. Default: number of CPU cores.
    /// FaF uses 1 thread per core; May does the same internally.
    pub workers: usize,

    /// Coroutine stack size in bytes. May's default is 32KB.
    /// Increase if your handler does deep recursion or large stack allocs.
    pub coroutine_stack_size: usize,

    /// Buffer pool: number of pre-allocated buffers per worker thread.
    /// Higher = more memory upfront, fewer allocation misses under load.
    pub buffers_per_worker: usize,

    /// Buffer pool: size of each buffer in bytes.
    /// Should be >= your largest expected request/response.
    pub buffer_size: usize,

    /// Maximum time a handler can run before being considered stuck.
    /// The safety layer uses this to prevent hung coroutines from
    /// blocking the worker thread forever.
    pub handler_timeout: Duration,

    /// Whether to catch panics in handlers (recommended for library use).
    /// FaF doesn't need this (standalone server), but we do.
    pub catch_panics: bool,

    /// Socket-level tuning options (TCP_NODELAY, SO_REUSEPORT, etc.).
    /// Applied to the listener and per-connection sockets.
    pub socket: SocketConfig,

    /// Security header preset for HTTP responses.
    /// Default: Basic (essential headers with minimal overhead).
    pub security: tachyon_http::response::SecurityPreset,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:3000".to_string(),
            workers: num_cpus(),
            coroutine_stack_size: 64 * 1024,
            buffers_per_worker: 512,
            buffer_size: 8 * 1024,
            handler_timeout: Duration::from_secs(30),
            catch_panics: true,
            socket: SocketConfig::default(),
            security: tachyon_http::response::SecurityPreset::default(),
        }
    }
}

impl ServerConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind(mut self, addr: &str) -> Self {
        self.bind_addr = addr.to_string();
        self
    }

    pub fn workers(mut self, n: usize) -> Self {
        self.workers = n.max(1);
        self
    }

    pub fn stack_size(mut self, bytes: usize) -> Self {
        self.coroutine_stack_size = bytes;
        self
    }

    pub fn buffer_pool(mut self, count: usize, size: usize) -> Self {
        self.buffers_per_worker = count;
        self.buffer_size = size;
        self
    }

    pub fn timeout(mut self, duration: Duration) -> Self {
        self.handler_timeout = duration;
        self
    }

    pub fn catch_panics(mut self, enabled: bool) -> Self {
        self.catch_panics = enabled;
        self
    }

    pub fn socket(mut self, socket: SocketConfig) -> Self {
        self.socket = socket;
        self
    }

    pub fn tcp_nodelay(mut self, enabled: bool) -> Self {
        self.socket.tcp_nodelay = enabled;
        self
    }

    pub fn reuse_port(mut self, enabled: bool) -> Self {
        self.socket.reuse_port = enabled;
        self
    }

    pub fn tcp_fastopen(mut self, enabled: bool) -> Self {
        self.socket.tcp_fastopen = enabled;
        self
    }

    pub fn busy_poll(mut self, microseconds: i32) -> Self {
        self.socket.busy_poll_us = microseconds;
        self
    }

    pub fn recv_buffer(mut self, bytes: i32) -> Self {
        self.socket.recv_buf_size = bytes;
        self
    }

    pub fn send_buffer(mut self, bytes: i32) -> Self {
        self.socket.send_buf_size = bytes;
        self
    }

    pub fn security(mut self, preset: tachyon_http::response::SecurityPreset) -> Self {
        self.security = preset;
        self
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}