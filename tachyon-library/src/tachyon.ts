import { TachyonRawServer } from "@tachyon-rs/server";
import { TachyonRequest } from "./request";
import { TachyonResponse } from "./response";

export type SecurityPreset = 'none' | 'basic' | 'strict'

export interface TachyonConfig {
  workers?: number
  security?: SecurityPreset
  /** Minimum body size in bytes to trigger gzip compression. 0 = compress all, -1 = disabled. Default: 1024 */
  compressionThreshold?: number
}

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

  private transformToResponse(response: ((req: TachyonRequest) => TachyonResponse) | string | Record<string, unknown>) {
    if (typeof response === "string") {
      return () => new TachyonResponse(200, response as string)
    } else if (typeof response === "function") {
      return response
    } else {
      return () => new TachyonResponse(200, JSON.stringify(response as Record<string, unknown>))
    }

  }

  public get(path: string, response: ((req: TachyonRequest) => TachyonResponse) | string | Record<string, unknown>) {
    this.routes.set('GET@'+path, this.transformToResponse(response))
    return this
  }

  public post(path: string, response: ((req: TachyonRequest) => TachyonResponse) | string | Record<string, unknown>) {
    this.routes.set('POST@'+path, this.transformToResponse(response))
    return this
  }

  public put(path: string, response: ((req: TachyonRequest) => TachyonResponse) | string | Record<string, unknown>) {
    this.routes.set('PUT@'+path, this.transformToResponse(response))
    return this
  }

  public delete(path: string, response: ((req: TachyonRequest) => TachyonResponse) | string | Record<string, unknown>) {
    this.routes.set('DELETE@'+path, this.transformToResponse(response))
    return this
  }

  public listen(port: number) {
    const server = new TachyonRawServer({
      bindAddr: '0.0.0.0:' + port,
      workers: this.config.workers ?? 4,
      security: this.config.security ?? 'basic',
      compressionThreshold: this.config.compressionThreshold,
    })

    server.start((raw) => {
      const req = new TachyonRequest(raw)

      // --- Pre-request hooks ---
      for (const plugin of this.plugins) {
        const result = plugin.pre?.(req)
        if (result) return result.json()  // short-circuit
      }

      // --- Route handler ---
      const handler = this.routes.get(req.method + '@' + req.path)
      let res = handler
        ? handler(req)
        : new TachyonResponse(404, 'Not Found')

      // --- Post-response hooks ---
      for (const plugin of this.plugins) {
        const result = plugin.pos?.(req, res)
        if (result) res = result  // replace response
      }

      return res.json()
    })
  }

}

export { Tachyon }
