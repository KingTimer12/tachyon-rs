use flate2::Compression;
use flate2::write::GzEncoder;
use std::io::Write;

/// Response builder passed to the user's handler callback.
/// Wraps a buffer from the pool — FaF-style: you write into a pre-allocated
/// buffer and return how many bytes you wrote.
///
/// If the response exceeds the pool buffer, transparently falls back to a
/// heap-allocated Vec (the "overflow" path). The caller uses `data()` to get
/// the final bytes regardless of which path was taken.
pub struct Response<'a> {
    buf: &'a mut [u8],
    pos: usize,
    overflow: Option<Vec<u8>>,
    security_headers: &'a [u8],
    custom_headers: Vec<u8>,
    accepts_gzip: bool,
    compression_threshold: usize,
}

impl<'a> Response<'a> {
    pub fn new(
        buf: &'a mut [u8],
        security_headers: &'a [u8],
        accepts_gzip: bool,
        compression_threshold: usize,
    ) -> Self {
        Self {
            buf,
            pos: 0,
            overflow: None,
            security_headers,
            custom_headers: Vec::new(),
            accepts_gzip,
            compression_threshold,
        }
    }

    /// Add a custom header to the response.
    pub fn header(&mut self, name: &[u8], value: &[u8]) {
        self.custom_headers.extend_from_slice(name);
        self.custom_headers.extend_from_slice(b": ");
        self.custom_headers.extend_from_slice(value);
        self.custom_headers.extend_from_slice(b"\r\n");
    }

    /// Should we compress this body?
    /// threshold == usize::MAX means compression is disabled.
    /// threshold == 0 means compress everything (no minimum size).
    fn should_compress(&self, body: &[u8]) -> bool {
        self.accepts_gzip
            && self.compression_threshold < usize::MAX
            && body.len() >= self.compression_threshold
    }

    /// Compress body with gzip. Returns None if compression doesn't shrink it.
    fn compress_gzip(body: &[u8]) -> Option<Vec<u8>> {
        let mut encoder = GzEncoder::new(Vec::with_capacity(body.len()), Compression::fast());
        if encoder.write_all(body).is_err() {
            return None;
        }
        let compressed = encoder.finish().ok()?;
        // Only use compressed version if it's actually smaller
        if compressed.len() < body.len() {
            Some(compressed)
        } else {
            None
        }
    }

    /// Write a complete HTTP response with JSON body.
    pub fn json(&mut self, status: u16, body: &[u8]) -> usize {
        let status_line = match status {
            200 => tachyon_http::response::STATUS_200,
            201 => tachyon_http::response::STATUS_201,
            204 => tachyon_http::response::STATUS_204,
            400 => tachyon_http::response::STATUS_400,
            404 => tachyon_http::response::STATUS_404,
            _ => tachyon_http::response::STATUS_500,
        };
        self.write_with_optional_compression(
            status_line,
            tachyon_http::response::CONTENT_JSON,
            body,
        )
    }

    /// Write a complete HTTP response with plain text body.
    pub fn text(&mut self, status: u16, body: &[u8]) -> usize {
        let status_line = match status {
            200 => tachyon_http::response::STATUS_200,
            404 => tachyon_http::response::STATUS_404,
            _ => tachyon_http::response::STATUS_500,
        };
        self.write_with_optional_compression(
            status_line,
            tachyon_http::response::CONTENT_TEXT,
            body,
        )
    }

    fn write_with_optional_compression(
        &mut self,
        status_line: &[u8],
        content_type: &[u8],
        body: &[u8],
    ) -> usize {
        if self.should_compress(body)
            && let Some(compressed) = Self::compress_gzip(body)
        {
            // Body was compressed — add encoding headers
            self.custom_headers
                .extend_from_slice(tachyon_http::response::ENCODING_GZIP);
            self.custom_headers
                .extend_from_slice(tachyon_http::response::VARY_ACCEPT_ENCODING);
            return self.write_final(status_line, content_type, &compressed);
        }
        self.write_final(status_line, content_type, body)
    }

    /// Write the final response, using the pool buffer if it fits, or heap-allocating otherwise.
    fn write_final(&mut self, status_line: &[u8], content_type: &[u8], body: &[u8]) -> usize {
        let date_header = crate::date::cached_date_header();
        let total = tachyon_http::response::response_size(
            status_line,
            content_type,
            body,
            self.security_headers,
            &self.custom_headers,
            date_header,
        );

        if total <= self.buf.len() {
            // Fast path: fits in the pre-allocated pool buffer
            self.pos = tachyon_http::response::write_response(
                self.buf,
                status_line,
                content_type,
                body,
                self.security_headers,
                &self.custom_headers,
                date_header,
            );
            self.pos
        } else {
            // Overflow path: heap-allocate for large responses
            let vec = tachyon_http::response::write_response_vec(
                status_line,
                content_type,
                body,
                self.security_headers,
                &self.custom_headers,
                date_header,
            );
            let len = vec.len();
            self.overflow = Some(vec);
            len
        }
    }

    /// Get the response bytes to send. Uses pool buffer or overflow Vec.
    pub fn data(&self) -> &[u8] {
        if let Some(ref vec) = self.overflow {
            vec
        } else {
            &self.buf[..self.pos]
        }
    }

    /// Whether the response overflowed to heap allocation.
    pub fn is_overflow(&self) -> bool {
        self.overflow.is_some()
    }

    /// Write raw bytes directly into the response buffer.
    pub fn write_raw(&mut self, data: &[u8]) -> usize {
        let end = self.pos + data.len();
        if end <= self.buf.len() {
            self.buf[self.pos..end].copy_from_slice(data);
            self.pos = end;
        }
        self.pos
    }

    /// Bytes written so far.
    pub fn len(&self) -> usize {
        if let Some(ref vec) = self.overflow {
            vec.len()
        } else {
            self.pos
        }
    }

    /// Returns true if no bytes have been written.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Write a JSON response built with the zero-alloc `JsonWriter`.
    ///
    /// The closure receives a `JsonWriter` backed by a stack buffer.
    /// The resulting JSON is used as the response body — no heap allocation
    /// for the JSON itself.
    ///
    /// ```ignore
    /// res.json_writer(200, |w| {
    ///     w.object(|w| {
    ///         w.key("message").string_raw("Hello, World!");
    ///         w.key("id").int(42);
    ///     });
    /// });
    /// ```
    pub fn json_writer(
        &mut self,
        status: u16,
        f: impl FnOnce(&mut tachyon_http::json::JsonWriter),
    ) -> usize {
        // Use a 4KB stack buffer for building the JSON body.
        // For responses larger than this, the caller should use `json()` with a pre-built body.
        let mut json_buf = [0u8; 4096];
        let mut writer = tachyon_http::json::JsonWriter::new(&mut json_buf);
        f(&mut writer);
        let json_len = writer.finish();
        self.json(status, &json_buf[..json_len])
    }
}
