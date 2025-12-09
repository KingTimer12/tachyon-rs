import { detectRuntime, type Runtime } from "./runtime.js";
import { createRequire } from "module";
import { fileURLToPath } from "url";
import { dirname } from "path";
import type {
  ITachyonAdapter,
  RouteCallback,
  TachyonContext,
  ResponseFunction,
  HTTPMethod,
} from "./types.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const require = createRequire(import.meta.url);

export class Tachyon {
  private adapter: ITachyonAdapter;
  private runtime: Runtime;

  constructor() {
    this.runtime = detectRuntime();
    this.adapter = this.loadAdapter();
  }

  private loadAdapter(): ITachyonAdapter {
    switch (this.runtime) {
      case "node":
        return this.loadNodeAdapter();
      case "bun":
        return this.loadBunAdapter();
      case "deno":
        return this.loadDenoAdapter();
      default:
        throw new Error(`Unsupported runtime: ${this.runtime}`);
    }
  }

  private loadNodeAdapter(): ITachyonAdapter {
    const { NodeAdapter } = require("./adapters/node.js");
    return new NodeAdapter();
  }

  private loadBunAdapter(): ITachyonAdapter {
    const { BunAdapter } = require("./adapters/bun.js");
    return new BunAdapter();
  }

  private loadDenoAdapter(): ITachyonAdapter {
    throw new Error(
      "Deno adapter must be imported directly via deno.ts entrypoint",
    );
  }

  /**
   * Register a GET route
   */
  get(path: string, callback: RouteCallback): this {
    this.adapter.registerRoute("GET", path, callback);
    return this;
  }

  /**
   * Register a POST route
   */
  post(path: string, callback: RouteCallback): this {
    this.adapter.registerRoute("POST", path, callback);
    return this;
  }

  /**
   * Register a PUT route
   */
  put(path: string, callback: RouteCallback): this {
    this.adapter.registerRoute("PUT", path, callback);
    return this;
  }

  /**
   * Register a DELETE route
   */
  delete(path: string, callback: RouteCallback): this {
    this.adapter.registerRoute("DELETE", path, callback);
    return this;
  }

  /**
   * Register a PATCH route
   */
  patch(path: string, callback: RouteCallback): this {
    this.adapter.registerRoute("PATCH", path, callback);
    return this;
  }

  /**
   * Register a HEAD route
   */
  head(path: string, callback: RouteCallback): this {
    this.adapter.registerRoute("HEAD", path, callback);
    return this;
  }

  /**
   * Register an OPTIONS route
   */
  options(path: string, callback: RouteCallback): this {
    this.adapter.registerRoute("OPTIONS", path, callback);
    return this;
  }

  /**
   * Start the server on the specified port
   */
  listen(port: number, callback?: () => void): void {
    this.adapter.listen(port);
    if (callback) {
      callback();
    }
  }

  /**
   * Close the server
   */
  close(): void {
    if (this.adapter.close) {
      this.adapter.close();
    }
  }

  /**
   * Get the current runtime
   */
  getRuntime(): Runtime {
    return this.runtime;
  }
}

/**
 * Factory function to create a new Tachyon instance
 */
export function tachyon(): Tachyon {
  return new Tachyon();
}

// Re-export types
export type {
  TachyonContext,
  TachyonResponse,
  RouteCallback,
  HTTPMethod,
  ResponseFunction,
} from "./types.js";
export { detectRuntime, type Runtime } from "./runtime.js";
