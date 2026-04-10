export const PORT = 3333;

export interface User {
  id: number;
  name: string;
  email: string;
  age: number;
  active: boolean;
}

export interface LargeRecord {
  id: number;
  title: string;
  score: number;
  active: boolean;
  tags: string[];
  meta: { created: string; views: number; rating: number };
}

export function makeUsers(count: number): User[] {
  return Array.from({ length: count }, (_, i) => ({
    id: i + 1,
    name: `User ${i + 1}`,
    email: `user${i + 1}@example.com`,
    age: 20 + (i % 50),
    active: i % 2 === 0,
  }));
}

export function makeLargePayload(count: number): LargeRecord[] {
  return Array.from({ length: count }, (_, i) => ({
    id: i + 1,
    title: `Record ${i + 1}`,
    score: Math.round((Math.random() * 100) * 100) / 100,
    active: i % 3 !== 0,
    tags: [`tag-${i % 5}`, `category-${i % 10}`],
    meta: {
      created: "2024-01-01T00:00:00Z",
      views: i * 42,
      rating: 1 + (i % 5),
    },
  }));
}

export const SCENARIOS: Array<{ name: string; method: "GET" | "POST"; path: string; body?: string }> = [
  { name: "Plaintext",   method: "GET",  path: "/text" },
  { name: "JSON tiny",   method: "GET",  path: "/json" },
  { name: "JSON users",  method: "GET",  path: "/users" },
  { name: "JSON large",  method: "GET",  path: "/large" },
  { name: "POST echo",   method: "POST", path: "/echo",
    body: JSON.stringify({ hello: "world", value: 42, nested: { ok: true } }) },
];
