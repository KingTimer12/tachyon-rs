# tachyon-rs

HTTP server nativo em Rust com bindings para Node.js/Bun. Construido para ser rapido, seguro e extensivel.

**[Read in English](README.md)**

## Por que tachyon?

| | tachyon | Express | Fastify | Elysia |
|---|---|---|---|---|
| Runtime | Rust (Tokio async) | Node.js | Node.js | Bun |
| HTTP parser | SIMD C++ (zero-copy) | llhttp (C) | llhttp (C) | llhttp (C) |
| Routing | Rust O(1) HashMap | JS trie | JS radix | JS radix |
| Buffer alloc | Pool pre-alocado (0 alloc/req) | GC-managed | GC-managed | GC-managed |
| 404 handling | Rust — zero chamada JS | JS | JS | JS |
| Overhead JS | So o handler | Tudo em JS | Tudo em JS | Tudo em JS |

O loop do servidor, parsing HTTP, buffer pool, dispatch de rotas e tratamento de 404 rodam em Rust. O JavaScript so executa o handler da rota correspondente.

## Uso

```typescript
import { Tachyon, TachyonResponse, status } from 'tachyon-rs'

new Tachyon()
  .get('/', 'Hello Tachyon!')
  .get('/json', { message: 'fast' })
  .get('/dynamic', (req) => status(200, { path: req.path }))
  .listen(3000)
```

## Plugins

Plugins usam hooks de ciclo de vida: `pre` (antes do handler) e `pos` (depois do handler).

```typescript
import { Tachyon, status, type Plugin } from 'tachyon-rs'

// Auth — bloqueia requests sem token
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

## Seguranca

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
new Tachyon()                               // default: comprime bodies >= 1KB
new Tachyon({ compressionThreshold: 0 })    // comprime tudo
new Tachyon({ compressionThreshold: 4096 }) // comprime bodies >= 4KB
new Tachyon({ compressionThreshold: -1 })   // desabilita compressao
```

- Bodies abaixo do threshold sao enviados sem compressao (zero overhead)
- Se o gzip nao reduzir o tamanho, envia sem compressao
- Bodies maiores que o buffer pool (8KB) usam heap allocation automaticamente

## Arquitetura

```
Request HTTP
    |
    v
[Rust] TcpListener (Tokio async single-thread)
    |
    v
[Rust] Buffer Pool (pre-alocado, RAII)
    |
    v
[C++] SIMD HTTP Parser (AVX2/SSE4.2/NEON, zero-copy)
    |
    v
[Rust] O(1) Route Dispatch (method_id → path HashMap)
    |                |
    |           404 em Rust (zero chamada JS)
    v
[Rust → JS] NAPI bridge (ThreadsafeFunction, rx.await — sem bloqueio)
    |
    v
[JS] Plugin hooks (pre) → Route handler → Plugin hooks (pos)
    |
    v
[Rust] Response buffer → gzip (se threshold) → write_all
```

### Otimizacoes da bridge

- **Routing em Rust**: cada rota tem sua propria funcao JS registrada no startup. O dispatch usa `HashMap<method_id, HashMap<path, handler>>` — O(1), sem overhead JS.
- **Headers flat**: os headers do request sao passados como uma unica string `"name\tvalue\n"` (1 alocacao) em vez de um `Vec` de structs (20+ alocacoes por request). Parseados lazily no JS somente se acessados.
- **Bridge assincrona**: usa `rx.await` (Tokio oneshot channel) em vez de `block_in_place`, garantindo que o event loop nunca bloqueie.

## Estrutura

```
tachyon-rs/
  tachyon-core/       Servidor, config, response builder
  tachyon-http/       Parser HTTP zero-copy, JSON writer
  tachyon-pool/       Buffer pool com RAII
  tachyon-simd/       Bridge C++ (SIMD scan, socket tuning)
  tachyon-napi/       Bindings NAPI (registro de rotas, bridge de request)
  tachyon-library/    API TypeScript (Tachyon class, plugins, routing)
  example/            Exemplo de uso
```

## Build

```bash
# Compilar o binding nativo + biblioteca TypeScript
npm run build

# Rodar o exemplo
bun example/index.ts

# Testes
cargo test
```

## Licenca

MIT
