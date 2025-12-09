// Comprehensive benchmark: Tachyon vs Fastify vs Elysia
// Run with: node examples/benchmark.js

import { tachyon } from "../dist/index.js";
import { spawn, execSync } from "child_process";
import http from "http";

const DURATION = 10; // seconds
const CONNECTIONS = 100;
const PIPELINING = 10;
const PORT_TACHYON = 3001;
const PORT_FASTIFY = 3002;
const PORT_ELYSIA = 3003;

// Colors for output
const colors = {
  reset: "\x1b[0m",
  bright: "\x1b[1m",
  green: "\x1b[32m",
  yellow: "\x1b[33m",
  blue: "\x1b[34m",
  cyan: "\x1b[36m",
  red: "\x1b[31m",
};

function log(color, ...args) {
  console.log(color, ...args, colors.reset);
}

// Check if autocannon is available
function checkAutocannon() {
  try {
    execSync("which autocannon", { stdio: "pipe" });
    return true;
  } catch {
    return false;
  }
}

// Run autocannon benchmark
function runBenchmark(name, port) {
  return new Promise((resolve, reject) => {
    log(colors.cyan, `\n🚀 Benchmarking ${name} on port ${port}...`);

    const autocannon = spawn("autocannon", [
      "-c",
      CONNECTIONS.toString(),
      "-d",
      DURATION.toString(),
      "-p",
      PIPELINING.toString(),
      "-j", // JSON output
      `http://localhost:${port}/`,
    ]);

    let output = "";
    let errorOutput = "";

    autocannon.stdout.on("data", (data) => {
      output += data.toString();
    });

    autocannon.stderr.on("data", (data) => {
      errorOutput += data.toString();
    });

    autocannon.on("close", (code) => {
      if (code !== 0) {
        reject(new Error(`Benchmark failed: ${errorOutput}`));
        return;
      }
      try {
        const result = JSON.parse(output);
        resolve({
          name,
          port,
          requests: result.requests.total,
          throughput: result.throughput.total,
          latency: {
            avg: result.latency.average,
            p50: result.latency.p50,
            p90: result.latency.p90,
            p99: result.latency.p99,
            max: result.latency.max,
          },
          rps: result.requests.average,
          errors: result.errors,
        });
      } catch (e) {
        reject(new Error(`Failed to parse results: ${e.message}`));
      }
    });
  });
}

// Simple HTTP benchmark without autocannon
function runSimpleBenchmark(name, port) {
  return new Promise((resolve) => {
    log(
      colors.cyan,
      `\n🚀 Running simple benchmark for ${name} on port ${port}...`,
    );

    const requests = 10000;
    let completed = 0;
    let errors = 0;
    const start = process.hrtime.bigint();
    const latencies = [];

    const makeRequest = () => {
      const reqStart = process.hrtime.bigint();

      const req = http.get(`http://localhost:${port}/`, (res) => {
        let data = "";
        res.on("data", (chunk) => (data += chunk));
        res.on("end", () => {
          const reqEnd = process.hrtime.bigint();
          latencies.push(Number(reqEnd - reqStart) / 1_000_000); // ms
          completed++;
          if (completed < requests) {
            setImmediate(makeRequest);
          } else {
            finish();
          }
        });
      });

      req.on("error", () => {
        errors++;
        completed++;
        if (completed < requests) {
          setImmediate(makeRequest);
        } else {
          finish();
        }
      });

      req.end();
    };

    const finish = () => {
      const end = process.hrtime.bigint();
      const duration = Number(end - start) / 1_000_000_000; // seconds

      latencies.sort((a, b) => a - b);

      resolve({
        name,
        port,
        requests: completed,
        errors,
        duration: duration.toFixed(2),
        rps: Math.round(completed / duration),
        latency: {
          avg: (
            latencies.reduce((a, b) => a + b, 0) / latencies.length
          ).toFixed(2),
          p50:
            latencies[Math.floor(latencies.length * 0.5)]?.toFixed(2) || "N/A",
          p90:
            latencies[Math.floor(latencies.length * 0.9)]?.toFixed(2) || "N/A",
          p99:
            latencies[Math.floor(latencies.length * 0.99)]?.toFixed(2) || "N/A",
          max: Math.max(...latencies).toFixed(2),
        },
      });
    };

    // Start concurrent requests
    const concurrency = 50;
    for (let i = 0; i < concurrency; i++) {
      makeRequest();
    }
  });
}

// Start Tachyon server
function startTachyon() {
  return new Promise((resolve) => {
    const app = tachyon();

    app.get("/", ({ response }) => {
      return response({ message: "Hello, World!" });
    });

    app.get("/json", ({ response }) => {
      return response({
        id: 1,
        name: "test",
        timestamp: Date.now(),
        data: { nested: true, values: [1, 2, 3] },
      });
    });

    app.listen(PORT_TACHYON, () => {
      log(colors.green, `✓ Tachyon started on port ${PORT_TACHYON}`);
      setTimeout(resolve, 500);
    });
  });
}

