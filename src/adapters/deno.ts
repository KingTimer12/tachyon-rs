import { join, dirname, fromFileUrl } from "https://deno.land/std@0.208.0/path/mod.ts";
import type { ITachyonAdapter, RouteCallback, ResponseFunction } from '../types.ts';

// Símbolos FFI para a biblioteca Rust
const symbols = {
  tachyon_new: { parameters: [], result: "pointer" },
  tachyon_listen: { parameters: ["pointer", "u16"], result: "pointer" },
  tachyon_free: { parameters: ["pointer"], result: "void" },
  tachyon_free_string: { parameters: ["pointer"], result: "void" },
} as const;

export class DenoAdapter implements ITachyonAdapter {
  private lib: Deno.DynamicLibrary<typeof symbols>;
  private handle: Deno.PointerValue;
  private routes: Map<string, RouteCallback>;

  constructor() {
    const __dirname = dirname(fromFileUrl(import.meta.url));

    // Detecta a plataforma e carrega a biblioteca apropriada
    const libExt = Deno.build.os === "windows" ? "dll" :
                   Deno.build.os === "darwin" ? "dylib" : "so";
    const libPath = join(__dirname, `../../target/release/libtachyon_raw.${libExt}`);

    try {
      this.lib = Deno.dlopen(libPath, symbols);
      this.handle = this.lib.symbols.tachyon_new();

      if (!this.handle) {
        throw new Error("Failed to create Tachyon instance");
      }
    } catch (error) {
      throw new Error(`Failed to load FFI library for Deno: ${error}`);
    }

    this.routes = new Map();
  }

  private createResponseFn(): ResponseFunction {
    return (data: any, status: number = 200) => {
      return { data, status };
    };
  }

  registerRoute(method: string, path: string, callback: RouteCallback): void {
    const routeKey = `${method}:${path}`;
    this.routes.set(routeKey, callback);
  }

  listen(port: number): void {
    if (!this.handle) {
      throw new Error("Tachyon instance not initialized");
    }

    const resultPtr = this.lib.symbols.tachyon_listen(this.handle, port);

    if (resultPtr) {
      const view = new Deno.UnsafePointerView(resultPtr);
      const result = view.getCString();
      console.log(result);
      this.lib.symbols.tachyon_free_string(resultPtr);
    }

    console.log(`[Deno] Registered ${this.routes.size} routes`);
  }

  close(): void {
    if (this.handle) {
      this.lib.symbols.tachyon_free(this.handle);
      this.handle = null;
    }
    this.routes.clear();
    this.lib.close();
  }
}
