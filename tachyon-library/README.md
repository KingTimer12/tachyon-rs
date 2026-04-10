# tachyon-rs

Native Rust HTTP server with TypeScript API for Node.js and Bun. Fast, secure, and extensible.

**[Leia em Portugues](README.pt-BR.md)**

## Installation

```bash
npm install tachyon-rs
# or
bun add tachyon-rs
```

## Quick Start

```typescript
import { Tachyon, TachyonResponse, status } from 'tachyon-rs'

new Tachyon()
  .get('/', 'Hello Tachyon!')
  .get('/json', { message: 'fast' })
  .get('/dynamic', (req) => status(200, { path: req.path }))
  .post('/echo', (req) => status(200, req.body ?? '{}'))
  .listen(3000)
```

## Responses

```typescript
import { TachyonResponse, status } from 'tachyon-rs'

// status(code, body) — shorthand for any status code
status(200, { message: 'ok' })
status(201, { id: 42 })
status(400, { error: 'bad request' })

// TachyonResponse — when you need headers or content-type control
new TachyonResponse(200, 'Hello!')
  .text()                                    // send as text/plain
  .header('Cache-Control', 'max-age=3600')
```

## Request

```typescript
app.get('/users', (req) => {
  req.method          // "GET"
  req.path            // "/users"
  req.body            // string | undefined
  req.header('x-api-key')  // string | undefined (lazy parsed, zero-cost if unused)
  req.headers         // ReadonlyMap<string, string>
  return status(200, [])
})
```

## Plugins

Lifecycle hooks: `pre` (before handler) and `pos` (after handler).

- `pre` returning a `TachyonResponse` short-circuits the request (e.g., 401).
- `pos` returning a `TachyonResponse` replaces the original response.
- Returning `void` from either continues normally.

```typescript
import { Tachyon, TachyonResponse, status, type Plugin } from 'tachyon-rs'

const auth: Plugin = {
  pre: (req) => {
    if (!req.header('authorization')) {
      return status(401, { error: 'Unauthorized' })
    }
  }
}

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

## Configuration

```typescript
new Tachyon({
  workers: 4,                   // worker threads (default: CPU count)
  security: 'basic',            // 'none' | 'basic' | 'strict'
  compressionThreshold: 1024,   // bytes, 0 = compress all, -1 = disabled
})
```

### Security presets

| Preset | Headers added |
|--------|---------------|
| `none` | None |
| `basic` | `X-Content-Type-Options: nosniff`, `X-Frame-Options: SAMEORIGIN` |
| `strict` | All from basic + `X-XSS-Protection`, `Referrer-Policy`, `Permissions-Policy`, `COOP`, `CORP` |

### Compression

Responses are automatically gzip-compressed when the client supports it (`Accept-Encoding: gzip`). Compression runs in Rust — zero JS overhead.

```typescript
new Tachyon()                               // default: compress bodies >= 1KB
new Tachyon({ compressionThreshold: 0 })    // compress everything
new Tachyon({ compressionThreshold: 4096 }) // compress bodies >= 4KB
new Tachyon({ compressionThreshold: -1 })   // disable compression
```

## How it works

- **Rust server** — TCP listener, buffer pool, HTTP parsing, and I/O run in Rust on Tokio workers
- **SIMD parser** — HTTP scanning with SSE4.2/AVX2/NEON (zero-copy, 68-90% faster than byte-by-byte)
- **Rust routing** — each route is dispatched via `HashMap<method, HashMap<path, handler>>` in Rust — O(1), no JS call for 404
- **Flat headers** — headers passed as a single string (1 allocation); parsed lazily in JS only if accessed
- **Native gzip** — compression in Rust, transparent to your handler
- **Non-blocking bridge** — JS callbacks use `rx.await` instead of blocking Tokio workers, so concurrency scales without thread explosion

## Platforms

| OS | Architecture |
|---|---|
| Linux | x64 |
| macOS | x64, ARM64 |
| Windows | x64 |

## License

MIT
