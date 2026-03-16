# tachyon-rs

HTTP server nativo em Rust com API TypeScript para Node.js e Bun. Rapido, seguro e extensivel.

## Instalacao

```bash
npm install tachyon-rs
# ou
bun add tachyon-rs
```

## Quick Start

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

Hooks de ciclo de vida: `pre` (antes do handler) e `pos` (depois do handler).

```typescript
import { Tachyon, TachyonResponse, type Plugin } from 'tachyon-rs'

const auth: Plugin = {
  pre: (req) => {
    if (!req.header('authorization')) {
      return new TachyonResponse(401, JSON.stringify({ error: 'Unauthorized' }))
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
  pos: (_req, res) => {
    return res.header('Access-Control-Allow-Origin', '*')
  }
}

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

## Configuracao

```typescript
new Tachyon({
  workers: 4,                    // threads (default: CPU count)
  security: 'basic',            // 'none' | 'basic' | 'strict'
  compressionThreshold: 1024,   // bytes, 0 = tudo, -1 = desabilitado
})
```

### Seguranca

| Preset | Headers |
|--------|---------|
| `none` | Nenhum |
| `basic` | `X-Content-Type-Options: nosniff`, `X-Frame-Options: SAMEORIGIN` |
| `strict` | Todos de basic + `X-XSS-Protection`, `Referrer-Policy`, `Permissions-Policy`, `COOP`, `CORP` |

### Compressao

Respostas grandes sao comprimidas com gzip automaticamente quando o cliente suporta (`Accept-Encoding: gzip`).

```typescript
new Tachyon()                              // default: comprime bodies >= 1KB
new Tachyon({ compressionThreshold: 0 })   // comprime tudo
new Tachyon({ compressionThreshold: 4096 })// comprime bodies >= 4KB
new Tachyon({ compressionThreshold: -1 })  // desabilita compressao
```

## Por que tachyon?

- **Servidor em Rust** — loop, parsing HTTP e I/O rodam em Rust com coroutines
- **Parser SIMD** — scanning HTTP com SSE4.2/AVX2/NEON, 68-90% mais rapido
- **Zero allocation** — buffer pool pre-alocado, zero alloc por request
- **Gzip nativo** — compressao transparente no Rust
- **Minimo overhead JS** — so o handler do usuario roda em JavaScript

## Plataformas

| OS | Arquitetura |
|---|---|
| Linux | x64 |
| macOS | x64, ARM64 |
| Windows | x64 |

## Licenca

MIT
