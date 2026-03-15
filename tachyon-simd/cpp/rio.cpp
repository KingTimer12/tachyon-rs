// tachyon-simd/cpp/rio.cpp
//
// Windows Registered I/O (RIO) implementation.
//
// RIO is a high-performance I/O API available on Windows 8+ that provides:
// 1. Pre-registered buffers: kernel pins pages once, not per-I/O call
// 2. Batched completions: one call dequeues multiple results
// 3. Reduced syscall overhead: no per-call buffer locking
//
// Architecture:
// - One global RIO function table (loaded once via WSAIoctl)
// - Per-connection RioContext with its own RQ (request queue) + CQ pair
// - Polling-based completion: integrates with may's cooperative scheduler
//   by returning "not ready" so the coroutine can yield and retry
//
// Non-Windows: all functions compile as no-op stubs returning failure.

#include "tachyon-simd/src/lib.rs.h"

#ifdef _WIN32

#include <winsock2.h>
#include <mswsock.h>
#include <ws2tcpip.h>
#pragma comment(lib, "ws2_32.lib")

// RIO completion queue size per connection.
// Each connection has at most 1 outstanding recv + 1 outstanding send,
// so a small CQ suffices. Using 4 for safety margin.
static constexpr DWORD RIO_CQ_SIZE = 4;

// Global RIO function table — loaded once, read-only after init.
static RIO_EXTENSION_FUNCTION_TABLE g_rio = {};
static bool g_rio_initialized = false;

// Per-connection RIO context.
// Owns a request queue and dedicated completion queues.
// Lifetime: created on accept, destroyed on connection close.
struct RioContext {
    RIO_RQ   rq;        // Request queue (1 per socket)
    RIO_CQ   recv_cq;   // Receive completion queue
    RIO_CQ   send_cq;   // Send completion queue
    SOCKET   socket;
    bool     recv_pending;
    bool     send_pending;
};

