import { TachyonRawServer } from "@tachyon-rs/server";
import { TachyonRequest } from "./request";
import { TachyonResponse } from "./response";
import type { TachyonConfig } from "./config";
import { status } from "./helper";

const methods = ["GET", "POST", "PUT", "DELETE"]

/**
 * Pre-request hook. Runs before the route handler.
 * - Return a `TachyonResponse` to short-circuit (e.g., 401 Unauthorized).
 * - Return `void` to let the request continue to the next hook / route handler.
 */
export type OnRequestHook = (req: TachyonRequest) => TachyonResponse | void

/**
 * Post-response hook. Runs after the route handler has produced a response.
 * - Return a new `TachyonResponse` to replace the original.
 * - Return `void` to keep the original response unchanged (useful for logging).
 */
export type OnResponseHook = (req: TachyonRequest, res: TachyonResponse) => TachyonResponse | void

export type Plugin = {
  pre?: OnRequestHook,
  pos?: OnResponseHook,
}

class Tachyon {

  private routes: Map<string, (req: TachyonRequest) => TachyonResponse>;
  private plugins: Plugin[] = []
  private config: TachyonConfig;

  constructor(config?: TachyonConfig) {
    this.routes = new Map();
    this.config = config ?? {};
  }

  public use(plugin: Plugin) {
    this.plugins.push(plugin)
    return this
  }

  private transformToResponse(response: ((req: TachyonRequest) => TachyonResponse) | string | Record<string, unknown> | Array<Record<string, unknown>>) {
    return typeof response === "function" ? response : () => status(200, response)
  }

  public get(path: string, response: ((req: TachyonRequest) => TachyonResponse) | string | Record<string, unknown>) {
    this.routes.set('0@'+path, this.transformToResponse(response))
    return this
  }

  public post(path: string, response: ((req: TachyonRequest) => TachyonResponse) | string | Record<string, unknown>) {
    this.routes.set('1@'+path, this.transformToResponse(response))
    return this
  }

  public put(path: string, response: ((req: TachyonRequest) => TachyonResponse) | string | Record<string, unknown>) {
    this.routes.set('2@'+path, this.transformToResponse(response))
    return this
  }

  public delete(path: string, response: ((req: TachyonRequest) => TachyonResponse) | string | Record<string, unknown>) {
    this.routes.set('3@'+path, this.transformToResponse(response))
    return this
  }

  public listen(port: number) {
    const server = new TachyonRawServer({
      bindAddr: '0.0.0.0:' + port,
      security: this.config.security ?? 'basic',
      compressionThreshold: this.config.compressionThreshold,
      catchPanics: this.config.catchPanics,
    })

    const plugins = this.plugins

    // Register each route individually — Rust dispatches with O(1) HashMap lookup.
    // Unknown paths return 404 entirely in Rust, zero JS call overhead.
    for (const [key, handler] of this.routes) {
      const atIdx = key.indexOf('@')
      const method = parseInt(key.slice(0, atIdx))
      const path = key.slice(atIdx + 1)

      server.route(methods[method] ?? 'GET', path, (raw) => {
        const req = new TachyonRequest(raw)

        // --- Pre-request hooks ---
        for (const plugin of plugins) {
          const result = plugin.pre?.(req)
          if (result) return result.toRaw()
        }

        // --- Route handler ---
        let res = handler(req)

        // --- Post-response hooks ---
        for (const plugin of plugins) {
          const result = plugin.pos?.(req, res)
          if (result) res = result
        }

        return typeof (res as any).toRaw === 'function' ? (res as any).toRaw() : res as any
      })
    }

    server.listen()
  }

}

export { Tachyon }
