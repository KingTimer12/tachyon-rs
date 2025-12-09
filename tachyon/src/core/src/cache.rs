//! Ultra-fast cache with lock-free reads and sharded writes
//!
//! Optimizations:
//! - Lock-free reads using ArcSwap for hot paths
//! - Sharded storage to reduce contention
//! - FNV-1a hashing for speed
//! - Pre-allocated slots to avoid runtime allocations

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use arc_swap::ArcSwapOption;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use crate::router::TachyonHandler;

/// Number of shards - power of 2 for fast modulo
const NUM_SHARDS: usize = 32;
const SHARD_MASK: usize = NUM_SHARDS - 1;

/// Maximum entries per shard before eviction
const MAX_ENTRIES_PER_SHARD: usize = 128;

/// Cache entry with handler and access count
struct CacheEntry {
    handler: TachyonHandler,
    access_count: AtomicU64,
}

impl CacheEntry {
    #[inline(always)]
    fn new(handler: TachyonHandler) -> Self {
        Self {
            handler,
            access_count: AtomicU64::new(1),
        }
    }

    #[inline(always)]
    fn touch(&self) {
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }
}

/// A single cache shard with its own lock
struct CacheShard {
    /// Map from route hash to entry
    entries: RwLock<FxHashMap<u64, Arc<CacheEntry>>>,
    /// Fast-path single slot for most recent entry (lock-free)
    hot_slot: ArcSwapOption<(u64, CacheEntry)>,
}

impl CacheShard {
    fn new() -> Self {
        Self {
            entries: RwLock::new(FxHashMap::default()),
            hot_slot: ArcSwapOption::empty(),
        }
    }

    /// Try to get from hot slot first (lock-free), then from map
    #[inline]
    fn get(&self, hash: u64) -> Option<TachyonHandler> {
        // Fast path: check hot slot (completely lock-free)
        if let Some(slot) = self.hot_slot.load().as_ref() {
            if slot.0 == hash {
                slot.1.touch();
                return Some(slot.1.handler.clone());
            }
        }

        // Slow path: check map with read lock
        let entries = self.entries.read();
        if let Some(entry) = entries.get(&hash) {
            entry.touch();
            Some(entry.handler.clone())
        } else {
            None
        }
    }

    /// Insert entry and update hot slot
    #[inline]
    fn insert(&self, hash: u64, handler: TachyonHandler) {
        let entry = Arc::new(CacheEntry::new(handler.clone()));

        // Update hot slot (lock-free)
        self.hot_slot
            .store(Some(Arc::new((hash, CacheEntry::new(handler)))));

        // Update map with write lock
        let mut entries = self.entries.write();

        // Evict if too many entries (simple LFU eviction)
        if entries.len() >= MAX_ENTRIES_PER_SHARD {
            self.evict_lfu(&mut entries);
        }

        entries.insert(hash, entry);
    }

    /// Evict least frequently used entries
    fn evict_lfu(&self, entries: &mut FxHashMap<u64, Arc<CacheEntry>>) {
        // Find entry with lowest access count
        if let Some((&key_to_remove, _)) = entries
            .iter()
            .min_by_key(|(_, entry)| entry.access_count.load(Ordering::Relaxed))
        {
            entries.remove(&key_to_remove);
        }
    }
}

/// High-performance sharded cache
pub struct HotCache {
    shards: [CacheShard; NUM_SHARDS],
    /// Global hit counter for stats
    hits: AtomicU64,
    /// Global miss counter for stats
    misses: AtomicU64,
}

impl HotCache {
    pub fn new() -> Self {
        Self {
            shards: std::array::from_fn(|_| CacheShard::new()),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Ultra-fast hash function (FNV-1a)
    #[inline(always)]
    fn hash(key: &str) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in key.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    /// Get shard index from hash
    #[inline(always)]
    fn shard_index(hash: u64) -> usize {
        (hash as usize) & SHARD_MASK
    }

    /// Get handler from cache
    #[inline]
    pub fn get(&self, route_key: &str) -> Option<TachyonHandler> {
        let hash = Self::hash(route_key);
        let shard_idx = Self::shard_index(hash);

        let result = self.shards[shard_idx].get(hash);

        if result.is_some() {
            self.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }

        result
    }

    /// Store handler in cache
    #[inline]
    pub fn set(&self, route_key: String, handler: TachyonHandler) {
        let hash = Self::hash(&route_key);
        let shard_idx = Self::shard_index(hash);

        self.shards[shard_idx].insert(hash, handler);
    }

    /// Get or insert with factory function
    #[inline]
    pub fn get_or_insert<F>(&self, route_key: &str, f: F) -> Option<TachyonHandler>
    where
        F: FnOnce() -> Option<TachyonHandler>,
    {
        // Try cache first
        if let Some(handler) = self.get(route_key) {
            return Some(handler);
        }

        // Compute new value
        let handler = f()?;

        // Store in cache
        self.set(route_key.to_owned(), handler.clone());

        Some(handler)
    }

    /// Clear all cache entries
    #[allow(dead_code)]
    pub fn clear(&self) {
        for shard in &self.shards {
            shard.hot_slot.store(None);
            let mut entries = shard.entries.write();
            entries.clear();
        }
    }

    /// Get cache statistics
    #[allow(dead_code)]
    pub fn stats(&self) -> (u64, u64) {
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
        )
    }

    /// Get hit rate
    #[allow(dead_code)]
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed) as f64;
        let misses = self.misses.load(Ordering::Relaxed) as f64;
        let total = hits + misses;
        if total == 0.0 {
            0.0
        } else {
            hits / total
        }
    }
}

impl Default for HotCache {
    fn default() -> Self {
        Self::new()
    }
}

// Safety: All fields are thread-safe
unsafe impl Send for HotCache {}
unsafe impl Sync for HotCache {}
