#!/bin/bash

# Script para testar todos os runtimes

echo "🚀 Testing Tachyon across all runtimes"
echo "========================================"
echo ""

# Build first
echo "📦 Building project..."
npm run build
if [ $? -ne 0 ]; then
    echo "❌ Build failed"
    exit 1
fi
echo "✅ Build complete"
echo ""

# Test Node.js
echo "🟢 Testing Node.js..."
timeout 3 node examples/node-example.js &
NODE_PID=$!
sleep 1
curl -s http://localhost:3000/ | jq . || echo "Node.js test failed"
kill $NODE_PID 2>/dev/null
echo ""

# Test Bun
echo "🟠 Testing Bun..."
timeout 3 bun examples/bun-example.js &
BUN_PID=$!
sleep 1
curl -s http://localhost:3001/ | jq . || echo "Bun test failed"
kill $BUN_PID 2>/dev/null
echo ""

# Test Deno
echo "🦕 Testing Deno..."
timeout 3 deno run --allow-ffi --allow-read examples/deno-example.ts &
DENO_PID=$!
sleep 1
curl -s http://localhost:3002/ | jq . || echo "Deno test failed"
kill $DENO_PID 2>/dev/null
echo ""

echo "✅ All runtime tests completed!"
