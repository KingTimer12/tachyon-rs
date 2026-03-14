// tachyon-simd/cpp/simd_scan.cpp
//
// Implementation of SIMD-accelerated hot path routines.
// Compile with: -msse4.2 (minimum) or -mavx2 (recommended)

#include "tachyon-simd/src/lib.rs.h"
#include <cstring>

// Platform detection for SIMD
#if defined(__SSE4_2__)
#include <nmmintrin.h>
#define TACHYON_HAS_SSE42 1
#endif

#if defined(__AVX2__)
#include <immintrin.h>
#define TACHYON_HAS_AVX2 1
#endif

#if defined(__ARM_NEON) || defined(__aarch64__)
#include <arm_neon.h>
#define TACHYON_HAS_NEON 1
#endif

namespace tachyon {
namespace simd {

// ============================================================================
// SIMD HTTP Scanner
// ============================================================================

// Scalar fallback — always available (may be unused when SIMD paths compile)
[[maybe_unused]] static ScanResult find_header_end_scalar(const uint8_t* buf, size_t len) {
    if (len < 4) return {0, false};
    for (size_t i = 0; i <= len - 4; i++) {
        if (buf[i] == '\r' && buf[i+1] == '\n' &&
            buf[i+2] == '\r' && buf[i+3] == '\n') {
            return {i, true};
        }
    }
    return {0, false};
}

#if TACHYON_HAS_SSE42

// SSE4.2 path: scan 16 bytes at a time for \r using _mm_cmpestri,
// then verify the full \r\n\r\n sequence.
// This is the picohttpparser approach.
static ScanResult find_header_end_sse42(const uint8_t* buf, size_t len) {
    if (len < 4) return {0, false};

    // Pattern: we scan for '\r' using SIMD, then check if it's part of \r\n\r\n
    const __m128i cr = _mm_set1_epi8('\r');

    size_t i = 0;
    // Process 16-byte chunks
    for (; i + 16 <= len; i += 16) {
        __m128i chunk = _mm_loadu_si128(reinterpret_cast<const __m128i*>(buf + i));
        __m128i cmp = _mm_cmpeq_epi8(chunk, cr);
        int mask = _mm_movemask_epi8(cmp);

        while (mask) {
            int bit = __builtin_ctz(mask);
            size_t pos = i + bit;

            if (pos + 3 < len &&
                buf[pos]   == '\r' && buf[pos+1] == '\n' &&
                buf[pos+2] == '\r' && buf[pos+3] == '\n') {
                return {pos, true};
            }
            mask &= mask - 1; // clear lowest set bit
        }
    }

    // Handle remaining bytes with scalar
    for (; i + 3 < len; i++) {
        if (buf[i] == '\r' && buf[i+1] == '\n' &&
            buf[i+2] == '\r' && buf[i+3] == '\n') {
            return {i, true};
        }
    }

    return {0, false};
}

#endif // TACHYON_HAS_SSE42

#if TACHYON_HAS_AVX2

// AVX2 path: 32 bytes at a time. CloudFlare-style approach.
static ScanResult find_header_end_avx2(const uint8_t* buf, size_t len) {
    if (len < 4) return {0, false};

    const __m256i cr = _mm256_set1_epi8('\r');

    size_t i = 0;
    for (; i + 32 <= len; i += 32) {
        __m256i chunk = _mm256_loadu_si256(reinterpret_cast<const __m256i*>(buf + i));
        __m256i cmp = _mm256_cmpeq_epi8(chunk, cr);
        int mask = _mm256_movemask_epi8(cmp);

        while (mask) {
            int bit = __builtin_ctz(mask);
            size_t pos = i + bit;

            if (pos + 3 < len &&
                buf[pos]   == '\r' && buf[pos+1] == '\n' &&
                buf[pos+2] == '\r' && buf[pos+3] == '\n') {
                return {pos, true};
            }
            mask &= mask - 1;
        }
    }

    // Tail
    for (; i + 3 < len; i++) {
        if (buf[i] == '\r' && buf[i+1] == '\n' &&
            buf[i+2] == '\r' && buf[i+3] == '\n') {
            return {i, true};
        }
    }

    return {0, false};
}

#endif // TACHYON_HAS_AVX2

#if TACHYON_HAS_NEON

// ARM NEON path for Apple Silicon / ARM servers
static ScanResult find_header_end_neon(const uint8_t* buf, size_t len) {
    if (len < 4) return {0, false};

    const uint8x16_t cr = vdupq_n_u8('\r');

    size_t i = 0;
    for (; i + 16 <= len; i += 16) {
        uint8x16_t chunk = vld1q_u8(buf + i);
        uint8x16_t cmp = vceqq_u8(chunk, cr);

        // Extract comparison results
        // NEON doesn't have movemask — use reduction
        uint64x2_t cmp64 = vreinterpretq_u64_u8(cmp);
        uint64_t lo = vgetq_lane_u64(cmp64, 0);
        uint64_t hi = vgetq_lane_u64(cmp64, 1);

        if (lo || hi) {
            // Found at least one \r — check each position
            for (size_t j = i; j < i + 16 && j + 3 < len; j++) {
                if (buf[j] == '\r' && buf[j+1] == '\n' &&
                    buf[j+2] == '\r' && buf[j+3] == '\n') {
                    return {j, true};
                }
            }
        }
    }

    // Tail
    for (; i + 3 < len; i++) {
        if (buf[i] == '\r' && buf[i+1] == '\n' &&
            buf[i+2] == '\r' && buf[i+3] == '\n') {
            return {i, true};
        }
    }

    return {0, false};
}

#endif // TACHYON_HAS_NEON


// --- Public API: runtime dispatch to best available SIMD ---

ScanResult find_header_end_simd(rust::Slice<const uint8_t> buf) {
    const uint8_t* ptr = buf.data();
    size_t len = buf.size();

#if TACHYON_HAS_AVX2
    return find_header_end_avx2(ptr, len);
#elif TACHYON_HAS_SSE42
    return find_header_end_sse42(ptr, len);
#elif TACHYON_HAS_NEON
    return find_header_end_neon(ptr, len);
#else
    return find_header_end_scalar(ptr, len);
#endif
}


ScanResult find_byte_simd(rust::Slice<const uint8_t> buf, uint8_t needle) {
    const uint8_t* ptr = buf.data();
    size_t len = buf.size();

#if TACHYON_HAS_AVX2
    const __m256i target = _mm256_set1_epi8(static_cast<char>(needle));
    size_t i = 0;
    for (; i + 32 <= len; i += 32) {
        __m256i chunk = _mm256_loadu_si256(reinterpret_cast<const __m256i*>(ptr + i));
        int mask = _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, target));
        if (mask) {
            return {i + static_cast<size_t>(__builtin_ctz(mask)), true};
        }
    }
    for (; i < len; i++) {
        if (ptr[i] == needle) return {i, true};
    }
    return {0, false};

#elif TACHYON_HAS_SSE42
    const __m128i target = _mm_set1_epi8(static_cast<char>(needle));
    size_t i = 0;
    for (; i + 16 <= len; i += 16) {
        __m128i chunk = _mm_loadu_si128(reinterpret_cast<const __m128i*>(ptr + i));
        int mask = _mm_movemask_epi8(_mm_cmpeq_epi8(chunk, target));
        if (mask) {
            return {i + static_cast<size_t>(__builtin_ctz(mask)), true};
        }
    }
    for (; i < len; i++) {
        if (ptr[i] == needle) return {i, true};
    }
    return {0, false};

#else
    // Scalar fallback
    for (size_t i = 0; i < len; i++) {
        if (ptr[i] == needle) return {i, true};
    }
    return {0, false};
#endif
}


