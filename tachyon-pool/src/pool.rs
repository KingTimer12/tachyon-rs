use crate::buffer::{BufGuard, BufferPool};

/// Default buffer size: 8KB covers most HTTP request/response bodies.
/// Increase for large payload workloads. FaF uses a compile-time constant
/// (`RES_BUFF_SIZE`) for the same purpose.
pub const DEFAULT_BUF_SIZE: usize = 8 * 1024;

/// Default max number of pooled buffers per thread.
/// The pool starts empty (lazy) and grows on demand up to this cap.
/// 32 buffers × 8KB = 256KB per thread — enough for typical concurrent connections.
pub const DEFAULT_POOL_CAPACITY: usize = 32;

// Thread-local pool access
thread_local! {
    static THREAD_POOL: BufferPool = BufferPool::new(DEFAULT_POOL_CAPACITY, DEFAULT_BUF_SIZE);
}

/// Acquire a buffer from the current thread's pool.
pub fn acquire() -> BufGuard {
    THREAD_POOL.with(|pool| pool.acquire())
}

/// Initialize the thread-local pool with custom parameters.
/// Call this at the start of each worker thread if you need non-default sizing.
pub fn init_thread_pool(capacity: usize, buf_size: usize) {
    THREAD_POOL.with(|pool| pool.reinit(capacity, buf_size));
}

/// Return a buffer to the current thread's pool.
/// Usually you don't call this directly — `BufGuard` does it on drop.
pub fn release(guard: BufGuard) {
    drop(guard);
}

impl Drop for BufGuard {
    fn drop(&mut self) {
        let buf = self.get_buf();
        // Try to return to thread-local pool.
        // If we're on a different thread (shouldn't happen with May's
        // coroutine model, but defensive), just let it deallocate.
        THREAD_POOL.with(|pool| pool.release(buf));
    }
}
