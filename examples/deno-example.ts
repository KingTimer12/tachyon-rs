// Deno example
// Run with: deno run --allow-ffi --allow-read deno-example.ts
import { tachyon } from '../src/deno.ts';

const app = tachyon();

console.log('Runtime: Deno 🦕');

// Basic routes
app.get('/', ({ response }) => {
  return response({
    message: 'Hello from Tachyon on Deno!',
    version: Deno.version.deno
  });
});

app.get('/env', ({ response }) => {
  return response({
    arch: Deno.build.arch,
    os: Deno.build.os,
    target: Deno.build.target
  });
});

app.post('/json', ({ response, body }) => {
  return response({
    received: body,
    processed: true
  });
});

app.get('/file', ({ response }) => {
  // This is just an example - in real implementation you'd read files
  return response({
    message: 'File operations would go here',
    note: 'Deno has excellent file system APIs'
  });
});

// Start server
app.listen(3002, () => {
  console.log('Deno server running on http://localhost:3002');
  console.log('Try:');
  console.log('  curl http://localhost:3002/');
  console.log('  curl http://localhost:3002/env');
  console.log('  curl -X POST http://localhost:3002/json -H "Content-Type: application/json" -d \'{"test":"data"}\'');
});
