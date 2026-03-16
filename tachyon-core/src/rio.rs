//! Windows Registered I/O (RIO) integration.
//!
//! Provides zero-copy recv/send that integrates with May's cooperative scheduler
//! by polling RIO completions and yielding between polls.
//!
//! # Usage
//!
//! RIO is initialized once at server startup. Each connection creates a `RioConn`
//! that wraps a socket handle and pre-registered buffers. The `rio_recv` and
//! `rio_send` functions cooperatively block by polling + yielding, compatible
//! with May's coroutine model.

use std::io;

/// Initialize the RIO subsystem. Call once before creating any connections.
/// Returns true if RIO is available on this system (Windows 8+).
pub fn init() -> bool {
    #[cfg(all(feature = "simd", windows))]
    {
        tachyon_simd::rio_init()
    }
    #[cfg(not(all(feature = "simd", windows)))]
    {
        false
    }
}

/// Check if RIO is available and initialized.
pub fn available() -> bool {
    #[cfg(all(feature = "simd", windows))]
    {
        tachyon_simd::rio_available()
    }
    #[cfg(not(all(feature = "simd", windows)))]
    {
        false
    }
}

/// Register a buffer for zero-copy RIO I/O.
/// Returns a buffer ID that can be used with `RioConn::recv` and `RioConn::send`.
/// The buffer must remain valid and at the same address until `deregister_buffer`.
pub fn register_buffer(buf: &mut [u8]) -> Option<i64> {
    #[cfg(all(feature = "simd", windows))]
    {
        let id = tachyon_simd::rio_register_buffer(buf);
        if id >= 0 { Some(id) } else { None }
    }
    #[cfg(not(all(feature = "simd", windows)))]
    {
        let _ = buf;
        None
    }
}

/// Deregister a previously registered buffer.
pub fn deregister_buffer(buf_id: i64) {
    #[cfg(all(feature = "simd", windows))]
    {
        tachyon_simd::rio_deregister_buffer(buf_id);
    }
    #[cfg(not(all(feature = "simd", windows)))]
    {
        let _ = buf_id;
    }
}

/// A RIO-backed connection context.
///
/// Wraps a socket handle with RIO request/completion queues for zero-copy I/O.
/// Each connection in the server loop gets one of these.
pub struct RioConn {
    #[allow(dead_code)] // used only on Windows via cfg-gated methods and Drop
    ctx: i64,
}

impl RioConn {
    /// Create a RIO context for a socket.
    /// The socket must have been created with `WSA_FLAG_REGISTERED_IO` for best
    /// performance, but RIO also works with regular sockets.
    #[cfg(all(feature = "simd", windows))]
    pub fn new(socket_handle: i64) -> Option<Self> {
        let ctx = tachyon_simd::rio_create_context(socket_handle);
        if ctx > 0 { Some(Self { ctx }) } else { None }
    }

    #[cfg(not(all(feature = "simd", windows)))]
    pub fn new(_socket_handle: i64) -> Option<Self> {
        None
    }

    /// Cooperatively blocking receive using RIO.
    ///
    /// Submits a RIO receive request on the pre-registered buffer, then polls
    /// for completion. Between polls, yields the current May coroutine so other
    /// coroutines can run on this thread.
    ///
    /// Returns the number of bytes received, or 0 for EOF.
    pub fn recv(&self, buf_id: i64, offset: u32, length: u32) -> io::Result<usize> {
        #[cfg(all(feature = "simd", windows))]
        {
            let rc = tachyon_simd::rio_submit_recv(self.ctx, buf_id, offset, length);
            if rc < 0 {
                return Err(io::Error::from_raw_os_error(-rc));
            }

            loop {
                let result = tachyon_simd::rio_poll_recv(self.ctx);
                if result >= 0 {
                    return Ok(result as usize);
                }
                if result == -1 {
                    // Not ready — yield to let other coroutines run
                    std::thread::yield_now();
                    continue;
                }
                // Error
                return Err(io::Error::from_raw_os_error(-result));
            }
        }
        #[cfg(not(all(feature = "simd", windows)))]
        {
            let _ = (buf_id, offset, length);
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "RIO not available",
            ))
        }
    }

    /// Cooperatively blocking send using RIO.
    ///
    /// Same pattern as recv: submit → poll with yield → return.
    pub fn send(&self, buf_id: i64, offset: u32, length: u32) -> io::Result<usize> {
        #[cfg(all(feature = "simd", windows))]
        {
            let rc = tachyon_simd::rio_submit_send(self.ctx, buf_id, offset, length);
            if rc < 0 {
                return Err(io::Error::from_raw_os_error(-rc));
            }

            loop {
                let result = tachyon_simd::rio_poll_send(self.ctx);
                if result >= 0 {
                    return Ok(result as usize);
                }
                if result == -1 {
                    std::thread::yield_now();
                    continue;
                }
                return Err(io::Error::from_raw_os_error(-result));
            }
        }
        #[cfg(not(all(feature = "simd", windows)))]
        {
            let _ = (buf_id, offset, length);
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "RIO not available",
            ))
        }
    }
}

impl Drop for RioConn {
    fn drop(&mut self) {
        #[cfg(all(feature = "simd", windows))]
        {
            tachyon_simd::rio_destroy_context(self.ctx);
        }
    }
}
