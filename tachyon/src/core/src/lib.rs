//! Tachyon Core - Ultra-fast HTTP server engine
//!
//! Optimizations:
//! - mimalloc global allocator
//! - Lock-free data structures where possible
//! - Thread-local caching
//! - SIMD JSON parsing
//! - Zero-allocation hot paths

mod cache;
mod http_call;
mod methods;
pub mod options;
mod router;
mod tachyon;
mod utils;

pub use methods::Method;
pub use router::{TachyonHandler, TachyonRouter};
pub use tachyon::Tachyon;
