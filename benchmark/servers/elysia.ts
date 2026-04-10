import { Elysia } from "elysia";
import { makeUsers, makeLargePayload, PORT } from "../shared.ts";

new Elysia()
  .get("/text", () => new Response("Hello, World!", { headers: { "Content-Type": "text/plain" } }))
  .get("/json", () => ({ message: "Hello, World!" }))
  .get("/users", () => makeUsers(10))
  .get("/large", () => makeLargePayload(100))
  .post("/echo", ({ body }) => body)
  .listen(PORT, () => {
    process.stdout.write("ready\n");
  });
