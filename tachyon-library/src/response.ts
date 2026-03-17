interface RustJsonField {
  key?: string
  value?: string
  valueType?: string
  children?: RustJsonField[]
}

function convertValue(value: unknown): RustJsonField {
  if (value === null || value === undefined) {
    return { value: 'null', valueType: 'null' }
  }
  switch (typeof value) {
    case 'string':
      return { value }
    case 'number':
      return { value: String(value), valueType: 'number' }
    case 'boolean':
      return { value: String(value), valueType: 'bool' }
    case 'object':
      if (Array.isArray(value)) {
        return {
          valueType: 'array',
          children: value.map(item => convertValue(item)),
        }
      }
      return {
        valueType: 'object',
        children: Object.entries(value as Record<string, unknown>).map(
          ([k, v]) => ({ key: k, ...convertValue(v) })
        ),
      }
    default:
      return { value: String(value) }
  }
}

class TachyonResponse {
  private headers: { name: string; value: string }[] = []
  private _contentType: 'json' | 'text' = 'json'

  constructor(public status: number, public body: string | Record<string, unknown> | Array<Record<string, unknown>>) { }

  header(name: string, value: string) {
    this.headers.push({ name, value })
    return this
  }

  /** Set response content type to plain text. */
  text() {
    this._contentType = 'text'
    return this
  }

  private convertToRustJson(): { json?: RustJsonField[]; array?: RustJsonField[] } | undefined {
    if (typeof this.body !== 'object' || this.body === null) return undefined

    if (Array.isArray(this.body)) {
      return {
        array: this.body.map(item => convertValue(item)),
      }
    }

    return {
      json: Object.entries(this.body).map(([key, value]) => ({
        key,
        ...convertValue(value),
      })),
    }
  }

  toRaw() {
    const rustJson = this.convertToRustJson()

    return {
      status: this.status,
      body: typeof this.body === 'string' ? this.body : undefined,
      contentType: this._contentType,
      headers: this.headers.length > 0 ? this.headers : undefined,
      json: rustJson?.json,
      array: rustJson?.array,
    }
  }
}

export { TachyonResponse }
