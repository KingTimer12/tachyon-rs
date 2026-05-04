import { makeUsers, makeLargePayload, PORT } from "../shared.ts";
import { Tachyon, status } from 'tachyon-rs'

const app = new Tachyon({ security: "none", compressionThreshold: -1, catchPanics: false });

app.get("/text",  "Hello, World!");
app.get("/json",  { message: "Hello, World!" });
app.get("/users", () => status(200, makeUsers(10)));
app.get("/large", () => status(200, makeLargePayload(100)));
app.post("/echo", (req: any) => status(200, req.body ?? "{}"));

app.listen(PORT);
process.stdout.write("ready\n");
