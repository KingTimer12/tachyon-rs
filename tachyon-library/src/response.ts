class TachyonResponse {
  private headers: { name: string; value: string }[] = []
  private _contentType: 'json' | 'text' = 'json'

  constructor(public status: number, public body: string) { }

  header(name: string, value: string) {
    this.headers.push({ name, value })
    return this
  }

  /** Set response content type to plain text. */
  text() {
    this._contentType = 'text'
    return this
  }

  json() {
    return {
      status: this.status,
      body: this.body,
      contentType: this._contentType,
      headers: this.headers.length > 0 ? this.headers : undefined,
    }
  }
}

export { TachyonResponse }
