import { TachyonResponse } from "./response"

export const status = <T>(status: number, response: T): TachyonResponse => {
  return new TachyonResponse(status, typeof response === "string" ? response : JSON.stringify(response))
}
