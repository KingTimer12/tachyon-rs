use may::net::TcpListener;

/// Apply socket tuning from config to a listener.
///
/// With `simd` feature: delegates to C via cxx bridge (handles all platforms cleanly).
/// Without `simd`: applies the subset available through Rust's std/libc.
pub fn apply_socket_config(listener: &TcpListener, socket: &crate::config::SocketConfig) {
    #[cfg(feature = "simd")]
    {
        use std::os::fd::AsRawFd;
        let fd = listener.as_raw_fd();
        let tuning = tachyon_simd::SocketTuning {
            reuse_port: socket.reuse_port,
            tcp_nodelay: socket.tcp_nodelay,
            tcp_fastopen: socket.tcp_fastopen,
            busy_poll_us: socket.busy_poll_us,
            recv_buf_size: socket.recv_buf_size,
            send_buf_size: socket.send_buf_size,
        };
        let err = tachyon_simd::apply_socket_tuning(fd, &tuning);
        if err != 0 {
            eprintln!("[turbine] Socket tuning warning: errno {}", err);
        }
    }
 
    #[cfg(not(feature = "simd"))]
    {
        // Pure Rust fallback: set what we can via std.
        // TCP_NODELAY and buffer sizes require per-connection setting
        // via std::net::TcpStream, which we do in the accept loop.
        // Listener-level options like SO_REUSEPORT need libc, so we
        // log a hint if the user wants them without the simd feature.
        if socket.reuse_port || socket.tcp_fastopen || socket.busy_poll_us > 0 {
            eprintln!(
                "[turbine] Socket options (reuse_port, tcp_fastopen, busy_poll) \
                 require the 'simd' feature. Build with: cargo build --features simd"
            );
        }
        let _ = listener;
    }
}