// Node.js example
import { tachyon } from '../dist/index.js';

const app = tachyon();

console.log(`Runtime detected: ${app.getRuntime()}`);

// Basic routes
app.get('/', ({ response }) => {
  return response({ message: 'Hello from Tachyon on Node.js!' });
});

app.get('/user/:id', ({ response, params }) => {
  return response({
    user: params?.id,
    runtime: 'node'
  });
});

app.post('/data', ({ response, body }) => {
  return response({
    received: body,
    status: 'created'
  }, 201);
});

app.put('/update/:id', ({ response, params, body }) => {
  return response({
    id: params?.id,
    updated: body,
    timestamp: Date.now()
  });
});

app.delete('/delete/:id', ({ response, params }) => {
  return response(null, 204);
});

// Start server
app.listen(3000, () => {
  console.log('Node.js server running on http://localhost:3000');
  console.log('Try:');
  console.log('  curl http://localhost:3000/');
  console.log('  curl http://localhost:3000/user/123');
  console.log('  curl -X POST http://localhost:3000/data -H "Content-Type: application/json" -d \'{"name":"test"}\'');
});
