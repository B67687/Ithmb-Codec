// allow: SIZE_OK — hand-rolled JSON parser is an indivisible state machine

use crate::error::DecodeError;

/// Cursor-based JSON parser for profile data.
pub(crate) struct Parser<'a> {
    pub bytes: &'a [u8],
    pub pos: usize,
}

impl Parser<'_> {
    // -- low-level helpers --

    pub fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    pub fn advance(&mut self) -> Result<u8, DecodeError> {
        let b = self
            .bytes
            .get(self.pos)
            .copied()
            .ok_or_else(|| DecodeError::Profile("unexpected end of input".into()))?;
        self.pos += 1;
        Ok(b)
    }

    pub fn expect(&mut self, want: u8) -> Result<(), DecodeError> {
        let got = self.advance()?;
        if got != want {
            return Err(DecodeError::Profile(format!(
                "expected '{}' (0x{:02X}), got '{}' (0x{:02X}) at offset {}",
                want as char,
                want,
                got as char,
                got,
                self.pos - 1,
            )));
        }
        Ok(())
    }

    pub fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if b != b' ' && b != b'\t' && b != b'\n' && b != b'\r' {
                break;
            }
            self.pos += 1;
        }
    }

    // -- value parsers --

    pub fn parse_string(&mut self) -> Result<String, DecodeError> {
        self.expect(b'"')?;
        let mut s = String::new();
        loop {
            let b = self.advance()?;
            match b {
                b'"' => return Ok(s),
                b'\\' => {
                    let esc = self.advance()?;
                    match esc {
                        b'"' => s.push('"'),
                        b'\\' => s.push('\\'),
                        b'/' => s.push('/'),
                        b'b' => s.push('\u{0008}'),
                        b'f' => s.push('\u{000C}'),
                        b'n' => s.push('\n'),
                        b'r' => s.push('\r'),
                        b't' => s.push('\t'),
                        b'u' => {
                            let code = self.parse_hex4()?;
                            let c = char::from_u32(code).ok_or_else(|| {
                                DecodeError::Profile(format!("invalid unicode escape: \\u{code:04X}"))
                            })?;
                            s.push(c);
                        }
                        _ => {
                            return Err(DecodeError::Profile(format!(
                                "invalid escape sequence: \\{}",
                                esc as char
                            )));
                        }
                    }
                }
                // JSON allows most characters inside strings, including UTF-8
                0x20..=0x7E | 0x80..=0xFF => s.push(b as char),
                _ => {
                    return Err(DecodeError::Profile(format!("invalid character in string: 0x{b:02X}")));
                }
            }
        }
    }

    pub fn parse_hex4(&mut self) -> Result<u32, DecodeError> {
        let mut val: u32 = 0;
        for _ in 0..4 {
            let b = self.advance()?;
            val <<= 4;
            val += match b {
                b'0'..=b'9' => u32::from(b - b'0'),
                b'a'..=b'f' => u32::from(b - b'a' + 10),
                b'A'..=b'F' => u32::from(b - b'A' + 10),
                _ => {
                    return Err(DecodeError::Profile(format!("invalid hex digit: '{}'", b as char)));
                }
            };
        }
        Ok(val)
    }

    pub fn parse_number_i32(&mut self) -> Result<i32, DecodeError> {
        self.skip_ws();
        let start = self.pos;
        if self.pos >= self.bytes.len() {
            return Err(DecodeError::Profile(format!("expected number at offset {}", self.pos)));
        }
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        if self.peek().is_none_or(|b| !b.is_ascii_digit()) {
            return Err(DecodeError::Profile(format!("expected number at offset {start}")));
        }
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.pos += 1;
        }
        let s = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|e| DecodeError::Profile(format!("invalid number encoding: {e}")))?;
        s.parse::<i32>()
            .map_err(|e| DecodeError::Profile(format!("invalid number '{s}': {e}")))
    }

    pub fn parse_bool(&mut self) -> Result<bool, DecodeError> {
        self.skip_ws();
        if self.pos >= self.bytes.len() {
            return Err(DecodeError::Profile(format!("expected bool at offset {}", self.pos)));
        }
        if self.bytes[self.pos..].starts_with(b"true") {
            self.pos += 4;
            Ok(true)
        } else if self.bytes[self.pos..].starts_with(b"false") {
            self.pos += 5;
            Ok(false)
        } else {
            Err(DecodeError::Profile(format!("expected bool at offset {}", self.pos)))
        }
    }

    pub fn skip_value(&mut self) -> Result<(), DecodeError> {
        self.skip_ws();
        match self.peek() {
            Some(b'"') => {
                self.parse_string()?;
            }
            Some(b't' | b'f') => {
                self.parse_bool()?;
            }
            Some(b'n') => {
                self.parse_null()?;
            }
            Some(b'[') => {
                self.skip_array()?;
            }
            Some(b'{') => {
                self.skip_object()?;
            }
            Some(b'-' | b'0'..=b'9') => {
                self.parse_number_i32()?;
            }
            Some(c) => {
                return Err(DecodeError::Profile(format!(
                    "unexpected character '{c}' at offset {}",
                    self.pos
                )));
            }
            None => {
                return Err(DecodeError::Profile("unexpected end of input".into()));
            }
        }
        Ok(())
    }

    pub fn parse_null(&mut self) -> Result<(), DecodeError> {
        self.skip_ws();
        if self.bytes[self.pos..].starts_with(b"null") {
            self.pos += 4;
            Ok(())
        } else {
            Err(DecodeError::Profile(format!("expected null at offset {}", self.pos)))
        }
    }

    pub fn skip_array(&mut self) -> Result<(), DecodeError> {
        self.expect(b'[')?;
        loop {
            self.skip_ws();
            if self.peek() == Some(b']') {
                self.pos += 1;
                return Ok(());
            }
            self.skip_value()?;
            self.skip_ws();
            if self.peek() == Some(b',') {
                self.pos += 1;
            }
        }
    }

    pub fn skip_object(&mut self) -> Result<(), DecodeError> {
        self.expect(b'{')?;
        loop {
            self.skip_ws();
            if self.peek() == Some(b'}') {
                self.pos += 1;
                return Ok(());
            }
            self.parse_string()?;
            self.skip_ws();
            self.expect(b':')?;
            self.skip_value()?;
            self.skip_ws();
            if self.peek() == Some(b',') {
                self.pos += 1;
            }
        }
    }
}
