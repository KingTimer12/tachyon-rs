import type { TachyonRawRequest } from "@tachyon-rs/server";

class TachyonRequest {
  method: string
  path: string
  body: string | undefined
  private _headers: Map<string, string>

  constructor(raw: TachyonRawRequest) {
    this.method = raw.method
    this.path = raw.path
    this.body = raw.body
    this._headers = new Map(
      raw.headers.map(h => [h.name.toLowerCase(), h.value])
    )
  }

  header(name: string): string | undefined {
    return this._headers.get(name.toLowerCase())
  }

  get headers(): ReadonlyMap<string, string> {
    return this._headers
  }
}

export { TachyonRequest }