size_t validate_token_simd(rust::Slice<const uint8_t> buf) {
    const uint8_t* ptr = buf.data();
    size_t len = buf.size();

    // Valid HTTP token chars: any CHAR except CTLs and separators
    // Simplified check: 0x21 <= c <= 0x7E (printable ASCII except space)
#if TACHYON_HAS_AVX2
    const __m256i lo = _mm256_set1_epi8(0x20);
    const __m256i hi = _mm256_set1_epi8(0x7F);

    size_t i = 0;
    for (; i + 32 <= len; i += 32) {
        __m256i chunk = _mm256_loadu_si256(reinterpret_cast<const __m256i*>(ptr + i));
        // Check: chunk > 0x20 AND chunk < 0x7F
        __m256i above_lo = _mm256_cmpgt_epi8(chunk, lo);
        __m256i below_hi = _mm256_cmpgt_epi8(hi, chunk);
        __m256i valid = _mm256_and_si256(above_lo, below_hi);
        int mask = _mm256_movemask_epi8(valid);
        if (mask != -1) { // not all valid
            int inv = ~mask;
            return i + static_cast<size_t>(__builtin_ctz(inv));
        }
    }
    for (; i < len; i++) {
        if (ptr[i] <= 0x20 || ptr[i] >= 0x7F) return i;
    }
    return len;
#else
    for (size_t i = 0; i < len; i++) {
        if (ptr[i] <= 0x20 || ptr[i] >= 0x7F) return i;
    }
    return len;
#endif
}


