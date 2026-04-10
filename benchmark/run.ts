/**
 * Tachyon Benchmark Runner
 *
 * Compares Express, Fastify, Elysia, and Tachyon across 5 scenarios:
 *   1. Plaintext response
 *   2. JSON tiny object
 *   3. JSON array of 10 users
 *   4. JSON large payload (100 records)
 *   5. POST echo
 *
 * Uses autocannon for HTTP load generation and pidusage for CPU/memory sampling.
 *
 * Usage:
 *   bun run run.ts [--duration 10] [--connections 100] [--skip-tachyon]
 */

import { spawn, type ChildProcess } from "child_process";
import autocannon from "autocannon";
import pidusage from "pidusage";
import { PORT, SCENARIOS } from "./shared.ts";

// ─── CLI args ────────────────────────────────────────────────────────────────

const args = process.argv.slice(2);
const getArg = (flag: string, fallback: number) => {
  const i = args.indexOf(flag);
  return i !== -1 ? parseInt(args[i + 1]) : fallback;
};
const DURATION    = getArg("--duration", 10);      // seconds per scenario
const CONNECTIONS = getArg("--connections", 100);   // concurrent connections
const WARMUP_SECS = 3;                              // warmup before each run
const SKIP_TACHYON = args.includes("--skip-tachyon");

// ─── Framework definitions ───────────────────────────────────────────────────

interface Framework {
  name: string;
  runtime: "bun" | "node";
  file: string;
  color: string;
}

const FRAMEWORKS: Framework[] = [
  { name: "Tachyon",  runtime: "bun", file: "servers/tachyon.ts",  color: "\x1b[36m" },
  { name: "Express",  runtime: "bun",  file: "servers/express.ts",  color: "\x1b[33m" },
  { name: "Fastify",  runtime: "bun",  file: "servers/fastify.ts",  color: "\x1b[34m" },
  { name: "Elysia",   runtime: "bun",  file: "servers/elysia.ts",   color: "\x1b[35m" },
];

const RESET = "\x1b[0m";
const BOLD  = "\x1b[1m";
const GREEN = "\x1b[32m";
const DIM   = "\x1b[2m";

// ─── Types ───────────────────────────────────────────────────────────────────

interface ScenarioResult {
  reqPerSec: number;
  latencyAvg: number;
  latencyP99: number;
  throughputMBps: number;
  errors: number;
  memoryPeakMB: number;
  cpuPeak: number;
}

type Results = Record<string, Record<string, ScenarioResult>>;

// ─── Server lifecycle ────────────────────────────────────────────────────────

function startServer(fw: Framework): Promise<ChildProcess> {
  return new Promise((resolve, reject) => {
    const cmd   = fw.runtime === "bun" ? "bun" : "node";
    const flags = fw.runtime === "node"
      ? ["--experimental-vm-modules", "--input-type=module"]
      : [];

    const child = spawn(cmd, [...flags, fw.file], {
      cwd: import.meta.dir,
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, NODE_ENV: "production" },
    });

    let ready = false;
    const timeout = setTimeout(() => {
      if (!ready) reject(new Error(`${fw.name} did not start within 10s`));
    }, 10_000);

    child.stdout?.on("data", (data: Buffer) => {
      if (!ready && data.toString().includes("ready")) {
        ready = true;
        clearTimeout(timeout);
        resolve(child);
      }
    });

    child.stderr?.on("data", (data: Buffer) => {
      const msg = data.toString().trim();
      if (msg) process.stderr.write(`  ${DIM}[${fw.name}] ${msg}${RESET}\n`);
    });

    child.on("error", reject);
  });
}

function killServer(child: ChildProcess): Promise<void> {
  return new Promise((resolve) => {
    child.on("exit", () => resolve());
    child.kill("SIGTERM");
    setTimeout(() => { child.kill("SIGKILL"); resolve(); }, 2000);
  });
}

/** Kill any leftover process occupying the benchmark port. */
async function freePort(): Promise<void> {
  try {
    const result = Bun.spawnSync(["lsof", "-ti", `:${PORT}`]);
    const pids = result.stdout.toString().trim().split("\n").filter(Boolean);
    for (const pid of pids) {
      try { process.kill(parseInt(pid), "SIGKILL"); } catch { /* already gone */ }
    }
    if (pids.length > 0) await Bun.sleep(300);
  } catch { /* lsof not available */ }
}

