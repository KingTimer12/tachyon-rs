/// Zero-allocation JSON writer that serializes directly into a `&mut [u8]` buffer.
///
/// Inspired by just-js's per-schema JSON serializer: instead of building a
/// `serde_json::Value` tree and then formatting it, write the JSON bytes
/// directly into the response buffer with zero intermediate allocations.
///
/// ```ignore
/// let mut buf = [0u8; 256];
/// let mut w = JsonWriter::new(&mut buf);
/// w.object(|w| {
///     w.key("message").string("Hello, World!");
///     w.key("id").int(42);
///     w.key("active").bool(true);
/// });
/// let json_bytes = w.finish(); // &[u8]
/// ```
pub struct JsonWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,
    /// Tracks whether we need a comma before the next value in an object/array.
    needs_comma: bool,
}

impl<'a> JsonWriter<'a> {
    #[inline]
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self {
            buf,
            pos: 0,
            needs_comma: false,
        }
    }

    /// Returns the written JSON bytes.
    #[inline]
    pub fn finish(self) -> usize {
        self.pos
    }

    /// Current write position (bytes written so far).
    #[inline]
    pub fn len(&self) -> usize {
        self.pos
    }

    #[inline]
    fn push_byte(&mut self, b: u8) {
        if self.pos < self.buf.len() {
            self.buf[self.pos] = b;
            self.pos += 1;
        }
    }

    #[inline]
    fn push_bytes(&mut self, src: &[u8]) {
        let end = self.pos + src.len();
        if end <= self.buf.len() {
            self.buf[self.pos..end].copy_from_slice(src);
            self.pos = end;
        }
    }

    #[inline]
    fn comma_if_needed(&mut self) {
        if self.needs_comma {
            self.push_byte(b',');
        }
    }

    /// Write a JSON string value (with escaping for control chars, quotes, backslashes).
    #[inline]
    pub fn string(&mut self, s: &str) -> &mut Self {
        self.comma_if_needed();
        self.push_byte(b'"');
        // Fast path: most API strings have no special chars
        for &b in s.as_bytes() {
            match b {
                b'"' => self.push_bytes(b"\\\""),
                b'\\' => self.push_bytes(b"\\\\"),
                b'\n' => self.push_bytes(b"\\n"),
                b'\r' => self.push_bytes(b"\\r"),
                b'\t' => self.push_bytes(b"\\t"),
                0..=0x1f => {
                    // \u00XX for other control characters
                    self.push_bytes(b"\\u00");
                    self.push_byte(HEX[(b >> 4) as usize]);
                    self.push_byte(HEX[(b & 0xf) as usize]);
                }
                _ => self.push_byte(b),
            }
        }
        self.push_byte(b'"');
        self.needs_comma = true;
        self
    }

    /// Write a raw JSON string that is known to be safe (no escaping needed).
    /// Caller guarantees the string contains no quotes, backslashes, or control chars.
    #[inline]
    pub fn string_raw(&mut self, s: &str) -> &mut Self {
        self.comma_if_needed();
        self.push_byte(b'"');
        self.push_bytes(s.as_bytes());
        self.push_byte(b'"');
        self.needs_comma = true;
        self
    }

    /// Write an integer value (zero-alloc via itoa).
    #[inline]
    pub fn int(&mut self, v: i64) -> &mut Self {
        self.comma_if_needed();
        let mut itoa_buf = itoa::Buffer::new();
        let s = itoa_buf.format(v);
        self.push_bytes(s.as_bytes());
        self.needs_comma = true;
        self
    }

    /// Write an unsigned integer value (zero-alloc via itoa).
    #[inline]
    pub fn uint(&mut self, v: u64) -> &mut Self {
        self.comma_if_needed();
        let mut itoa_buf = itoa::Buffer::new();
        let s = itoa_buf.format(v);
        self.push_bytes(s.as_bytes());
        self.needs_comma = true;
        self
    }

    /// Write a float value (zero-alloc via ryu).
    #[inline]
    pub fn float(&mut self, v: f64) -> &mut Self {
        self.comma_if_needed();
        let mut ryu_buf = ryu::Buffer::new();
        let s = ryu_buf.format(v);
        self.push_bytes(s.as_bytes());
        self.needs_comma = true;
        self
    }

    /// Write a boolean value.
    #[inline]
    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.comma_if_needed();
        if v {
            self.push_bytes(b"true");
        } else {
            self.push_bytes(b"false");
        }
        self.needs_comma = true;
        self
    }

    /// Write a null value.
    #[inline]
    pub fn null(&mut self) -> &mut Self {
        self.comma_if_needed();
        self.push_bytes(b"null");
        self.needs_comma = true;
        self
    }

    /// Write an object key. Must be followed by a value method.
    #[inline]
    pub fn key(&mut self, k: &str) -> &mut Self {
        self.comma_if_needed();
        self.push_byte(b'"');
        self.push_bytes(k.as_bytes());
        self.push_bytes(b"\":");
        self.needs_comma = false;
        self
    }

    /// Write a JSON object using a closure.
    #[inline]
    pub fn object(&mut self, f: impl FnOnce(&mut Self)) -> &mut Self {
        self.comma_if_needed();
        self.push_byte(b'{');
        let saved = self.needs_comma;
        self.needs_comma = false;
        f(self);
        self.push_byte(b'}');
        self.needs_comma = saved || true;
        self
    }

    /// Write a JSON array using a closure.
    #[inline]
    pub fn array(&mut self, f: impl FnOnce(&mut Self)) -> &mut Self {
        self.comma_if_needed();
        self.push_byte(b'[');
        let saved = self.needs_comma;
        self.needs_comma = false;
        f(self);
        self.push_byte(b']');
        self.needs_comma = saved || true;
        self
    }

    /// Write raw JSON bytes directly (caller responsible for validity).
    #[inline]
    pub fn raw(&mut self, json: &[u8]) -> &mut Self {
        self.comma_if_needed();
        self.push_bytes(json);
        self.needs_comma = true;
        self
    }
}