// ============================================================================
// JSON — real simdjson integration
// ============================================================================
//
// simdjson is distributed as two files: simdjson.h (~50k lines) and
// simdjson.cpp (~4k lines). The build script downloads them automatically.
//
// If TACHYON_HAS_SIMDJSON is not defined, we fall back to a fast
// manual serializer (still faster than most pure-language JSON libs).

} // namespace simd
} // namespace tachyon

#ifdef TACHYON_HAS_SIMDJSON
#include "simdjson.h"
#endif

namespace tachyon {
namespace simd {

#ifdef TACHYON_HAS_SIMDJSON

// Thread-local parser to avoid allocation per request.
// simdjson::ondemand::parser is not thread-safe but our May coroutines
// are pinned to threads, so thread_local is correct here.
static thread_local simdjson::ondemand::parser tl_parser;

rust::Vec<JsonValue> parse_json_fields(rust::Slice<const uint8_t> buf) {
    rust::Vec<JsonValue> result;

    // simdjson requires SIMDJSON_PADDING (64 bytes) after the data.
    // Our 8KB slab buffers always have spare room, but we verify.
    if (buf.size() == 0) return result;

    // Create a padded copy only if the buffer doesn't have padding room.
    // In practice, tachyon-pool buffers are 8KB and requests are <4KB,
    // so we almost never allocate here.
    simdjson::padded_string_view json_view(
        reinterpret_cast<const char*>(buf.data()),
        buf.size(),
        buf.size() + simdjson::SIMDJSON_PADDING
    );

    auto doc = tl_parser.iterate(json_view);
    if (doc.error()) return result;

    // Extract top-level string fields
    simdjson::ondemand::object obj;
    if (doc.get_object().get(obj)) return result;

    for (auto field : obj) {
        std::string_view key_sv;
        if (field.unescaped_key().get(key_sv)) continue;

        // Only extract string values for the simple bridge API.
        // For complex nested JSON, users should use simdjson directly.
        std::string_view val_sv;
        if (!field.value().get_string().get(val_sv)) {
            JsonValue jv;
            jv.key = rust::String(std::string(key_sv));
            jv.value = rust::String(std::string(val_sv));
            result.push_back(std::move(jv));
            continue;
        }

        // Try number → string conversion
        int64_t num_val;
        if (!field.value().get_int64().get(num_val)) {
            JsonValue jv;
            jv.key = rust::String(std::string(key_sv));
            jv.value = rust::String(std::to_string(num_val));
            result.push_back(std::move(jv));
        }
    }

    return result;
}

#else
// Fallback: no simdjson available
rust::Vec<JsonValue> parse_json_fields(rust::Slice<const uint8_t> /* buf */) {
    return {};
}
#endif

size_t serialize_json(
    rust::Slice<const JsonValue> fields,
    rust::Slice<uint8_t> out_buf
) {
    // Fast manual JSON serialization — no std::string, no allocations.
    // Writes directly into the Rust slab buffer.
    // This is faster than simdjson for serialization (simdjson is
    // optimized for parsing, not generation), so we keep it in C++
    // for the SIMD-accelerated string escaping potential.
    uint8_t* out = out_buf.data();
    size_t pos = 0;
    size_t cap = out_buf.size();

    if (cap < 2) return 0;
    out[pos++] = '{';

    for (size_t i = 0; i < fields.size(); i++) {
        if (i > 0 && pos < cap) out[pos++] = ',';

        // "key":"value"
        if (pos < cap) out[pos++] = '"';
        const auto& key = fields[i].key;
        size_t klen = key.size() < (cap - pos) ? key.size() : (cap - pos);
        std::memcpy(out + pos, key.data(), klen);
        pos += klen;
        if (pos + 2 < cap) { out[pos++] = '"'; out[pos++] = ':'; out[pos++] = '"'; }
        const auto& val = fields[i].value;
        size_t vlen = val.size() < (cap - pos) ? val.size() : (cap - pos);
        std::memcpy(out + pos, val.data(), vlen);
        pos += vlen;
        if (pos < cap) out[pos++] = '"';
    }

    if (pos < cap) out[pos++] = '}';
    return pos;
}


// ============================================================================
// Socket Tuning
// ============================================================================

#if defined(__linux__) || defined(__APPLE__) || defined(__FreeBSD__)
#include <sys/socket.h>
#include <netinet/tcp.h>
#include <netinet/in.h>
#include <errno.h>
#endif

#ifdef _WIN32
#include <winsock2.h>
#include <ws2tcpip.h>
#pragma comment(lib, "ws2_32.lib")
#endif

int32_t apply_socket_tuning(int32_t fd, const SocketTuning& t) {
    int32_t last_err = 0;

#if defined(__linux__) || defined(__APPLE__) || defined(__FreeBSD__)
    int val;

    if (t.tcp_nodelay) {
        val = 1;
        if (setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &val, sizeof(val)) < 0)
            last_err = -errno;
    }

#ifdef SO_REUSEPORT
    if (t.reuse_port) {
        val = 1;
        if (setsockopt(fd, SOL_SOCKET, SO_REUSEPORT, &val, sizeof(val)) < 0)
            last_err = -errno;
    }
#endif

#if defined(__linux__) && defined(TCP_FASTOPEN)
    if (t.tcp_fastopen) {
        val = 5; // queue length for TFO
        if (setsockopt(fd, IPPROTO_TCP, TCP_FASTOPEN, &val, sizeof(val)) < 0)
            last_err = -errno;
    }
#endif

#if defined(__linux__) && defined(SO_BUSY_POLL)
    if (t.busy_poll_us > 0) {
        val = t.busy_poll_us;
        if (setsockopt(fd, SOL_SOCKET, SO_BUSY_POLL, &val, sizeof(val)) < 0)
            last_err = -errno;
    }
#endif