async function waitReady(maxMs = 5000): Promise<boolean> {
  const deadline = Date.now() + maxMs;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`http://127.0.0.1:${PORT}/json`);
      if (res.ok) return true;
    } catch { /* not ready yet */ }
    await Bun.sleep(100);
  }
  return false;
}

// ─── Benchmarking ────────────────────────────────────────────────────────────

async function runScenario(
  scenario: typeof SCENARIOS[number],
  pid: number,
  duration: number,
): Promise<ScenarioResult> {
  const url = `http://127.0.0.1:${PORT}${scenario.path}`;
  const isPost = scenario.method === "POST";

  // Sample CPU/memory in parallel with autocannon
  const samples: { cpu: number; mem: number }[] = [];
  const sampler = setInterval(async () => {
    try {
      const s = await pidusage(pid);
      samples.push({ cpu: s.cpu, mem: s.memory / 1024 / 1024 });
    } catch { /* process may be gone */ }
  }, 200);

  const result = await autocannon({
    url,
    method: isPost ? "POST" : "GET",
    body: isPost ? scenario.body : undefined,
    headers: isPost ? { "content-type": "application/json" } : undefined,
    connections: CONNECTIONS,
    duration,
    pipelining: 1,
  });

  clearInterval(sampler);

  const memPeak  = samples.length ? Math.max(...samples.map(s => s.mem))  : 0;
  const cpuPeak  = samples.length ? Math.max(...samples.map(s => s.cpu))  : 0;

  return {
    reqPerSec:       result.requests.average,
    latencyAvg:      result.latency.average,
    latencyP99:      result.latency.p99,
    throughputMBps:  result.throughput.average / 1024 / 1024,
    errors:          result.errors,
    memoryPeakMB:    Math.round(memPeak * 10) / 10,
    cpuPeak:         Math.round(cpuPeak * 10) / 10,
  };
}

// ─── Output ───────────────────────────────────────────────────────────────────

