use std::{
    cell::RefCell,
    ops::{Deref, DerefMut},
};

/// A pool of reusable byte buffers, local to a single thread.
///
/// Inspired by FaF's pre-allocated response buffers, but generalized:
/// instead of one global buffer, each thread gets a pool that coroutines
/// can borrow from without allocation.
///
/// **Lazy allocation**: the pool starts empty and grows on demand.
/// Buffers returned via `release()` are kept up to `max_capacity`.
/// This means idle servers use near-zero memory, while active servers
/// build up a hot pool of reusable buffers over time.
pub struct BufferPool {
    buf_size: usize,
    max_capacity: usize,
    buffers: RefCell<Vec<Vec<u8>>>,
    #[cfg(debug_assertions)]
    stats: RefCell<PoolStats>,
}

#[cfg(debug_assertions)]
#[derive(Default)]
struct PoolStats {
    acquires: u64,
    releases: u64,
    alloc_misses: u64,
}

/// RAII guard that returns the buffer to the pool on drop.
/// While held, it derefs to `&mut [u8]` for zero-copy I/O.
pub struct BufGuard {
    buf: Option<Vec<u8>>,
    len: usize,
}

impl BufferPool {
    /// Create a new **lazy** pool. No buffers are allocated until first `acquire()`.
    /// Buffers returned via `release()` are kept up to `max_capacity`.
    pub fn new(max_capacity: usize, buf_size: usize) -> Self {
        Self {
            buf_size,
            max_capacity,
            buffers: RefCell::new(Vec::with_capacity(max_capacity)),
            #[cfg(debug_assertions)]
            stats: RefCell::new(PoolStats::default()),
        }
    }

    /// Acquire a buffer from the pool. If the pool is empty, allocates a new one
    /// (this is the "miss" path — ideally rare once the pool is warmed up).
    pub fn acquire(&self) -> BufGuard {
        #[cfg(debug_assertions)]
        {
            self.stats.borrow_mut().acquires += 1;
        }

        let buf = self.buffers.borrow_mut().pop().unwrap_or_else(|| {
            #[cfg(debug_assertions)]
            {
                self.stats.borrow_mut().alloc_misses += 1;
            }
            vec![0u8; self.buf_size]
        });

        BufGuard {
            buf: Some(buf),
            len: 0,
        }
    }

    /// Return a buffer to the pool. Called automatically by `BufGuard::drop`.
    /// If the pool is at max capacity, the buffer is dropped (deallocated).
    pub fn release(&self, mut buf: Vec<u8>) {
        #[cfg(debug_assertions)]
        {
            self.stats.borrow_mut().releases += 1;
        }

        let mut buffers = self.buffers.borrow_mut();
        if buffers.len() < self.max_capacity {
            // Reset length but keep capacity — the allocation is reused.
            buf.clear();
            buffers.push(buf);
        }
        // else: pool is full, drop the buffer (Vec deallocates)
    }

    pub fn get_buffers(&self) -> Vec<Vec<u8>> {
        self.buffers.borrow().clone()
    }
}

impl BufGuard {
    /// The writable slice of the buffer (up to full capacity).
    /// After a pool round-trip (release → acquire), the Vec's len may be 0
    /// due to clear(). We restore it to capacity so the full buffer is writable.
    pub fn as_write_buf(&mut self) -> &mut [u8] {
        let buf = self.buf.as_mut().unwrap();
        let cap = buf.capacity();
        if buf.len() < cap {
            buf.resize(cap, 0);
        }
        buf.as_mut_slice()
    }

    /// Mark `n` bytes as written. Used after a raw read syscall.
    pub fn set_len(&mut self, n: usize) {
        self.len = n;
    }

    /// The filled portion of the buffer.
    pub fn filled(&self) -> &[u8] {
        &self.buf.as_ref().unwrap()[..self.len]
    }

    /// Capacity of the underlying buffer.
    pub fn capacity(&self) -> usize {
        self.buf.as_ref().unwrap().len()
    }

    /// Take ownership of the inner Vec, detaching from the pool.
    /// Use sparingly — this defeats the zero-alloc purpose.
    pub fn take(mut self) -> Vec<u8> {
        let mut buf = self.buf.take().unwrap();
        buf.truncate(self.len);
        buf
    }

    pub fn get_buf(&mut self) -> Vec<u8> {
        self.buf.take().unwrap()
    }
}

impl Deref for BufGuard {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.filled()
    }
}

impl DerefMut for BufGuard {
    fn deref_mut(&mut self) -> &mut [u8] {
        let len = self.len;
        &mut self.buf.as_mut().unwrap()[..len]
    }
}
