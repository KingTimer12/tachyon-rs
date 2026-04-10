import type { TachyonRawRequest } from "@tachyon-rs/server";

class TachyonRequest {
  method: string
  path: string
  body: string | undefined
  private _headersRaw: string
  private _headers: Map<string, string> | undefined

  constructor(raw: TachyonRawRequest) {
    this.method = raw.method
    this.path = raw.path
    this.body = raw.body
    this._headersRaw = raw.headers
  }

  header(name: string): string | undefined {
    if (!this._headers) this._parseHeaders()
    return this._headers!.get(name.toLowerCase())
  }

  get headers(): ReadonlyMap<string, string> {
    if (!this._headers) this._parseHeaders()
    return this._headers!
  }

  private _parseHeaders() {
    this._headers = new Map()
    const raw = this._headersRaw
    let i = 0
    while (i < raw.length) {
      const tab = raw.indexOf('\t', i)
      if (tab === -1) break
      const nl = raw.indexOf('\n', tab + 1)
      if (nl === -1) break
      const name = raw.slice(i, tab).toLowerCase()
      const value = raw.slice(tab + 1, nl)
      this._headers.set(name, value)
      i = nl + 1
    }
  }
}

export { TachyonRequest }
