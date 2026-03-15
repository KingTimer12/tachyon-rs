// tachyon-simd/cpp/rio.h
//
// Windows Registered I/O (RIO) — zero-copy, kernel-bypass networking.
//
// RIO eliminates per-I/O buffer locking: you register memory once, then
// the kernel uses the pre-pinned pages for every recv/send. Combined with
// batched completions, this gives ~2x throughput over standard Winsock/IOCP.
//
// On non-Windows platforms, all functions are safe no-op stubs.

#pragma once
#include "rust/cxx.h"
#include <cstdint>

namespace tachyon {
namespace simd {

// Initialize the RIO function table. Call once at server startup.
// Returns true if RIO is available and initialized.
bool rio_init();

// Check if RIO was successfully initialized.
bool rio_available();

// Register a buffer region for zero-copy I/O.
// The memory must remain valid and at the same address until deregistered.
// Returns buffer ID (>= 0) on success, -1 on failure.
int64_t rio_register_buffer(rust::Slice<uint8_t> buf);

// Deregister a previously registered buffer.
void rio_deregister_buffer(int64_t buf_id);

// Create a RIO context for a socket (allocates RQ + CQ pair).
// Returns context handle (> 0) on success, -1 on failure.
int64_t rio_create_context(int64_t socket_handle);

// Destroy a RIO context and free associated resources.
void rio_destroy_context(int64_t ctx);

// Submit a receive request (non-blocking).
// Returns 0 on success, negative error code on failure.
int32_t rio_submit_recv(int64_t ctx, int64_t buf_id, uint32_t offset, uint32_t length);

// Submit a send request (non-blocking).
// Returns 0 on success, negative error code on failure.
int32_t rio_submit_send(int64_t ctx, int64_t buf_id, uint32_t offset, uint32_t length);

// Poll for receive completion (non-blocking).
// Returns: >= 0 = bytes received (0 = EOF), -1 = not ready, < -1 = error.
int32_t rio_poll_recv(int64_t ctx);

// Poll for send completion (non-blocking).
// Returns: >= 0 = bytes sent, -1 = not ready, < -1 = error.
int32_t rio_poll_send(int64_t ctx);

} // namespace simd
} // namespace tachyon
