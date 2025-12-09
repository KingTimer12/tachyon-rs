import { createRequire } from "node:module";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import type {
  ITachyonAdapter,
  RouteCallback,
  TachyonContext,
  ResponseFunction,
} from "../types.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const require = createRequire(import.meta.url);

// Types matching what Rust expects
interface JsCallbackData {
  body?: string;
  params?: string;
}

interface JsCallbackResult {
  data: string;
  status: number;
}

export class NodeAdapter implements ITachyonAdapter {
  private native: any;
  private routes: Map<string, RouteCallback>;

  constructor() {
    const binaryPath = join(__dirname, "../../napi.linux-x64-gnu.node");
    try {
      const module = require(binaryPath);
      this.native = new module.Tachyon();
    } catch (error) {
      throw new Error(`Failed to load NAPI binary for Node.js: ${error}`);
    }
    this.routes = new Map();
  }

  registerRoute(method: string, path: string, callback: RouteCallback): void {
    const routeKey = `${method}:${path}`;
    this.routes.set(routeKey, callback);

    // Create wrapper that converts between TS and Rust formats
    // The callback signature from Rust is: (err: Error | null, data: JsCallbackData) => JsCallbackResult
    const rustCallback = (
      err: Error | null,
      data: JsCallbackData,
    ): JsCallbackResult => {
      try {
        // Handle error from Rust side
        if (err) {
          console.error(`Error from Rust in route ${routeKey}:`, err);
          return {
            data: JSON.stringify({ error: err.message }),
            status: 500,
          };
        }

        // Parse incoming data from Rust (strings that are JSON)
        const body = data.body ? JSON.parse(data.body) : undefined;
        const params = data.params ? JSON.parse(data.params) : undefined;

        // Create response function for user convenience
        const response: ResponseFunction = (
          responseData: any,
          status: number = 200,
        ) => {
          return { data: responseData, status };
        };

        // Build context object for user's callback
        const ctx: TachyonContext = {
          body,
          params,
          query: undefined, // TODO: implement query parsing
          headers: undefined, // TODO: implement headers
          response,
        };

        // Call the user's callback
        const result = callback(ctx);

        // Handle sync result (Promise handling would need async support from Rust side)
        if (result && typeof result === "object" && "data" in result) {
          return {
            data: JSON.stringify(result.data),
            status: result.status || 200,
          };
        }

        // Fallback response
        return {
          data: JSON.stringify({ error: "Invalid response from callback" }),
          status: 500,
        };
      } catch (error) {
        // Handle errors gracefully
        console.error(`Error in route callback ${routeKey}:`, error);
        return {
          data: JSON.stringify({
            error: error instanceof Error ? error.message : "Unknown error",
          }),
          status: 500,
        };
      }
    };

    // Register with Rust - the callback is wrapped by ThreadsafeFunction
    this.native.registerRoute(method, path, rustCallback);
  }

  listen(port: number): void {
    this.native
      .listen(port)
      .then(() => {
        // Success - server is running
      })
      .catch((err: Error) => {
        console.error(`Failed to start server: ${err.message}`);
      });
  }

  close(): void {
    this.routes.clear();
  }
}
