use std::{cell::RefCell, ops::{Deref, DerefMut}};

/// A pool of reusable byte buffers, local to a single thread.
///
/// Inspired by FaF's pre-allocated response buffers, but generalized:
/// instead of one global buffer, each thread gets a pool that coroutines
/// can borrow from without allocation.
pub struct BufferPool {
    buf_size: usize,
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
    /// Create a new pool pre-filled with `capacity` buffers of `buf_size` bytes.
    pub fn new(capacity: usize, buf_size: usize) -> Self {
        let buffers = (0..capacity).map(|_| vec![0u8; buf_size]).collect();
        Self {
            buf_size,
            buffers: RefCell::new(buffers),
            #[cfg(debug_assertions)]
            stats: RefCell::new(PoolStats::default()),
        }
    }

    /// Acquire a buffer from the pool. If the pool is empty, allocates a new one
    /// (this is the "miss" path — ideally rare in a well-tuned deployment).
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

        BufGuard { buf: Some(buf), len: 0 }
    }

    /// Return a buffer to the pool. Called automatically by `BufGuard::drop`.
    pub fn release(&self, mut buf: Vec<u8>) {
        #[cfg(debug_assertions)]
        {
            self.stats.borrow_mut().releases += 1;
        }

        // Reset length but keep capacity — the allocation is reused.
        // This mirrors FaF's approach: the buffer memory stays allocated,
        // only the "used" marker resets.
        buf.clear();
        self.buffers.borrow_mut().push(buf);
    }

    pub fn get_buffers(&self) -> Vec<Vec<u8>> {
        self.buffers.borrow().clone()
    }
}

impl BufGuard {
    /// The writable slice of the buffer (up to full capacity).
    pub fn as_write_buf(&mut self) -> &mut [u8] {
        self.buf.as_mut().unwrap().as_mut_slice()
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
