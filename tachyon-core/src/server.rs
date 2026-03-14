use std::{io::{Read, Write}, sync::Arc};

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
 
        eprintln!(
            "[tachyon] Listening on {} ({} workers, {}KB stack, {} buffers/worker)",
            config.bind_addr,
            config.workers,
            config.coroutine_stack_size / 1024,
            config.buffers_per_worker,
        );
 
        while let Ok((mut stream, _addr)) = listener.accept() {
            let handler = handler.clone();
            let config = config.clone();
 
            // May's go! macro: spawn a stackful coroutine for this connection.
            // This is where May and FaF philosophies merge:
            // - May provides cheap coroutine creation + cooperative I/O
            // - FaF provides zero-alloc buffer management inside the coroutine
            go!(move || {
                // Apply per-connection socket tuning (TCP_NODELAY)
                if config.socket.tcp_nodelay {
                    let _ = stream.set_nodelay(true);
                }
 
                // Step 1: Acquire buffer from thread-local pool (FaF pattern)
                let mut read_buf = tachyon_pool::pool::acquire();
                let mut write_buf = tachyon_pool::pool::acquire();
 
                loop {
                    // Step 2: Read request into pooled buffer
                    let n = match stream.read(read_buf.as_write_buf()) {
                        Ok(0) => break, // Connection closed
                        Ok(n) => n,
                        Err(_) => break,
                    };
                    read_buf.set_len(n);
 
                    // Step 3: Parse zero-copy (FaF pattern)
                    let request = match tachyon_http::parser::parse(read_buf.filled()) {
                        tachyon_http::parser::ParseResult::Complete(req) => req,
                        tachyon_http::parser::ParseResult::Incomplete => {
                            // TODO: accumulate more data for partial reads
                            continue;
                        }
                        tachyon_http::parser::ParseResult::Error(_) => {
                            // Write a 400 response
                            let mut res = Response::new(write_buf.as_write_buf());
                            let n = res.text(400, b"Bad Request");
                            let _ = stream.write_all(&write_buf.as_write_buf()[..n]);
                            break;
                        }
                    };
 
                    // Step 4: Call handler with safety boundary
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
 
                    // Step 5: Write response
                    if stream
                        .write_all(&write_buf.as_write_buf()[..response_len])
                        .is_err()
                    {
                        break;
                    }
 
                    // Check Connection: close
                    if request.version_minor == 0
                        || request
                            .header(b"connection")
                            .map(|v| v == b"close")
                            .unwrap_or(false)
                    {
                        break;
                    }
                }
                // Step 6: BufGuards drop here → buffers return to pool automatically
            });
        }
 
        Ok(())
    }
}