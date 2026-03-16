# tachyon-rs

HTTP server nativo em Rust com bindings para Node.js/Bun. Construido para ser rapido, seguro e extensivel.

## Inspiracao

Tachyon combina ideias de projetos que resolvem problemas especificos muito bem:

- **FaF** — Modelo de callback com buffers pre-alocados, zero-copy parsing e pool thread-local. A arquitetura core do tachyon segue esse pattern.
- **picohttpparser** — Scanning de delimitadores HTTP com SIMD (SSE4.2/AVX2/NEON). 68-90% mais rapido que byte-a-byte.
- **simdjson** — Bridge para parsing JSON de alta performance via C++.
- **May** — Coroutines cooperativas leves. Uma por conexao, sem overhead de threads OS.
- **Windows RIO** — Registered I/O para zero-copy networking no Windows 8+.

## Por que tachyon?

| | tachyon | Express | Fastify | Hono |
|---|---|---|---|---|
| Runtime | Rust (coroutines) | Node.js (event loop) | Node.js (event loop) | Node.js (event loop) |
| HTTP parser | SIMD C++ (zero-copy) | http_parser (C) | llhttp (C) | llhttp (C) |
| Buffer alloc | Pool pre-alocado (0 alloc/req) | GC-managed | GC-managed | GC-managed |
| Threads | N coroutines por worker thread | Single-threaded | Single-threaded | Single-threaded |
| Overhead JS | Minimo (handler only) | Tudo em JS | Tudo em JS | Tudo em JS |

O loop do servidor, parsing HTTP, buffer pool e I/O rodam em Rust. O JS so executa o handler do usuario.

## Uso

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

Plugins usam hooks de ciclo de vida: `pre` (antes do handler) e `pos` (depois do handler).

```typescript
import { Tachyon, TachyonResponse, type Plugin } from 'tachyon-rs'

// JWT — bloqueia requests sem token
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

## Seguranca

Headers de seguranca sao configurados por preset no servidor:

```typescript
new Tachyon({ security: 'basic' })   // padrao
new Tachyon({ security: 'strict' })  // producao
new Tachyon({ security: 'none' })    // velocidade maxima (use atras de proxy)
```

| Preset | Headers |
|--------|---------|
| `none` | Nenhum |
| `basic` | `X-Content-Type-Options: nosniff`, `X-Frame-Options: SAMEORIGIN` |
| `strict` | Todos de basic + `X-XSS-Protection`, `Referrer-Policy`, `Permissions-Policy`, `COOP`, `CORP` |

## Compressao

Respostas grandes sao comprimidas com gzip automaticamente quando o cliente suporta (`Accept-Encoding: gzip`). A compressao acontece no Rust, transparente para o handler JS.

```typescript
new Tachyon()                              // default: comprime bodies >= 1KB
new Tachyon({ compressionThreshold: 0 })   // comprime tudo
new Tachyon({ compressionThreshold: 4096 })// comprime bodies >= 4KB
new Tachyon({ compressionThreshold: -1 })  // desabilita compressao
```

- Bodies abaixo do threshold sao enviados sem compressao (zero overhead)
- Se o gzip nao reduzir o tamanho, envia sem compressao
- Bodies maiores que o buffer pool (8KB) usam heap allocation automaticamente

## Arquitetura

```
Request HTTP
    |
    v
[Rust] TcpListener (May coroutines, N workers)
    |
    v
[Rust] Buffer Pool (pre-alocado, thread-local, RAII)
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
[Rust] Response buffer -> gzip (se threshold) -> write_all / RIO zero-copy
```

## Estrutura

```
tachyon-rs/
  tachyon-core/       Servidor, config, response builder, RIO
  tachyon-http/       Parser HTTP zero-copy, constantes de response
  tachyon-pool/       Buffer pool thread-local com RAII
  tachyon-simd/       Bridge C++ (SIMD scan, socket tuning, RIO)
  tachyon-napi/       Bindings NAPI para Node.js/Bun
  tachyon-library/    API TypeScript (Tachyon, plugins, routing)
  example/            Exemplo de uso
```

## Build

```bash
# Compilar o binding nativo
npm run build

# Rodar o exemplo
npm run example

# Testes
cargo test
```

## Licenca

MIT
