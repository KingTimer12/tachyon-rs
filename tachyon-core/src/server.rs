use std::{
    io::{Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Instant,
};

use may::{go, net::TcpListener};

use crate::{config::ServerConfig, response::Response, rio, safety, utils::apply_socket_config};

/// Write response bytes using RIO zero-copy send if available, else standard write_all.
/// For RIO, loops until all bytes are sent since a single send may be partial.
fn rio_or_write(
    rio_conn: &Option<rio::RioConn>,
    write_buf_id: Option<i64>,
    stream: &mut impl Write,
    data: &[u8],
) -> std::io::Result<()> {
    if let (Some(rio), Some(wb_id)) = (rio_conn, write_buf_id) {
        let mut sent = 0;
        while sent < data.len() {
            let n = rio.send(wb_id, sent as u32, (data.len() - sent) as u32)?;
            if n == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "RIO send returned 0",
                ));
            }
            sent += n;
        }
        Ok(())
    } else {
        stream.write_all(data)
    }
}

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

        // Initialize RIO (Windows Registered I/O) for zero-copy networking.
        // On non-Windows this is a no-op that returns false.
        let use_rio = rio::init();
        if use_rio {
            eprintln!("[tachyon] RIO (Registered I/O) enabled for zero-copy networking");
        }

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
                if config.socket.tcp_nodelay {
                    let _ = stream.set_nodelay(true);
                }

                let mut read_buf = tachyon_pool::pool::acquire();
                let mut write_buf = tachyon_pool::pool::acquire();
                let sec_headers = config.security.as_bytes();

                // Set up RIO zero-copy path if available (Windows only).
                // We get the raw socket handle, create a RIO context, and register
                // both buffers so the kernel can DMA directly into/from them.
                let rio_conn = if use_rio {
                    #[cfg(windows)]
                    let handle = {
                        use std::os::windows::io::AsRawSocket;
                        stream.as_raw_socket() as i64
                    };
                    #[cfg(not(windows))]
                    let handle: i64 = -1;
                    rio::RioConn::new(handle)
                } else {
                    None
                };

                let read_buf_id = if rio_conn.is_some() {
                    rio::register_buffer(read_buf.as_write_buf())
                } else {
                    None
                };
                let write_buf_id = if rio_conn.is_some() {
                    rio::register_buffer(write_buf.as_write_buf())
                } else {
                    None
                };

                loop {
                    // READ: use RIO zero-copy recv when available, else standard read
                    let n = if let (Some(rio), Some(rb_id)) = (&rio_conn, read_buf_id) {
                        match rio.recv(rb_id, 0, read_buf.capacity() as u32) {
                            Ok(0) => break,
                            Ok(n) => n,
                            Err(_) => break,
                        }
                    } else {
                        match stream.read(read_buf.as_write_buf()) {
                            Ok(0) => break,
                            Ok(n) => n,
                            Err(_) => break,
                        }
                    };
                    read_buf.set_len(n);

                    let request = match tachyon_http::parser::parse(read_buf.filled()) {
                        tachyon_http::parser::ParseResult::Complete(req) => req,
                        tachyon_http::parser::ParseResult::Incomplete => {
                            continue;
                        }
                        tachyon_http::parser::ParseResult::Error(_) => {
                            let mut res = Response::new(write_buf.as_write_buf(), sec_headers);
                            let n = res.text(400, b"Bad Request");
                            let _ = rio_or_write(&rio_conn, write_buf_id, &mut stream, &write_buf.as_write_buf()[..n]);
                            break;
                        }
                    };

                    let response_len = if config.catch_panics {
                        let result = safety::catch_handler_mut(|| {
                            let mut res = Response::new(write_buf.as_write_buf(), sec_headers);
                            let n = handler(&request, &mut res);
                            Ok(n)
                        });
                        match result.into_result() {
                            Ok(n) => n,
                            Err(e) => {
                                eprintln!("[tachyon] Handler failed: {}", e);
                                let mut res = Response::new(write_buf.as_write_buf(), sec_headers);
                                res.json(500, b"{\"error\":\"internal\"}")
                            }
                        }
                    } else {
                        let mut res = Response::new(write_buf.as_write_buf(), sec_headers);
                        handler(&request, &mut res)
                    };

                    // WRITE: use RIO zero-copy send when available, else standard write
                    if rio_or_write(&rio_conn, write_buf_id, &mut stream, &write_buf.as_write_buf()[..response_len]).is_err()
                    {
                        break;
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

                // Cleanup: deregister buffers before they return to the pool.
                // RioConn is dropped automatically, destroying the context.
                if let Some(id) = read_buf_id {
                    rio::deregister_buffer(id);
                }
                if let Some(id) = write_buf_id {
                    rio::deregister_buffer(id);
                }
            });
        }

        Ok(())
    }
}
