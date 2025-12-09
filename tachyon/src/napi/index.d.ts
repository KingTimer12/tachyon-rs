export declare class Tachyon {
  constructor()
  /** Register a route with optimized callback handling */
  registerRoute(method: string, path: string, callback: (err: Error | null, data: JsCallbackData) => JsCallbackResult): void
  /** Start the server */
  listen(port: number): Promise<void>
  /** Get callback statistics for profiling */
  getStats(): string
}

/** Data passed to JS callback - optimized structure */
export interface JsCallbackData {
  body?: string
  params?: string
}

/** Result from JS callback */
export interface JsCallbackResult {
  data: string
  status: number
}
