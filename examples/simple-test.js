// Simple test to validate all HTTP methods work correctly
import { tachyon } from "../dist/index.js";

const app = tachyon();

console.log(`Runtime detected: ${app.getRuntime()}`);
console.log("Starting Tachyon test server...\n");

// GET route
app.get("/", ({ response }) => {
  return response({
    message: "Hello from Tachyon!",
    method: "GET",
  });
});

// GET with params
app.get("/user/:id", ({ response, params }) => {
  return response({
    method: "GET",
    userId: params?.id,
    message: `User ${params?.id} found`,
  });
});

// POST route
app.post("/data", ({ response, body }) => {
  return response(
    {
      method: "POST",
      received: body,
      message: "Data received successfully",
    },
    201,
  );
});

// PUT route
app.put("/update/:id", ({ response, params, body }) => {
  return response({
    method: "PUT",
    id: params?.id,
    updated: body,
    message: `Resource ${params?.id} updated`,
  });
});

// DELETE route
app.delete("/delete/:id", ({ response, params }) => {
  return response({
    method: "DELETE",
    id: params?.id,
    message: `Resource ${params?.id} deleted`,
  });
});

// PATCH route
app.patch("/patch/:id", ({ response, params, body }) => {
  return response({
    method: "PATCH",
    id: params?.id,
    patched: body,
    message: `Resource ${params?.id} patched`,
  });
});

// Health check
app.get("/health", ({ response }) => {
  return response({
    status: "healthy",
    runtime: app.getRuntime(),
    timestamp: new Date().toISOString(),
  });
});

// JSON response test
app.get("/json", ({ response }) => {
  return response({
    string: "hello",
    number: 42,
    boolean: true,
    array: [1, 2, 3],
    nested: { foo: "bar" },
  });
});

const PORT = process.env.PORT || 3000;

app.listen(PORT, () => {
  console.log(`✅ Server running on http://localhost:${PORT}`);
  console.log("\n📋 Test endpoints:");
  console.log(`  GET    http://localhost:${PORT}/`);
  console.log(`  GET    http://localhost:${PORT}/user/123`);
  console.log(`  POST   http://localhost:${PORT}/data`);
  console.log(`  PUT    http://localhost:${PORT}/update/123`);
  console.log(`  DELETE http://localhost:${PORT}/delete/123`);
  console.log(`  PATCH  http://localhost:${PORT}/patch/123`);
  console.log(`  GET    http://localhost:${PORT}/health`);
  console.log(`  GET    http://localhost:${PORT}/json`);
  console.log("\n📝 Example commands:");
  console.log(`  curl http://localhost:${PORT}/`);
  console.log(`  curl http://localhost:${PORT}/user/42`);
  console.log(
    `  curl -X POST http://localhost:${PORT}/data -H "Content-Type: application/json" -d '{"name":"test"}'`,
  );
  console.log(
    `  curl -X PUT http://localhost:${PORT}/update/1 -H "Content-Type: application/json" -d '{"name":"updated"}'`,
  );
  console.log(`  curl -X DELETE http://localhost:${PORT}/delete/1`);
  console.log(
    `  curl -X PATCH http://localhost:${PORT}/patch/1 -H "Content-Type: application/json" -d '{"field":"value"}'`,
  );
});
