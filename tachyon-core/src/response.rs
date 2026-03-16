/// Response builder passed to the user's handler callback.
/// Wraps a buffer from the pool — FaF-style: you write into a pre-allocated
/// buffer and return how many bytes you wrote.
pub struct Response<'a> {
    buf: &'a mut [u8],
    pos: usize,
    security_headers: &'a [u8],
    custom_headers: Vec<u8>,
}

impl<'a> Response<'a> {
    pub fn new(buf: &'a mut [u8], security_headers: &'a [u8]) -> Self {
        Self { buf, pos: 0, security_headers, custom_headers: Vec::new() }
    }

    /// Add a custom header to the response.
    /// Headers are accumulated and written in order before the body.
    pub fn header(&mut self, name: &[u8], value: &[u8]) {
        self.custom_headers.extend_from_slice(name);
        self.custom_headers.extend_from_slice(b": ");
        self.custom_headers.extend_from_slice(value);
        self.custom_headers.extend_from_slice(b"\r\n");
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
        self.pos = tachyon_http::response::write_response(
            self.buf,
            status_line,
            tachyon_http::response::CONTENT_JSON,
            body,
            self.security_headers,
            &self.custom_headers,
        );
        self.pos
    }

    /// Write a complete HTTP response with plain text body.
    pub fn text(&mut self, status: u16, body: &[u8]) -> usize {
        let status_line = match status {
            200 => tachyon_http::response::STATUS_200,
            404 => tachyon_http::response::STATUS_404,
            _ => tachyon_http::response::STATUS_500,
        };
        self.pos = tachyon_http::response::write_response(
            self.buf,
            status_line,
            tachyon_http::response::CONTENT_TEXT,
            body,
            self.security_headers,
            &self.custom_headers,
        );
        self.pos
    }

    /// Write raw bytes directly into the response buffer.
    /// Returns the new position. Use this for custom responses.
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
        self.pos
    }
}
