import { DenoAdapter } from './adapters/deno.ts';
import type {
  RouteCallback,
  TachyonContext,
  ResponseFunction,
  HTTPMethod
} from './types.ts';

export class Tachyon {
  private adapter: DenoAdapter;

  constructor() {
    this.adapter = new DenoAdapter();
  }

  private createResponseFn(): ResponseFunction {
    return (data: any, status: number = 200) => {
      return { data, status };
    };
  }

  /**
   * Register a GET route
   */
  get(path: string, callback: RouteCallback): this {
    this.adapter.registerRoute('GET', path, callback);
    return this;
  }

  /**
   * Register a POST route
   */
  post(path: string, callback: RouteCallback): this {
    this.adapter.registerRoute('POST', path, callback);
    return this;
  }

  /**
   * Register a PUT route
   */
  put(path: string, callback: RouteCallback): this {
    this.adapter.registerRoute('PUT', path, callback);
    return this;
  }

  /**
   * Register a DELETE route
   */
  delete(path: string, callback: RouteCallback): this {
    this.adapter.registerRoute('DELETE', path, callback);
    return this;
  }

  /**
   * Register a PATCH route
   */
  patch(path: string, callback: RouteCallback): this {
    this.adapter.registerRoute('PATCH', path, callback);
    return this;
  }

  /**
   * Register a HEAD route
   */
  head(path: string, callback: RouteCallback): this {
    this.adapter.registerRoute('HEAD', path, callback);
    return this;
  }

  /**
   * Register an OPTIONS route
   */
  options(path: string, callback: RouteCallback): this {
    this.adapter.registerRoute('OPTIONS', path, callback);
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
    this.adapter.close();
  }
}

/**
 * Factory function to create a new Tachyon instance for Deno
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
  ResponseFunction
} from './types.ts';
