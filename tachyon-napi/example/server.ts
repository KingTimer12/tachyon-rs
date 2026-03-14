import { TachyonRawServer } from '../index'

const server = new TachyonRawServer({ bindAddr: '0.0.0.0:3000', workers: 4 })

server.start((req) => {
  // GET / — sync 200 JSON
  if (req.method === 'GET' && req.path === '/') {
    return {
      status: 200,
      body: JSON.stringify({ message: 'hello from tachyon' }),
    }
  }

  // GET /health — async empty 200
  if (req.method === 'GET' && req.path === '/health') {
    return { status: 200, contentType: 'text' }
  }

  // POST /users — sync 201 JSON
  if (req.method === 'POST' && req.path === '/users') {
    const body = req.body ? JSON.parse(req.body) : {}
    return {
      status: 201,
      body: JSON.stringify({ id: 1, ...body }),
    }
  }

  // PUT /users/:id — async 204 no body
  if (req.method === 'PUT' && req.path.startsWith('/users/')) {
    return { status: 204 }
  }

  // 404 fallback
  return {
    status: 404,
    body: JSON.stringify({ error: 'not found' }),
  }
})

console.log('[example] tachyon listening on http://localhost:3000')
