# tachyon-rs

Native Rust HTTP server with Node.js/Bun bindings. Built for speed, security, and extensibility.

**[Leia em Portugues](README.pt-BR.md)**

## Why tachyon?

| | tachyon | Express | Fastify | Elysia |
|---|---|---|---|---|
| Runtime | Rust (Tokio async) | Node.js | Node.js | Bun |
| HTTP parser | SIMD C++ (zero-copy) | llhttp (C) | llhttp (C) | llhttp (C) |
| Routing | Rust O(1) HashMap | JS trie | JS radix | JS radix |
| Buffer alloc | Pre-allocated pool (0 alloc/req) | GC-managed | GC-managed | GC-managed |
| 404 handling | Rust — zero JS call | JS | JS | JS |
| JS overhead | Handler only | Everything | Everything | Everything |

The server loop, HTTP parsing, buffer pool, routing dispatch, and 404 handling all run in Rust. JavaScript only executes the matched route handler.

## Usage

```typescript
import { Tachyon, TachyonResponse, status } from 'tachyon-rs'

new Tachyon()
  .get('/', 'Hello Tachyon!')
  .get('/json', { message: 'fast' })
  .get('/dynamic', (req) => status(200, { path: req.path }))
  .listen(3000)
```

## Plugins

Plugins use lifecycle hooks: `pre` (before handler) and `pos` (after handler).

```typescript
import { Tachyon, status, type Plugin } from 'tachyon-rs'

// Auth — blocks requests without token
const auth: Plugin = {
  pre: (req) => {
    if (!req.header('authorization')) {
      return status(401, { error: 'Unauthorized' })
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
  pos: (_req, res) => res.header('Access-Control-Allow-Origin', '*')
}

// Logger
const logger: Plugin = {
  pos: (req, res) => console.log(`${req.method} ${req.path} -> ${res.status}`)
}

new Tachyon({ security: 'strict' })
  .use(cors)
  .use(auth)
  .use(logger)
  .get('/api/users', () => status(200, []))
  .listen(3000)
```

## Security

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
new Tachyon()                               // default: compress bodies >= 1KB
new Tachyon({ compressionThreshold: 0 })    // compress everything
new Tachyon({ compressionThreshold: 4096 }) // compress bodies >= 4KB
new Tachyon({ compressionThreshold: -1 })   // disable compression
```

## Architecture

```
HTTP Request
    |
    v
[Rust] TcpListener (Tokio single-thread async)
    |
    v
[Rust] Buffer Pool (pre-allocated, RAII)
    |
    v
[C++] SIMD HTTP Parser (AVX2/SSE4.2/NEON, zero-copy)
    |
    v
[Rust] O(1) Route Dispatch (method_id → path HashMap)
    |                |
    |           404 in Rust (zero JS call)
    v
[Rust → JS] NAPI bridge (ThreadsafeFunction, rx.await — no blocking)
    |
    v
[JS] Plugin hooks (pre) → Route handler → Plugin hooks (pos)
    |
    v
[Rust] Response buffer → gzip (if above threshold) → write_all
```

### Bridge optimizations

- **Rust routing**: each route has its own JS function registered at startup. Dispatch is a two-level `HashMap<method_id, HashMap<path, handler>>` lookup — O(1), no JS overhead.
- **Flat headers**: request headers are passed as a single `"name\tvalue\n"` string (1 allocation) instead of a `Vec` of structs (20+ allocations per request). Parsed lazily in JS only if accessed.
- **Async bridge**: uses `rx.await` (Tokio oneshot channel) instead of `block_in_place`, so the event loop is never blocked — no thread explosion under load.

## Structure

```
tachyon-rs/
  tachyon-core/       Server, config, response builder
  tachyon-http/       Zero-copy HTTP parser, JSON writer
  tachyon-pool/       Buffer pool with RAII
  tachyon-simd/       C++ bridge (SIMD scan, socket tuning)
  tachyon-napi/       NAPI bindings (route registration, request bridge)
  tachyon-library/    TypeScript API (Tachyon class, plugins, routing)
  example/            Usage example
```

## Build

```bash
# Build native binding + TypeScript library
npm run build

# Run example
bun example/index.ts

# Tests
cargo test
```

## License

MIT
