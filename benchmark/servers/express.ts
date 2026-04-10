import express from "express";
import { makeUsers, makeLargePayload, PORT } from "../shared.ts";

const app = express();
app.use(express.json());

app.get("/text", (_req, res) => {
  res.type("text").send("Hello, World!");
});

app.get("/json", (_req, res) => {
  res.json({ message: "Hello, World!" });
});

app.get("/users", (_req, res) => {
  res.json(makeUsers(10));
});

app.get("/large", (_req, res) => {
  res.json(makeLargePayload(100));
});

app.post("/echo", (req, res) => {
  res.json(req.body);
});

app.listen(PORT, () => {
  process.stdout.write("ready\n");
});
