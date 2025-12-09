export type ResponseFunction = (data: any, status?: number) => TachyonResponse;

export interface TachyonContext {
  body?: any;
  params?: Record<string, string>;
  query?: Record<string, string>;
  headers?: Record<string, string>;
  response: ResponseFunction;
}

export interface TachyonResponse {
  data: any;
  status: number;
}

export type RouteCallback = (
  ctx: TachyonContext,
) => TachyonResponse | Promise<TachyonResponse>;

export interface ITachyonAdapter {
  registerRoute(method: string, path: string, callback: RouteCallback): void;
  listen(port: number): void;
  close?(): void;
}

export interface ITachyonNative {
  listen(port: number): string;
}

export type HTTPMethod =
  | "GET"
  | "POST"
  | "PUT"
  | "DELETE"
  | "PATCH"
  | "HEAD"
  | "OPTIONS";
