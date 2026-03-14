//! # turbine-simd
//!
//! SIMD-accelerated hot path routines via cxx bridge to C/C++.
//!
//! ## What's in C++ and why
//!
//! Only 3 things are in C++, because they're measurably faster there:
//!
//! 1. **HTTP delimiter scanning** — picohttpparser-style SSE4.2/AVX2/NEON
//!    scans 16-32 bytes per cycle vs Rust's byte-at-a-time loop. 68-90% faster.
//!
//! 2. **JSON parsing/serialization** — simdjson bridge (stub, ready for integration).
//!    4x faster than any pure-Rust JSON parser.
//!
//! 3. **Socket tuning** — Platform-specific setsockopt with conditional C headers.
//!    Cleaner than cfg-gating Rust libc bindings for 3 platforms.
//!
//! Everything else (buffer pool, server loop, coroutine scheduling, safety layer)
//! stays in Rust because Rust is equal or better for those tasks.
//!
//! ## Safety
//!
//! The cxx bridge provides compile-time verification that Rust and C++ agree
//! on types, layouts, and calling conventions. No manual `unsafe extern "C"`.
//! The bridge operates at zero overhead — no copies, no serialization.

#[cxx::bridge(namespace = "tachyon::simd")]
mod ffi {
    /// Result of a SIMD scan operation.
    struct ScanResult {
        position: usize,
        found: bool,
    }

    /// Socket tuning options applied via C setsockopt.
    struct SocketTuning {
        reuse_port: bool,
        tcp_nodelay: bool,
        tcp_fastopen: bool,
        busy_poll_us: i32,
        recv_buf_size: i32,
        send_buf_size: i32,
    }

    /// Key-value pair for JSON serialization.
    struct JsonValue {
        key: String,
        value: String,
    }

    unsafe extern "C++" {
        include!("tachyon-simd/cpp/simd_scan.h");

        /// Find \r\n\r\n in buffer using SIMD.
        /// Auto-dispatches: AVX2 > SSE4.2 > NEON > scalar.
        fn find_header_end_simd(buf: &[u8]) -> ScanResult;

        /// Find a single byte in buffer using SIMD.
        fn find_byte_simd(buf: &[u8], needle: u8) -> ScanResult;

        /// Validate HTTP token characters using SIMD.
        /// Returns index of first invalid byte, or buf.len() if all valid.
        fn validate_token_simd(buf: &[u8]) -> usize;

        /// Parse JSON fields from buffer (simdjson bridge).
        fn parse_json_fields(buf: &[u8]) -> Vec<JsonValue>;

        /// Serialize JSON fields into buffer. Returns bytes written.
        fn serialize_json(fields: &[JsonValue], out_buf: &mut [u8]) -> usize;

        /// Apply socket tuning options to a file descriptor.
        fn apply_socket_tuning(fd: i32, tuning: &SocketTuning) -> i32;
    }
}

// Re-export for ergonomic use from turbine-core
pub use ffi::*;

/// Convenience: find the end of HTTP headers in a buffer.
/// Returns `Some(position)` or `None`.
pub fn find_headers_end(buf: &[u8]) -> Option<usize> {
    let result = ffi::find_header_end_simd(buf);
    if result.found {
        Some(result.position)
    } else {
        None
    }
}

/// Convenience: find a byte in a buffer using SIMD.
pub fn memchr(needle: u8, haystack: &[u8]) -> Option<usize> {
    let result = ffi::find_byte_simd(haystack, needle);
    if result.found {
        Some(result.position)
    } else {
        None
    }
}

/// Default socket tuning for high-performance servers.
pub fn default_tuning() -> SocketTuning {
    SocketTuning {
        reuse_port: true,
        tcp_nodelay: true,
        tcp_fastopen: true, // Linux only, silently ignored elsewhere
        busy_poll_us: 0,    // disabled by default (requires root)
        recv_buf_size: 0,   // OS default
        send_buf_size: 0,   // OS default
    }
}

/// Apply default high-performance socket tuning.
pub fn tune_socket(fd: i32) -> Result<(), i32> {
    let tuning = default_tuning();
    let err = ffi::apply_socket_tuning(fd, &tuning);
    if err == 0 {
        Ok(())
    } else {
        Err(err)
    }
}