function fmt(n: number, decimals = 0): string {
  return n.toLocaleString("en-US", { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
}

function pad(s: string, len: number, right = false): string {
  return right ? s.padStart(len) : s.padEnd(len);
}

function printScenarioTable(scenario: string, results: Results) {
  const fw = Object.keys(results);
  const W = { name: 10, rps: 12, latAvg: 10, latP99: 10, mb: 10, mem: 12, cpu: 8 };
  const header = [
    pad("Framework",  W.name),
    pad("Req/s",      W.rps,   true),
    pad("Lat avg",    W.latAvg, true),
    pad("Lat p99",    W.latP99, true),
    pad("MB/s",       W.mb,    true),
    pad("Mem peak",   W.mem,   true),
    pad("CPU peak",   W.cpu,   true),
  ].join("  ");

  const line = "─".repeat(header.length);
  console.log(`\n${BOLD}${GREEN}▶ ${scenario}${RESET}`);
  console.log(`${DIM}${line}${RESET}`);
  console.log(`${BOLD}${header}${RESET}`);
  console.log(`${DIM}${line}${RESET}`);

  // Sort by req/s descending
  const sorted = fw
    .filter(f => results[f][scenario])
    .sort((a, b) => results[b][scenario].reqPerSec - results[a][scenario].reqPerSec);

  const best = results[sorted[0]]?.[scenario]?.reqPerSec ?? 1;

  for (const name of sorted) {
    const r = results[name][scenario];
    const color = FRAMEWORKS.find(f => f.name === name)?.color ?? "";
    const ratio = r.reqPerSec / best;
    const bar   = "█".repeat(Math.round(ratio * 10));
    const errStr = r.errors > 0 ? ` ${DIM}(${r.errors} err)${RESET}` : "";

    console.log([
      `${color}${BOLD}${pad(name, W.name)}${RESET}`,
      pad(fmt(r.reqPerSec), W.rps, true),
      pad(`${fmt(r.latencyAvg, 2)} ms`, W.latAvg, true),
      pad(`${fmt(r.latencyP99, 2)} ms`, W.latP99, true),
      pad(`${fmt(r.throughputMBps, 2)}`, W.mb, true),
      pad(`${fmt(r.memoryPeakMB, 1)} MB`, W.mem, true),
      pad(`${fmt(r.cpuPeak, 1)}%`, W.cpu, true),
      `  ${DIM}${bar}${RESET}${errStr}`,
    ].join("  "));
  }

  console.log(`${DIM}${line}${RESET}`);
}

function printSummary(results: Results) {
  console.log(`\n${BOLD}${GREEN}═══ SUMMARY — Best req/s per scenario ═══${RESET}\n`);

  const fwNames = Object.keys(results);
  const scores: Record<string, number> = Object.fromEntries(fwNames.map(n => [n, 0]));

  for (const scenario of SCENARIOS) {
    const sorted = fwNames
      .filter(f => results[f][scenario.name])
      .sort((a, b) => results[b][scenario.name].reqPerSec - results[a][scenario.name].reqPerSec);

    if (sorted.length > 0) {
      scores[sorted[0]]++;
      const winner = sorted[0];
      const color  = FRAMEWORKS.find(f => f.name === winner)?.color ?? "";
      const rps    = results[winner][scenario.name].reqPerSec;
      console.log(`  ${pad(scenario.name, 14)} ${color}${BOLD}${pad(winner, 10)}${RESET}  ${fmt(rps)} req/s`);
    }
  }

  console.log(`\n${BOLD}  Wins:${RESET}`);
  for (const [fw, wins] of Object.entries(scores).sort((a, b) => b[1] - a[1])) {
    const color = FRAMEWORKS.find(f => f.name === fw)?.color ?? "";
    console.log(`  ${color}${BOLD}${pad(fw, 10)}${RESET}  ${"🏆".repeat(wins)} ${wins}/${SCENARIOS.length}`);
  }
}

// ─── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  const fwToRun = SKIP_TACHYON
    ? FRAMEWORKS.filter(f => f.name !== "Tachyon")
    : FRAMEWORKS;

  console.log(`\n${BOLD}${GREEN}╔══════════════════════════════════════╗${RESET}`);
  console.log(`${BOLD}${GREEN}║     Tachyon Benchmark Suite          ║${RESET}`);
  console.log(`${BOLD}${GREEN}╚══════════════════════════════════════╝${RESET}`);
  console.log(`\n  Duration:    ${DURATION}s per scenario`);
  console.log(`  Connections: ${CONNECTIONS} concurrent`);
  console.log(`  Warmup:      ${WARMUP_SECS}s`);
  console.log(`  Frameworks:  ${fwToRun.map(f => f.name).join(", ")}\n`);

  const results: Results = Object.fromEntries(fwToRun.map(f => [f.name, {}]));

  for (const fw of fwToRun) {
    console.log(`\n${BOLD}▷ Starting ${fw.name} (${fw.runtime})...${RESET}`);

    await freePort();

    let child: ChildProcess;
    try {
      child = await startServer(fw);
    } catch (e) {
      console.error(`  ✗ Failed to start ${fw.name}: ${e}`);
      continue;
    }

    const pid = child.pid!;
    const ready = await waitReady();
    if (!ready) {
      console.error(`  ✗ ${fw.name} not responding on port ${PORT}`);
      await killServer(child);
      await Bun.sleep(500);
      continue;
    }

    console.log(`  ✓ ${fw.name} ready (pid ${pid})`);

    // Warmup — one pass through all endpoints
    console.log(`  ↺ Warming up (${WARMUP_SECS}s)...`);
    for (const sc of SCENARIOS) {
      await autocannon({
        url: `http://127.0.0.1:${PORT}${sc.path}`,
        method: sc.method === "POST" ? "POST" : "GET",
        body: sc.method === "POST" ? sc.body : undefined,
        headers: sc.method === "POST" ? { "content-type": "application/json" } : undefined,
        connections: Math.min(CONNECTIONS, 50),
        duration: Math.ceil(WARMUP_SECS / SCENARIOS.length),
      });
    }

    for (const scenario of SCENARIOS) {
      process.stdout.write(`  ⟳ ${pad(scenario.name, 14)} `);
      const r = await runScenario(scenario, pid, DURATION);
      results[fw.name][scenario.name] = r;
      process.stdout.write(`${fmt(r.reqPerSec)} req/s\n`);
    }

    await killServer(child);
    await Bun.sleep(500); // let port free up
  }

  // Print per-scenario tables
  for (const scenario of SCENARIOS) {
    printScenarioTable(scenario.name, results);
  }

  // Print summary
  printSummary(results);

  console.log(`\n${DIM}All servers benchmarked on 127.0.0.1:${PORT} — ${new Date().toISOString()}${RESET}\n`);
}

main().catch(console.error);
