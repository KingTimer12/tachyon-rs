class TachyonResponse {
  private headers: { name: string; value: string }[] = []

  constructor(public status: number, public body: string) { }

  header(name: string, value: string) {
    this.headers.push({ name, value })
    return this
  }

  json() {
    return {
      status: this.status,
      body: this.body,
      headers: this.headers.length > 0 ? this.headers : undefined,
    }
  }
}

export { TachyonResponse }
