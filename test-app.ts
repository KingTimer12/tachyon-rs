// Quick test application
import { tachyon } from "./src/index";

console.log("🚀 Starting Tachyon test application...\n");

const app = tachyon();

console.log(`📦 Runtime: ${app.getRuntime()}`);

// Health check endpoint
app.get("/health", ({ response }) => {
  return response({
    status: "ok",
    timestamp: Date.now(),
    runtime: app.getRuntime(),
  });
});

// Root endpoint
app.get("/", ({ response }) => {
  return response({
    message: "Welcome to Tachyon!",
    version: "0.1.0",
    endpoints: [
      "GET  /",
      "GET  /health",
      "GET  /hello/:name",
      "POST /data",
      "PUT  /update/:id",
      "DELETE /delete/:id",
    ],
  });
});

// Parameterized route
app.get("/hello/:name", ({ response, params }) => {
  return response({
    message: `Hello, ${params?.name || "World"}!`,
    timestamp: Date.now(),
  });
});

// POST endpoint
app.post("/data", ({ response, body }) => {
  return response(
    {
      received: body,
      processed: true,
      timestamp: Date.now(),
    },
    201,
  );
});

// PUT endpoint
app.put("/update/:id", ({ response, params, body }) => {
  return response({
    id: params?.id,
    data: body,
    updated: true,
    timestamp: Date.now(),
  });
});

// DELETE endpoint
app.delete("/delete/:id", ({ response, params }) => {
  console.log(`Deleting item ${params?.id}`);
  return response(null, 204);
});

// Error handling example
app.get("/error", ({ response }) => {
  return response(
    {
      error: "This is an error response",
    },
    500,
  );
});

const PORT = 3000;

app.listen(PORT, () => {
  console.log(`\n✅ Server is running on http://localhost:${PORT}`);
  console.log("\n📝 Test commands:");
  console.log(`   curl http://localhost:${PORT}/`);
  console.log(`   curl http://localhost:${PORT}/health`);
  console.log(`   curl http://localhost:${PORT}/hello/Tachyon`);
  console.log(
    `   curl -X POST http://localhost:${PORT}/data -H "Content-Type: application/json" -d '{"test": "data"}'`,
  );
  console.log(
    `   curl -X PUT http://localhost:${PORT}/update/123 -H "Content-Type: application/json" -d '{"value": "updated"}'`,
  );
  console.log(`   curl -X DELETE http://localhost:${PORT}/delete/456`);
  console.log("\n⚡ Press Ctrl+C to stop\n");
});

// Graceful shutdown
process.on("SIGINT", () => {
  console.log("\n\n🛑 Shutting down gracefully...");
  app.close();
  console.log("✅ Server closed");
  process.exit(0);
});

process.on("SIGTERM", () => {
  console.log("\n\n🛑 Received SIGTERM, shutting down...");
  app.close();
  process.exit(0);
});
