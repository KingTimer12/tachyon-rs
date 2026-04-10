use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Instant,
};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::{config::ServerConfig, response::Response, utils::apply_socket_config};

/// Write function returned by an async handler. Called synchronously after the future resolves.
pub type WriteFn = Box<dyn FnOnce(&mut Response) -> usize + Send>;

/// The handler function type. Takes a borrowed request, returns a future.
/// The future resolves to a WriteFn that writes the HTTP response.
/// This design avoids block_in_place — the async bridge uses rx.await instead.
pub type Handler = Arc<
    dyn for<'r> Fn(
            &'r tachyon_http::http::Request<'r>,
        ) -> Pin<Box<dyn Future<Output = WriteFn> + Send>>
        + Send
        + Sync,
>;

/// The tachyon server.
pub struct Server {
    config: ServerConfig,
}

impl Server {
    pub fn new(config: ServerConfig) -> Self {
        Self { config }
    }

    /// Pre-initialize all lazy systems so the first real request is fast.
    /// Also pins each worker thread to its own CPU core (Linux only via sched_setaffinity).
    async fn warmup_pools(workers: usize) {
        use std::sync::atomic::AtomicI32;
        let cpu_counter = Arc::new(AtomicI32::new(0));
        let mut handles = Vec::with_capacity(workers * 2);
        for _ in 0..(workers * 2) {
            let cpu_counter = cpu_counter.clone();
            handles.push(tokio::spawn(async move {
                thread_local! {
                    static PINNED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
                }
                PINNED.with(|pinned| {
                    if !pinned.get() {
                        pinned.set(true);
                        #[cfg(feature = "simd")]
                        {
                            let cpu_id = cpu_counter.fetch_add(1, Ordering::Relaxed);
                            let err = tachyon_simd::set_cpu_affinity(cpu_id);
                            if err != 0 {
                                eprintln!(
                                    "[tachyon] CPU affinity warning for core {}: errno {}",
                                    cpu_id, -err
                                );
                            }
                        }
                        #[cfg(not(feature = "simd"))]
                        let _ = &cpu_counter;
                    }
                });
                let _buf = tachyon_pool::pool::acquire();
            }));
        }
        for h in handles {
            let _ = h.await;
        }
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
    /// Creates a multi-threaded Tokio runtime and blocks until the server stops.
    pub fn run(self, handler: Handler) -> std::io::Result<()> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(self.config.workers)
            .enable_io()
            .build()?;
        rt.block_on(self.run_inner(handler))
    }

    async fn run_inner(self, handler: Handler) -> std::io::Result<()> {
        let listener = TcpListener::bind(&self.config.bind_addr).await?;

        apply_socket_config(&listener, &self.config.socket);

        crate::date::start_date_cache();

        let config = Arc::new(self.config);

        // Phase 1: Warm buffer pools on all worker threads
        let warmup_start = Instant::now();
        Self::warmup_pools(config.workers).await;
        eprintln!("[tachyon] Pool warmup: {:?}", warmup_start.elapsed());

        // Phase 2: Warmup requests through the full pipeline to trigger V8 JIT.
        let loopback_addr = Self::to_loopback(&config.bind_addr);
        let warmup_count: usize = 10;
        let warmup_completed = Arc::new(AtomicUsize::new(0));
        let warmup_ready = Arc::new(AtomicBool::new(false));

        let warmup_completed2 = warmup_completed.clone();
        let warmup_ready2 = warmup_ready.clone();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
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
            eprintln!(
                "[tachyon] All {} warmup requests done: {:?}",
                warmup_count,
                t.elapsed()
            );
            warmup_ready2.store(true, Ordering::Release);
        });

        let mut warmup_printed = false;

        loop {
            let (stream, _addr) = listener.accept().await?;

            if !warmup_printed && warmup_ready.load(Ordering::Acquire) {
                eprintln!(
                    "[tachyon] Listening on {} ({} workers, {} buffers/worker)",
                    config.bind_addr, config.workers, config.buffers_per_worker,
                );
                warmup_printed = true;
            }

            let handler = handler.clone();
            let config = config.clone();

            tokio::spawn(async move {
                if config.socket.tcp_nodelay {
                    let _ = stream.set_nodelay(true);
                }

                let mut read_buf = tachyon_pool::pool::acquire();
                let mut write_buf = tachyon_pool::pool::acquire();
                let sec_headers = config.security.as_bytes();
                let comp_threshold = config.compression_threshold;

                // Split into owned read/write halves to allow concurrent use
                let (mut reader, mut writer) = stream.into_split();

                let mut buf_offset: usize = 0;
                let mut buf_len: usize = 0;

                'conn: loop {
                    if buf_offset >= buf_len {
                        buf_offset = 0;
                        let n = match reader.read(read_buf.as_write_buf()).await {
                            Ok(0) => break,
                            Ok(n) => n,
                            Err(_) => break,
                        };
                        buf_len = n;
                        read_buf.set_len(n);
                    }

                    loop {
                        let data = &read_buf.as_write_buf()[buf_offset..buf_len];
                        if data.is_empty() {
                            break;
                        }

                        let request = match tachyon_http::parser::parse(data) {
                            tachyon_http::parser::ParseResult::Complete(req) => req,
                            tachyon_http::parser::ParseResult::Incomplete => {
                                if buf_offset > 0 {
                                    let remaining = buf_len - buf_offset;
                                    let wbuf = read_buf.as_write_buf();
                                    wbuf.copy_within(buf_offset..buf_len, 0);
                                    buf_offset = 0;
                                    read_buf.set_len(remaining);
                                    let n = match reader
                                        .read(&mut read_buf.as_write_buf()[remaining..])
                                        .await
                                    {
                                        Ok(0) => break 'conn,
                                        Ok(n) => n,
                                        Err(_) => break 'conn,
                                    };
                                    buf_len = remaining + n;
                                    read_buf.set_len(buf_len);
                                }
                                break;
                            }
                            tachyon_http::parser::ParseResult::Error(_) => {
                                let mut res = Response::new(
                                    write_buf.as_write_buf(),
                                    sec_headers,
                                    false,
                                    comp_threshold,
                                );
                                res.text(400, b"Bad Request");
                                let _ = writer.write_all(res.data()).await;
                                break 'conn;
                            }
                        };

                        buf_offset += request.consumed();

                        let accepts_gzip = request
                            .header(b"accept-encoding")
                            .map(|v| v.windows(4).any(|w| w == b"gzip"))
                            .unwrap_or(false);

                        let mut res = Response::new(
                            write_buf.as_write_buf(),
                            sec_headers,
                            accepts_gzip,
                            comp_threshold,
                        );
                        let write = handler(&request).await;
                        if config.catch_panics {
                            use std::panic::{AssertUnwindSafe, catch_unwind};
                            if catch_unwind(AssertUnwindSafe(|| {
                                write(&mut res);
                            }))
                            .is_err()
                            {
                                eprintln!("[tachyon] Handler panicked");
                                res = Response::new(
                                    write_buf.as_write_buf(),
                                    sec_headers,
                                    accepts_gzip,
                                    comp_threshold,
                                );
                                res.json(500, b"{\"error\":\"internal\"}");
                            }
                        } else {
                            write(&mut res);
                        };

                        if writer.write_all(res.data()).await.is_err() {
                            break 'conn;
                        }

                        if request.version_minor == 0
                            || request
                                .header(b"connection")
                                .map(|v| v == b"close")
                                .unwrap_or(false)
                        {
                            break 'conn;
                        }
                    }
                }
            });
        }
    }
}