// Start Fastify server (if available)
async function startFastify() {
  return new Promise(async (resolve, reject) => {
    try {
      const { default: Fastify } = await import("fastify");
      const fastify = Fastify({ logger: false });

      fastify.get("/", async () => {
        return { message: "Hello, World!" };
      });

      fastify.get("/json", async () => {
        return {
          id: 1,
          name: "test",
          timestamp: Date.now(),
          data: { nested: true, values: [1, 2, 3] },
        };
      });

      await fastify.listen({ port: PORT_FASTIFY });
      log(colors.green, `✓ Fastify started on port ${PORT_FASTIFY}`);
      setTimeout(() => resolve(fastify), 500);
    } catch (e) {
      log(colors.yellow, "⚠ Fastify not available, skipping...");
      resolve(null);
    }
  });
}

// Print results table
function printResults(results) {
  log(colors.bright, "\n" + "=".repeat(80));
  log(colors.bright, "📊 BENCHMARK RESULTS");
  log(colors.bright, "=".repeat(80));

  // Sort by RPS descending
  results.sort((a, b) => b.rps - a.rps);

  console.log(
    "\n┌─────────────┬───────────────┬──────────────┬──────────────┬──────────────┐",
  );
  console.log(
    "│ Framework   │ Requests/sec  │ Latency avg  │ Latency p99  │ Errors       │",
  );
  console.log(
    "├─────────────┼───────────────┼──────────────┼──────────────┼──────────────┤",
  );

  results.forEach((r, i) => {
    const medal = i === 0 ? "🥇" : i === 1 ? "🥈" : i === 2 ? "🥉" : "  ";
    const name = (r.name + "    ").slice(0, 9);
    const rps = (r.rps.toLocaleString() + "        ").slice(0, 11);
    const avgLat = ((r.latency.avg || "N/A") + " ms       ").slice(0, 10);
    const p99Lat = ((r.latency.p99 || "N/A") + " ms       ").slice(0, 10);
    const errors = ((r.errors || 0) + "          ").slice(0, 10);

    console.log(
      `│ ${medal} ${name} │ ${rps}   │ ${avgLat}   │ ${p99Lat}   │ ${errors}   │`,
    );
  });

  console.log(
    "└─────────────┴───────────────┴──────────────┴──────────────┴──────────────┘",
  );

  if (results.length >= 2) {
    const fastest = results[0];
    const second = results[1];
    const speedup = ((fastest.rps / second.rps - 1) * 100).toFixed(1);

    log(
      colors.bright,
      `\n🏆 ${fastest.name} is ${speedup}% faster than ${second.name}!`,
    );
  }

  // Detailed latency breakdown
  log(colors.bright, "\n📈 LATENCY BREAKDOWN (ms)");
  console.log(
    "┌─────────────┬──────────┬──────────┬──────────┬──────────┬──────────┐",
  );
  console.log(
    "│ Framework   │ Average  │ p50      │ p90      │ p99      │ Max      │",
  );
  console.log(
    "├─────────────┼──────────┼──────────┼──────────┼──────────┼──────────┤",
  );

  results.forEach((r) => {
    const name = (r.name + "    ").slice(0, 9);
    const avg = ((r.latency.avg || "N/A") + "     ").slice(0, 6);
    const p50 = ((r.latency.p50 || "N/A") + "     ").slice(0, 6);
    const p90 = ((r.latency.p90 || "N/A") + "     ").slice(0, 6);
    const p99 = ((r.latency.p99 || "N/A") + "     ").slice(0, 6);
    const max = ((r.latency.max || "N/A") + "     ").slice(0, 6);

    console.log(
      `│   ${name} │ ${avg}   │ ${p50}   │ ${p90}   │ ${p99}   │ ${max}   │`,
    );
  });

  console.log(
    "└─────────────┴──────────┴──────────┴──────────┴──────────┴──────────┘",
  );
}

// Main
async function main() {
  log(colors.bright, "\n" + "═".repeat(60));
  log(colors.bright, "🔥 TACHYON BENCHMARK - High Performance HTTP Framework");
  log(colors.bright, "═".repeat(60));

  const hasAutocannon = checkAutocannon();
  if (!hasAutocannon) {
    log(colors.yellow, "\n⚠ autocannon not found. Using simple benchmark.");
    log(
      colors.yellow,
      "  For better results, install: npm install -g autocannon\n",
    );
  }

  const benchmarkFn = hasAutocannon ? runBenchmark : runSimpleBenchmark;
  const results = [];

  // Start servers
  log(colors.blue, "\n📦 Starting servers...");

  await startTachyon();
  const fastify = await startFastify();

  // Wait for servers to be ready
  await new Promise((r) => setTimeout(r, 1000));

  // Run benchmarks
  log(colors.blue, "\n🏁 Running benchmarks...");
  log(
    colors.blue,
    `   Duration: ${hasAutocannon ? DURATION : "~10"}s per framework`,
  );
  log(colors.blue, `   Connections: ${hasAutocannon ? CONNECTIONS : 50}`);

  try {
    // Benchmark Tachyon
    const tachyonResult = await benchmarkFn("Tachyon", PORT_TACHYON);
    results.push(tachyonResult);

    // Benchmark Fastify
    if (fastify) {
      const fastifyResult = await benchmarkFn("Fastify", PORT_FASTIFY);
      results.push(fastifyResult);
    }

    // Print results
    printResults(results);
  } catch (error) {
    log(colors.red, `\n❌ Benchmark error: ${error.message}`);
  }

  // Cleanup
  log(colors.blue, "\n🧹 Cleaning up...");

  if (fastify) {
    await fastify.close();
  }

  log(colors.green, "\n✅ Benchmark complete!\n");
  process.exit(0);
}

main().catch(console.error);
