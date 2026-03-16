# @tachyon-rs/server

Bindings NAPI nativos do tachyon-rs para Node.js e Bun. Este pacote e a ponte entre o servidor HTTP em Rust e o runtime JavaScript.

> **Nota:** Este pacote e usado internamente pelo [`tachyon-rs`](https://www.npmjs.com/package/tachyon-rs). Para a maioria dos casos, use `tachyon-rs` diretamente.

## Instalacao

```bash
npm install @tachyon-rs/server
```

O binario nativo correto para sua plataforma e instalado automaticamente via `optionalDependencies`.

### Plataformas suportadas

| Plataforma | Arquitetura | Pacote |
|---|---|---|
| Linux | x64 | `@tachyon-rs/server-linux-x64-gnu` |
| macOS | x64 | `@tachyon-rs/server-darwin-x64` |
| macOS | ARM64 | `@tachyon-rs/server-darwin-arm64` |
| Windows | x64 | `@tachyon-rs/server-win32-x64-msvc` |

## Uso direto (baixo nivel)

```typescript
import { TachyonRawServer } from '@tachyon-rs/server'

const server = new TachyonRawServer({
  bindAddr: '0.0.0.0:3000',
  workers: 4,
  security: 'basic',
  compressionThreshold: 1024,
})

server.start((request) => {
  return {
    status: 200,
    body: JSON.stringify({ hello: 'world' }),
    headers: [{ name: 'Content-Type', value: 'application/json' }],
  }
})
```

## Configuracao

| Opcao | Tipo | Default | Descricao |
|---|---|---|---|
| `bindAddr` | `string` | `"0.0.0.0:3000"` | Endereco e porta |
| `workers` | `number` | CPU count | Threads de worker |
| `stackSizeKb` | `number` | `64` | Stack size das coroutines (KB) |
| `buffersPerWorker` | `number` | `128` | Buffers no pool por worker |
| `bufferSize` | `number` | `8192` | Tamanho de cada buffer (bytes) |
| `timeoutSecs` | `number` | `30` | Timeout do handler (segundos) |
| `tcpNodelay` | `boolean` | `true` | TCP_NODELAY |
| `reusePort` | `boolean` | `true` | SO_REUSEPORT (Linux/BSD) |
| `tcpFastopen` | `boolean` | `true` | TCP Fast Open (Linux) |
| `busyPollUs` | `number` | `0` | SO_BUSY_POLL microseconds |
| `recvBufSize` | `number` | `0` | SO_RCVBUF (0 = OS default) |
| `sendBufSize` | `number` | `0` | SO_SNDBUF (0 = OS default) |
| `security` | `string` | `"basic"` | `"none"` \| `"basic"` \| `"strict"` |
| `compressionThreshold` | `number` | `1024` | 0 = comprime tudo, -1 = desabilitado |

## Build local

```bash
bun install
bun run build        # release
bun run build:debug  # debug
```

## Licenca

MIT
