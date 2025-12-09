export type Runtime = 'node' | 'bun' | 'deno' | 'unknown';

export function detectRuntime(): Runtime {
  // @ts-ignore
  if (typeof Deno !== 'undefined') {
    return 'deno';
  }

  // @ts-ignore
  if (typeof Bun !== 'undefined') {
    return 'bun';
  }

  // @ts-ignore
  if (typeof process !== 'undefined' && process.versions && process.versions.node) {
    return 'node';
  }

  return 'unknown';
}
