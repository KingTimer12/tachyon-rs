// Bun example
import { tachyon } from '../dist/index.js';

const app = tachyon();

console.log(`Runtime detected: ${app.getRuntime()}`);

// Basic routes
app.get('/', ({ response }) => {
  return response({
    message: 'Hello from Tachyon on Bun!',
    performance: 'blazing fast ⚡'
  });
});

app.get('/benchmark', ({ response }) => {
  const start = performance.now();

  // Simulate some work
  let sum = 0;
  for (let i = 0; i < 1000000; i++) {
    sum += i;
  }

  const end = performance.now();

  return response({
    result: sum,
    time: `${(end - start).toFixed(2)}ms`,
    runtime: 'bun'
  });
});

app.post('/echo', ({ response, body }) => {
  return response({
    echo: body,
    timestamp: Date.now()
  });
});

app.get('/health', ({ response }) => {
  return response({
    status: 'healthy',
    uptime: process.uptime(),
    memory: process.memoryUsage()
  });
});

// Start server
app.listen(3001, () => {
  console.log('Bun server running on http://localhost:3001');
  console.log('Try:');
  console.log('  curl http://localhost:3001/');
  console.log('  curl http://localhost:3001/benchmark');
  console.log('  curl http://localhost:3001/health');
});
