use std::{
    io::{Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Instant,
};

use may::{go, net::TcpListener};

use crate::{config::ServerConfig, response::Response, safety, utils::apply_socket_config};

/// The handler function type. Receives a parsed request and a response builder.
/// Returns the number of bytes written to the response buffer.
///
/// This is directly inspired by FaF's callback model:
/// ```ignore
/// // FaF: you get a buffer, fill it, return length
/// fn callback(buf: &mut [u8]) -> usize { ... }
///
/// // tachyon: you get a parsed request + response builder
/// fn handler(req: &Request, res: &mut Response) -> usize { ... }
/// ```
pub type Handler = Arc<dyn Fn(&tachyon_http::http::Request, &mut Response) -> usize + Send + Sync>;

/// The tachyon server.
pub struct Server {
    config: ServerConfig,
}

impl Server {
    pub fn new(config: ServerConfig) -> Self {
        Self { config }
    }

    /// Pre-initialize all lazy systems so the first real request is fast.
    fn warmup_pools(workers: usize) {
        let (tx, rx) = std::sync::mpsc::channel();
        for _ in 0..(workers * 2) {
            let tx = tx.clone();
            go!(move || {
                let _buf = tachyon_pool::pool::acquire();
                let _ = tx.send(());
            });
        }
        drop(tx);
        for _ in rx.iter() {}
    }

    /// Convert a bind address like "0.0.0.0:3000" to "127.0.0.1:3000".
    fn to_loopback(bind_addr: &str) -> String {
        if let Some(pos) = bind_addr.rfind(':') {
            let port = &bind_addr[pos + 1..];
            let host = &bind_addr[..pos];
            if host == "0.0.0.0" || host == "::" || host == "[::]" || host.is_empty() {
                return format!("127.0.0.1:{}", port);
            }
        }
        bind_addr.to_string()
    }

    /// Start the server with the given handler.
    ///
    /// This blocks the current thread. Each incoming connection spawns
    /// a May coroutine (via `go!`), which:
    /// 1. Acquires a buffer from the thread-local pool (FaF pattern)
    /// 2. Reads the request into it
    /// 3. Parses zero-copy (FaF pattern)
    /// 4. Calls your handler
    /// 5. Writes the response
    /// 6. Returns the buffer to the pool (automatic via RAII)
    pub fn run(self, handler: Handler) -> std::io::Result<()> {
        // Configure May's coroutine runtime
        may::config()
            .set_workers(self.config.workers)
            .set_stack_size(self.config.coroutine_stack_size);

        let listener = TcpListener::bind(&self.config.bind_addr)?;

        // Apply socket tuning from config
        apply_socket_config(&listener, &self.config.socket);

        let config = Arc::new(self.config);

        // Phase 1: Warm buffer pools on all worker threads
        let warmup_start = Instant::now();
        Self::warmup_pools(config.workers);
        eprintln!("[tachyon] Pool warmup: {:?}", warmup_start.elapsed());

        // Phase 2: Send multiple warmup requests through the full pipeline.
        // This warms: accept path, IOCP, buffer pools, handler (JS JIT), write path.
        // Multiple iterations trigger V8's TurboFan JIT optimization tiers.
        let loopback_addr = Self::to_loopback(&config.bind_addr);
        let warmup_count: usize = 10;
        let warmup_completed = Arc::new(AtomicUsize::new(0));
        let warmup_ready = Arc::new(AtomicBool::new(false));

        // Spawn a thread that sends warmup requests sequentially.
        // Each request uses Connection: close, so the accept loop handles them one by one.
        let warmup_completed2 = warmup_completed.clone();
        let warmup_ready2 = warmup_ready.clone();
        std::thread::spawn(move || {
            let t = Instant::now();
            for i in 0..warmup_count {
                match std::net::TcpStream::connect(&loopback_addr) {
                    Ok(mut s) => {
                        let _ = s.write_all(
                            b"GET /__tachyon_warmup HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
                        );
                        let mut buf = [0u8; 512];
                        let _ = s.read(&mut buf);
                        warmup_completed2.fetch_add(1, Ordering::Release);
                        if i == 0 {
                            eprintln!("[tachyon] First warmup round-trip: {:?}", t.elapsed());
                        }
                    }
                    Err(e) => {
                        eprintln!("[tachyon] Warmup connect failed: {}", e);
                        break;
                    }
                }
            }
            eprintln!("[tachyon] All {} warmup requests done: {:?}", warmup_count, t.elapsed());
            warmup_ready2.store(true, Ordering::Release);
        });

        // The accept loop processes warmup requests first, then real requests.
        // We track warmup state so we can print "Listening" only after warmup is done.
        let mut warmup_printed = false;

        while let Ok((mut stream, _addr)) = listener.accept() {
            // Print "Listening" once warmup is complete
            if !warmup_printed && warmup_ready.load(Ordering::Acquire) {
                eprintln!(
                    "[tachyon] Listening on {} ({} workers, {}KB stack, {} buffers/worker)",
                    config.bind_addr,
                    config.workers,
                    config.coroutine_stack_size / 1024,
                    config.buffers_per_worker,
                );
                warmup_printed = true;
            }

            let handler = handler.clone();
            let config = config.clone();

            go!(move || {
                let conn_start = Instant::now();

                if config.socket.tcp_nodelay {
                    let _ = stream.set_nodelay(true);
                }

                let mut read_buf = tachyon_pool::pool::acquire();
                let mut write_buf = tachyon_pool::pool::acquire();

                loop {
                    let t0 = Instant::now();
                    let n = match stream.read(read_buf.as_write_buf()) {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(_) => break,
                    };
                    read_buf.set_len(n);
                    let read_time = t0.elapsed();

                    let t1 = Instant::now();
                    let request = match tachyon_http::parser::parse(read_buf.filled()) {
                        tachyon_http::parser::ParseResult::Complete(req) => req,
                        tachyon_http::parser::ParseResult::Incomplete => {
                            continue;
                        }
                        tachyon_http::parser::ParseResult::Error(_) => {
                            let mut res = Response::new(write_buf.as_write_buf());
                            let n = res.text(400, b"Bad Request");
                            let _ = stream.write_all(&write_buf.as_write_buf()[..n]);
                            break;
                        }
                    };
                    let parse_time = t1.elapsed();

                    let is_warmup = request.path_str() == "/__tachyon_warmup";

                    let t2 = Instant::now();
                    let response_len = if config.catch_panics {
                        let result = safety::catch_handler_mut(|| {
                            let mut res = Response::new(write_buf.as_write_buf());
                            let n = handler(&request, &mut res);
                            Ok(n)
                        });
                        match result.into_result() {
                            Ok(n) => n,
                            Err(e) => {
                                eprintln!("[tachyon] Handler failed: {}", e);
                                let mut res = Response::new(write_buf.as_write_buf());
                                res.json(500, b"{\"error\":\"internal\"}")
                            }
                        }
                    } else {
                        let mut res = Response::new(write_buf.as_write_buf());
                        handler(&request, &mut res)
                    };
                    let handler_time = t2.elapsed();

                    let t3 = Instant::now();
                    if stream
                        .write_all(&write_buf.as_write_buf()[..response_len])
                        .is_err()
                    {
                        break;
                    }
                    let write_time = t3.elapsed();

                    // Log timing for warmup or slow requests
                    if is_warmup || conn_start.elapsed().as_millis() > 50 {
                        eprintln!(
                            "[tachyon] {} {} total={:?} read={:?} parse={:?} handler={:?} write={:?}",
                            if is_warmup { "WARMUP" } else { "SLOW" },
                            request.path_str(),
                            conn_start.elapsed(),
                            read_time,
                            parse_time,
                            handler_time,
                            write_time,
                        );
                    }

                    if request.version_minor == 0
                        || request
                            .header(b"connection")
                            .map(|v| v == b"close")
                            .unwrap_or(false)
                    {
                        break;
                    }
                }
            });
        }

        Ok(())
    }
}