namespace tachyon {
namespace simd {

bool rio_init() {
    if (g_rio_initialized) return true;

    // Ensure Winsock is initialized
    WSADATA wsa;
    WSAStartup(MAKEWORD(2, 2), &wsa);

    // Create a temporary socket to load the RIO function table
    SOCKET s = WSASocketW(AF_INET, SOCK_STREAM, IPPROTO_TCP,
                          nullptr, 0, WSA_FLAG_REGISTERED_IO);
    if (s == INVALID_SOCKET) return false;

    GUID rio_guid = WSAID_MULTIPLE_RIO;
    DWORD bytes = 0;
    int rc = WSAIoctl(
        s,
        SIO_GET_MULTIPLE_EXTENSION_FUNCTION_POINTER,
        &rio_guid, sizeof(rio_guid),
        &g_rio, sizeof(g_rio),
        &bytes, nullptr, nullptr
    );
    closesocket(s);

    if (rc != 0) return false;

    g_rio_initialized = true;
    return true;
}

bool rio_available() {
    return g_rio_initialized;
}

int64_t rio_register_buffer(rust::Slice<uint8_t> buf) {
    if (!g_rio_initialized) return -1;

    RIO_BUFFERID id = g_rio.RIORegisterBuffer(
        reinterpret_cast<PCHAR>(buf.data()),
        static_cast<DWORD>(buf.size())
    );

    if (id == RIO_INVALID_BUFFERID) return -1;
    return static_cast<int64_t>(reinterpret_cast<intptr_t>(id));
}

void rio_deregister_buffer(int64_t buf_id) {
    if (!g_rio_initialized || buf_id < 0) return;
    g_rio.RIODeregisterBuffer(reinterpret_cast<RIO_BUFFERID>(static_cast<intptr_t>(buf_id)));
}

int64_t rio_create_context(int64_t socket_handle) {
    if (!g_rio_initialized) return -1;

    auto* ctx = new (std::nothrow) RioContext();
    if (!ctx) return -1;

    ctx->socket = static_cast<SOCKET>(socket_handle);
    ctx->recv_pending = false;
    ctx->send_pending = false;

    // Create per-connection completion queues (polling mode, no event notification)
    ctx->recv_cq = g_rio.RIOCreateCompletionQueue(RIO_CQ_SIZE, nullptr);
    if (ctx->recv_cq == RIO_INVALID_CQ) {
        delete ctx;
        return -1;
    }

    ctx->send_cq = g_rio.RIOCreateCompletionQueue(RIO_CQ_SIZE, nullptr);
    if (ctx->send_cq == RIO_INVALID_CQ) {
        g_rio.RIOCloseCompletionQueue(ctx->recv_cq);
        delete ctx;
        return -1;
    }

    // Create request queue: 1 outstanding recv, 1 data buffer per recv,
    // 1 outstanding send, 1 data buffer per send.
    ctx->rq = g_rio.RIOCreateRequestQueue(
        ctx->socket,
        1, 1,  // max outstanding receive, max receive data buffers
        1, 1,  // max outstanding send, max send data buffers
        ctx->recv_cq,
        ctx->send_cq,
        ctx    // socket context pointer (returned in completions)
    );

    if (ctx->rq == RIO_INVALID_RQ) {
        g_rio.RIOCloseCompletionQueue(ctx->send_cq);
        g_rio.RIOCloseCompletionQueue(ctx->recv_cq);
        delete ctx;
        return -1;
    }

    return reinterpret_cast<int64_t>(ctx);
}

void rio_destroy_context(int64_t handle) {
    if (handle <= 0) return;
    auto* ctx = reinterpret_cast<RioContext*>(handle);

    // RQ is implicitly closed when the socket closes.
    // CQs must be explicitly closed.
    g_rio.RIOCloseCompletionQueue(ctx->recv_cq);
    g_rio.RIOCloseCompletionQueue(ctx->send_cq);
    delete ctx;
}

int32_t rio_submit_recv(int64_t handle, int64_t buf_id, uint32_t offset, uint32_t length) {
    if (handle <= 0 || !g_rio_initialized) return -EINVAL;
    auto* ctx = reinterpret_cast<RioContext*>(handle);

    if (ctx->recv_pending) return -EBUSY;

    RIO_BUF rio_buf;
    rio_buf.BufferId = reinterpret_cast<RIO_BUFFERID>(static_cast<intptr_t>(buf_id));
    rio_buf.Offset = offset;
    rio_buf.Length = length;

    if (!g_rio.RIOReceive(ctx->rq, &rio_buf, 1, 0, nullptr)) {
        return -static_cast<int32_t>(WSAGetLastError());
    }

    ctx->recv_pending = true;
    return 0;
}

int32_t rio_submit_send(int64_t handle, int64_t buf_id, uint32_t offset, uint32_t length) {
    if (handle <= 0 || !g_rio_initialized) return -EINVAL;
    auto* ctx = reinterpret_cast<RioContext*>(handle);

    if (ctx->send_pending) return -EBUSY;

    RIO_BUF rio_buf;
    rio_buf.BufferId = reinterpret_cast<RIO_BUFFERID>(static_cast<intptr_t>(buf_id));
    rio_buf.Offset = offset;
    rio_buf.Length = length;

    if (!g_rio.RIOSend(ctx->rq, &rio_buf, 1, 0, nullptr)) {
        return -static_cast<int32_t>(WSAGetLastError());
    }

    ctx->send_pending = true;
    return 0;
}

int32_t rio_poll_recv(int64_t handle) {
    if (handle <= 0) return -EINVAL;
    auto* ctx = reinterpret_cast<RioContext*>(handle);

    if (!ctx->recv_pending) return -EINVAL;

    RIORESULT results[1];
    ULONG count = g_rio.RIODequeueCompletion(ctx->recv_cq, results, 1);

    if (count == 0) return -1;  // Not ready

    if (count == RIO_CORRUPT_CQ) {
        ctx->recv_pending = false;
        return -EFAULT;
    }

    ctx->recv_pending = false;

    if (results[0].Status != 0) {
        return -static_cast<int32_t>(results[0].Status);
    }

    return static_cast<int32_t>(results[0].BytesTransferred);
}

int32_t rio_poll_send(int64_t handle) {
    if (handle <= 0) return -EINVAL;
    auto* ctx = reinterpret_cast<RioContext*>(handle);

    if (!ctx->send_pending) return -EINVAL;

    RIORESULT results[1];
    ULONG count = g_rio.RIODequeueCompletion(ctx->send_cq, results, 1);

    if (count == 0) return -1;  // Not ready

    if (count == RIO_CORRUPT_CQ) {
        ctx->send_pending = false;
        return -EFAULT;
    }

    ctx->send_pending = false;

    if (results[0].Status != 0) {
        return -static_cast<int32_t>(results[0].Status);
    }

    return static_cast<int32_t>(results[0].BytesTransferred);
}

} // namespace simd
} // namespace tachyon

#else
// ============================================================================
// Non-Windows stubs — all functions return failure/no-op
// ============================================================================

namespace tachyon {
namespace simd {

bool rio_init() { return false; }
bool rio_available() { return false; }
int64_t rio_register_buffer(rust::Slice<uint8_t>) { return -1; }
void rio_deregister_buffer(int64_t) {}
int64_t rio_create_context(int64_t) { return -1; }
void rio_destroy_context(int64_t) {}
int32_t rio_submit_recv(int64_t, int64_t, uint32_t, uint32_t) { return -1; }
int32_t rio_submit_send(int64_t, int64_t, uint32_t, uint32_t) { return -1; }
int32_t rio_poll_recv(int64_t) { return -1; }
int32_t rio_poll_send(int64_t) { return -1; }

} // namespace simd
} // namespace tachyon

#endif // _WIN32
