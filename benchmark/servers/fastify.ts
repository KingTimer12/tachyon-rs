import Fastify from "fastify";
import { makeUsers, makeLargePayload, PORT } from "../shared.ts";

const app = Fastify({ logger: false });

app.get("/text", (_req, reply) => {
  reply.type("text/plain").send("Hello, World!");
});

app.get("/json", (_req, reply) => {
  reply.send({ message: "Hello, World!" });
});

app.get("/users", (_req, reply) => {
  reply.send(makeUsers(10));
});

app.get("/large", (_req, reply) => {
  reply.send(makeLargePayload(100));
});

app.post("/echo", (req, reply) => {
  reply.send(req.body);
});

await app.listen({ port: PORT, host: "0.0.0.0" });
process.stdout.write("ready\n");
