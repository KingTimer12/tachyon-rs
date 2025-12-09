# 🚀 Tachyon

High-performance, minimalist web framework powered by Rust.

Tachyon is a multi-runtime web framework that provides blazing-fast HTTP server capabilities across Node.js, Bun, and Deno. It leverages Rust's performance through NAPI bindings and FFI.

## ⚡ Features

- **Multi-Runtime Support**: Works seamlessly with Node.js, Bun, and Deno
- **High Performance**: Powered by Rust and Hyper HTTP library
- **Minimalist API**: Simple, intuitive routing interface
- **Zero Configuration**: Automatic runtime detection
- **Type-Safe**: Full TypeScript support

## 📦 Architecture

### Node.js & Bun

Uses NAPI (N-API) bindings for native performance:

- Compiled binary: `napi.*.node`
- Direct Rust integration through NAPI-RS

### Deno

Uses FFI (Foreign Function Interface):

- Dynamic library: `libtachyon_raw.so` (Linux), `.dll` (Windows), `.dylib` (macOS)
- Zero-copy data transfer

## 🛠️ Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/tachyon-rs.git
cd tachyon-rs

# Install dependencies
npm install

# Build the project
npm run build
```

## 🔨 Building

### Build Everything

```bash
npm run build
```

### Build Native Binaries

```bash
# Build both NAPI and FFI libraries
npm run build:native

# Build only NAPI (Node.js/Bun)
npm run build:napi

# Build only FFI (Deno)
npm run build:ffi
```

### Build TypeScript

```bash
npm run build:ts
```

## 🚀 Quick Start

### Node.js / Bun

```javascript
import { tachyon } from "tachyon";

const app = tachyon();

app.get("/", ({ response }) => {
  return response({ message: "Hello, World!" });
});

app.post("/data", ({ response, body }) => {
  return response({ received: body }, 201);
});

app.listen(3000, () => {
  console.log("Server running on http://localhost:3000");
});
```

Run with:

```bash
# Node.js
node examples/node-example.js

# Bun
bun examples/bun-example.js
```

### Deno

```typescript
import { tachyon } from "./src/deno.ts";

const app = tachyon();

app.get("/", ({ response }) => {
  return response({ message: "Hello from Deno!" });
});

app.listen(3000);
```

Run with:

```bash
deno run --allow-ffi --allow-read examples/deno-example.ts
```

## 📖 API Reference

### Creating an App

```javascript
import { tachyon } from "tachyon";
const app = tachyon();
```

### Routing

#### GET

```javascript
app.get("/path", ({ response, params, query, headers }) => {
  return response(data, status);
});
```

#### POST

```javascript
app.post("/path", ({ response, body }) => {
  return response(data, 201);
});
```

#### PUT

```javascript
app.put("/path/:id", ({ response, params, body }) => {
  return response({ id: params.id, ...body });
});
```

#### DELETE

```javascript
app.delete("/path/:id", ({ response, params }) => {
  return response(null, 204);
});
```

#### Other Methods

- `app.patch(path, callback)` - PATCH requests
- `app.head(path, callback)` - HEAD requests
- `app.options(path, callback)` - OPTIONS requests

### Context Object

The callback receives a context object with:

```typescript
interface TachyonContext {
  body?: any; // Request body (parsed JSON)
  params?: Record<string, string>; // URL parameters
  query?: Record<string, string>; // Query string parameters
  headers?: Record<string, string>; // Request headers
  response: ResponseFunction; // Response function
}
```

### Response Function

```typescript
response(data: any, status?: number): TachyonResponse
```

- `data`: Response data (will be JSON stringified)
- `status`: HTTP status code (default: 200)

### Server Control

```javascript
// Start server
app.listen(port, callback?);

// Stop server
app.close();

// Get runtime
const runtime = app.getRuntime(); // 'node' | 'bun' | 'deno'
```

## 🧪 Testing

### Run Simple Test

```bash
node examples/simple-test.js
```

### Run Benchmark

```bash
node examples/benchmark.js

# Then benchmark with tools like:
wrk -t4 -c100 -d30s http://localhost:3000/json
autocannon -c 100 -d 30 http://localhost:3000/json
oha -c 100 -z 30s http://localhost:3000/json
```

### Test All Runtimes

```bash
./examples/test-all.sh
```

## 📁 Project Structure

```
tachyon-rs/
├── src/
│   ├── adapters/
│   │   ├── node.ts      # Node.js NAPI adapter
│   │   ├── bun.ts       # Bun NAPI adapter
│   │   └── deno.ts      # Deno FFI adapter
│   ├── index.ts         # Main entrypoint (Node/Bun)
│   ├── deno.ts          # Deno entrypoint
│   ├── runtime.ts       # Runtime detection
│   └── types.ts         # TypeScript types
├── tachyon/
│   ├── src/
│   │   ├── core/        # Core Rust library
│   │   ├── napi/        # NAPI bindings
│   │   └── lib/         # FFI library
│   └── Cargo.toml       # Rust workspace
├── examples/            # Usage examples
└── dist/                # Compiled TypeScript
```

## 🏗️ Rust Core

The Rust core (`tachyon/src/core`) provides:

- High-performance HTTP server using Hyper
- Efficient routing with DashMap
- Zero-copy data handling
- Async runtime with Tokio

## 🔧 Development

```bash
# Clean build artifacts
npm run clean

# Build in watch mode (TypeScript)
tsc --watch

# Check Rust code
cd tachyon && cargo check --workspace
```

## 🎯 Roadmap

- [ ] Middleware support
- [ ] Request/response interceptors
- [ ] WebSocket support
- [ ] Static file serving
- [ ] Template engine integration
- [ ] Cookie and session management
- [ ] CORS handling
- [ ] Rate limiting
- [ ] Compression (gzip, brotli)
- [ ] Multi-threaded mode
- [ ] Clustering support

## 📄 License

MIT

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
