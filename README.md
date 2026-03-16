# tachyon-rs

Native Rust HTTP server with Node.js/Bun bindings. Built for speed, security, and extensibility.

**[Leia em Portugues](README.pt-BR.md)**

## Inspiration

Tachyon combines ideas from projects that solve specific problems really well:

- **FaF** — Callback model with pre-allocated buffers, zero-copy parsing, and thread-local pool. Tachyon's core architecture follows this pattern.
- **picohttpparser** — HTTP delimiter scanning with SIMD (SSE4.2/AVX2/NEON). 68-90% faster than byte-by-byte.
- **simdjson** — High-performance JSON parsing bridge via C++.
- **May** — Lightweight cooperative coroutines. One per connection, no OS thread overhead.
- **Windows RIO** — Registered I/O for zero-copy networking on Windows 8+.

## Why tachyon?

| | tachyon | Express | Fastify | Hono |
|---|---|---|---|---|
| Runtime | Rust (coroutines) | Node.js (event loop) | Node.js (event loop) | Node.js (event loop) |
| HTTP parser | SIMD C++ (zero-copy) | http_parser (C) | llhttp (C) | llhttp (C) |
| Buffer alloc | Pre-allocated pool (0 alloc/req) | GC-managed | GC-managed | GC-managed |
| Threads | N coroutines per worker thread | Single-threaded | Single-threaded | Single-threaded |
| JS overhead | Minimal (handler only) | Everything in JS | Everything in JS | Everything in JS |

The server loop, HTTP parsing, buffer pool, and I/O run in Rust. JS only executes the user's handler.

## Usage

```typescript
import { Tachyon, TachyonResponse } from 'tachyon-rs'

new Tachyon()
  .get('/', 'Hello Tachyon!')
  .get('/json', { message: 'fast' })
  .get('/dynamic', (req) => {
    return new TachyonResponse(200, JSON.stringify({ path: req.path }))
  })
  .listen(3000)
```

## Plugins

Plugins use lifecycle hooks: `pre` (before handler) and `pos` (after handler).

```typescript
import { Tachyon, TachyonResponse, type Plugin } from 'tachyon-rs'

// JWT — blocks requests without token
const auth: Plugin = {
  pre: (req) => {
    const token = req.header('authorization')
    if (!token) {
      return new TachyonResponse(401, JSON.stringify({ error: 'Unauthorized' }))
    }
  }
}

// CORS
const cors: Plugin = {
  pre: (req) => {
    if (req.method === 'OPTIONS') {
      return new TachyonResponse(204, '')
        .header('Access-Control-Allow-Origin', '*')
        .header('Access-Control-Allow-Methods', 'GET, POST, PUT, DELETE')
        .header('Access-Control-Allow-Headers', 'Content-Type, Authorization')
    }
  },
  pos: (_req, res) => {
    return res.header('Access-Control-Allow-Origin', '*')
  }
}

// Logger
const logger: Plugin = {
  pos: (req, res) => {
    console.log(`${req.method} ${req.path} -> ${res.status}`)
  }
}

new Tachyon({ security: 'strict' })
  .use(cors)
  .use(auth)
  .use(logger)
  .get('/api/users', () => new TachyonResponse(200, '[]'))
  .listen(3000)
```

## Security

Security headers are configured by preset on the server:

```typescript
new Tachyon({ security: 'basic' })   // default
new Tachyon({ security: 'strict' })  // production
new Tachyon({ security: 'none' })    // max speed (use behind a proxy)
```

| Preset | Headers |
|--------|---------|
| `none` | None |
| `basic` | `X-Content-Type-Options: nosniff`, `X-Frame-Options: SAMEORIGIN` |
| `strict` | All from basic + `X-XSS-Protection`, `Referrer-Policy`, `Permissions-Policy`, `COOP`, `CORP` |

## Compression

Large responses are automatically gzip-compressed when the client supports it (`Accept-Encoding: gzip`). Compression happens in Rust, transparent to the JS handler.

```typescript
new Tachyon()                              // default: compress bodies >= 1KB
new Tachyon({ compressionThreshold: 0 })   // compress everything
new Tachyon({ compressionThreshold: 4096 })// compress bodies >= 4KB
new Tachyon({ compressionThreshold: -1 })  // disable compression
```

- Bodies below the threshold are sent uncompressed (zero overhead)
- If gzip doesn't reduce size, sends uncompressed
- Bodies larger than the buffer pool (8KB) use heap allocation automatically

## Architecture

```
HTTP Request
    |
    v
[Rust] TcpListener (May coroutines, N workers)
    |
    v
[Rust] Buffer Pool (pre-allocated, thread-local, RAII)
    |
    v
[C++] SIMD HTTP Parser (AVX2/SSE4.2/NEON)
    |
    v
[Rust -> JS] NAPI bridge (ThreadsafeFunction)
    |
    v
[JS] Plugin hooks (pre) -> Route handler -> Plugin hooks (pos)
    |
    v
[Rust] Response buffer -> gzip (if threshold) -> write_all / RIO zero-copy
```

## Structure

```
tachyon-rs/
  tachyon-core/       Server, config, response builder, RIO
  tachyon-http/       Zero-copy HTTP parser, response constants
  tachyon-pool/       Thread-local buffer pool with RAII
  tachyon-simd/       C++ bridge (SIMD scan, socket tuning, RIO)
  tachyon-napi/       NAPI bindings for Node.js/Bun
  tachyon-library/    TypeScript API (Tachyon, plugins, routing)
  example/            Usage example
```

## Build

```bash
# Build native binding
npm run build

# Run example
npm run example

# Tests
cargo test
```

## License

MIT