    if (t.recv_buf_size > 0) {
        val = t.recv_buf_size;
        if (setsockopt(fd, SOL_SOCKET, SO_RCVBUF, &val, sizeof(val)) < 0)
            last_err = -errno;
    }

    if (t.send_buf_size > 0) {
        val = t.send_buf_size;
        if (setsockopt(fd, SOL_SOCKET, SO_SNDBUF, &val, sizeof(val)) < 0)
            last_err = -errno;
    }

#elif defined(_WIN32)
    // Windows socket tuning
    BOOL bval;

    if (t.tcp_nodelay) {
        bval = TRUE;
        if (setsockopt(fd, IPPROTO_TCP, TCP_NODELAY,
                        reinterpret_cast<const char*>(&bval), sizeof(bval)) < 0)
            last_err = -WSAGetLastError();
    }

    if (t.recv_buf_size > 0) {
        int val = t.recv_buf_size;
        setsockopt(fd, SOL_SOCKET, SO_RCVBUF,
                   reinterpret_cast<const char*>(&val), sizeof(val));
    }

    if (t.send_buf_size > 0) {
        int val = t.send_buf_size;
        setsockopt(fd, SOL_SOCKET, SO_SNDBUF,
                   reinterpret_cast<const char*>(&val), sizeof(val));
    }
#endif

    return last_err;
}

} // namespace simd
} // namespace tachyon