const HEX: [u8; 16] = *b"0123456789abcdef";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_object() {
        let mut buf = [0u8; 256];
        let mut w = JsonWriter::new(&mut buf);
        w.object(|w| {
            w.key("message").string_raw("Hello, World!");
        });
        let len = w.finish();
        assert_eq!(&buf[..len], b"{\"message\":\"Hello, World!\"}");
    }

    #[test]
    fn test_tfb_json() {
        let mut buf = [0u8; 256];
        let mut w = JsonWriter::new(&mut buf);
        w.object(|w| {
            w.key("message").string_raw("Hello, World!");
        });
        let len = w.finish();
        assert_eq!(
            std::str::from_utf8(&buf[..len]).unwrap(),
            r#"{"message":"Hello, World!"}"#
        );
    }

    #[test]
    fn test_nested() {
        let mut buf = [0u8; 512];
        let mut w = JsonWriter::new(&mut buf);
        w.object(|w| {
            w.key("id").int(42);
            w.key("name").string("test");
            w.key("active").bool(true);
            w.key("score").float(3.14);
            w.key("tags").array(|w| {
                w.string_raw("fast");
                w.string_raw("zero-alloc");
            });
            w.key("meta").null();
        });
        let len = w.finish();
        let s = std::str::from_utf8(&buf[..len]).unwrap();
        assert_eq!(
            s,
            r#"{"id":42,"name":"test","active":true,"score":3.14,"tags":["fast","zero-alloc"],"meta":null}"#
        );
    }

    #[test]
    fn test_escaping() {
        let mut buf = [0u8; 256];
        let mut w = JsonWriter::new(&mut buf);
        w.string("hello \"world\"\nnewline");
        let len = w.finish();
        assert_eq!(
            std::str::from_utf8(&buf[..len]).unwrap(),
            r#""hello \"world\"\nnewline""#
        );
    }

    #[test]
    fn test_array_of_objects() {
        let mut buf = [0u8; 512];
        let mut w = JsonWriter::new(&mut buf);
        w.array(|w| {
            for i in 1..=3 {
                w.object(|w| {
                    w.key("id").int(i);
                    w.key("randomNumber").int(i * 1000);
                });
            }
        });
        let len = w.finish();
        assert_eq!(
            std::str::from_utf8(&buf[..len]).unwrap(),
            r#"[{"id":1,"randomNumber":1000},{"id":2,"randomNumber":2000},{"id":3,"randomNumber":3000}]"#
        );
    }
}
