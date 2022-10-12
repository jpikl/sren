use std::io::BufRead;
use std::io::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Separator {
    Newline,
    Null,
}

impl Default for Separator {
    fn default() -> Self {
        Separator::Newline
    }
}

impl Separator {
    fn as_byte(&self) -> u8 {
        match self {
            Separator::Newline => b'\n',
            Separator::Null => b'\0',
        }
    }

    fn remove_trail(&self, buffer: &mut Vec<u8>) {
        match self {
            Separator::Newline => {
                if buffer.last() == Some(&b'\n') {
                    buffer.pop();

                    if buffer.last() == Some(&b'\r') {
                        buffer.pop();
                    }
                }
            }
            Separator::Null => {
                if buffer.last() == Some(&b'\0') {
                    buffer.pop();
                }
            }
        }
    }
}

pub struct Reader<R> {
    inner: R,
    separator: Separator,
}

impl<R> Reader<R> {
    pub fn new(inner: R, separator: Separator) -> Self {
        Self { inner, separator }
    }
}

use std::io::Read;

impl<R: BufRead> Reader<R> {
    pub fn read(&mut self, buf: &mut Vec<u8>, limit: usize) -> Result<usize> {
        let length = self
            .inner
            .by_ref()
            .take(limit as u64)
            .read_until(self.separator.as_byte(), buf)?;

        self.separator.remove_trail(buf);
        Ok(length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bstr::ByteSlice;
    use claim::assert_ok;
    use claim::assert_ok_eq;
    use test_case::test_case;

    #[test]
    fn separator_default() {
        assert_eq!(Separator::default(), Separator::Newline);
    }

    #[test_case(Separator::Newline, b'\n' ; "newline")]
    #[test_case(Separator::Null,    b'\0' ; "null")]
    fn separator_as_byte(separator: Separator, byte: u8) {
        assert_eq!(separator.as_byte(), byte);
    }

    #[test_case(Separator::Newline, "",      ""      ; "newline eof")]
    #[test_case(Separator::Newline, "\n",    ""      ; "newline lf")]
    #[test_case(Separator::Newline, "\n\r",  "\n\r"  ; "newline lf/cr")]
    #[test_case(Separator::Newline, "\r",    "\r"    ; "newline rf")]
    #[test_case(Separator::Newline, "\r\n",  ""      ; "newline cr/lf")]
    #[test_case(Separator::Newline, "\0",    "\0"    ; "newline null")]
    #[test_case(Separator::Newline, "a",     "a"     ; "newline value")]
    #[test_case(Separator::Newline, "a\n",   "a"     ; "newline value/lf")]
    #[test_case(Separator::Newline, "a\n\r", "a\n\r" ; "newline value/lf/cr")]
    #[test_case(Separator::Newline, "a\r",   "a\r"   ; "newline value/cr")]
    #[test_case(Separator::Newline, "a\r\n", "a"     ; "newline value/cr/lf")]
    #[test_case(Separator::Newline, "a\0",   "a\0"   ; "newline value/null")]
    #[test_case(Separator::Null,    "",      ""      ; "null eof")]
    #[test_case(Separator::Null,    "\n",    "\n"    ; "null lf")]
    #[test_case(Separator::Null,    "\n\r",  "\n\r"  ; "null lf/cr")]
    #[test_case(Separator::Null,    "\r",    "\r"    ; "null cr")]
    #[test_case(Separator::Null,    "\r\n",  "\r\n"  ; "null cr/lf")]
    #[test_case(Separator::Null,    "\0",    ""      ; "null null")]
    #[test_case(Separator::Null,    "a",     "a"     ; "null value")]
    #[test_case(Separator::Null,    "a\n",   "a\n"   ; "null value/lf")]
    #[test_case(Separator::Newline, "a\n\r", "a\n\r" ; "null value/lf/cr")]
    #[test_case(Separator::Null,    "a\r",   "a\r"   ; "null value/cr")]
    #[test_case(Separator::Null,    "a\r\n", "a\r\n" ; "null value/cr/lf")]
    #[test_case(Separator::Null,    "a\0",   "a"     ; "null value/null")]
    fn separator_remove_trail(separator: Separator, input: &str, output: &str) {
        let mut value = input.as_bytes().to_vec();
        separator.remove_trail(&mut value);

        assert_eq!(value.as_bstr(), output);
    }

    #[test_case(Separator::Newline, "",            &[]         ; "newline eof ")]
    #[test_case(Separator::Newline, "\n",          &[""]       ; "newline lf")]
    #[test_case(Separator::Newline, "\r\n",        &[""]       ; "newline crlf")]
    #[test_case(Separator::Newline, "a\nbc",       &["a", "bc"]; "newline lf/eof")]
    #[test_case(Separator::Newline, "a\nbc\n",     &["a", "bc"]; "newline lf/lf")]
    #[test_case(Separator::Newline, "a\nbc\r\n",   &["a", "bc"]; "newline lf/crlf")]
    #[test_case(Separator::Newline, "a\r\nbc",     &["a", "bc"]; "newline crlf/eof")]
    #[test_case(Separator::Newline, "a\r\nbc\n",   &["a", "bc"]; "newline crlf/lf")]
    #[test_case(Separator::Newline, "a\r\nbc\r\n", &["a", "bc"]; "newline crlf/crlf")]
    #[test_case(Separator::Null,    "",            &[]         ; "null eof ")]
    #[test_case(Separator::Null,    "\0",          &[""]       ; "null null")]
    #[test_case(Separator::Null,    "a\0bc",       &["a", "bc"]; "null null/eof")]
    #[test_case(Separator::Null,    "a\0bc\0",     &["a", "bc"]; "null null/null")]
    fn reader(separator: Separator, input: &str, output: &[&str]) {
        let mut reader = Reader::new(input.as_bytes(), separator);
        let mut buffer = Vec::new();
        let mut result = Vec::new();

        while assert_ok!(reader.read(&mut buffer, 1024)) > 0 {
            result.push(buffer.to_str_lossy().to_string());
            buffer.clear();
        }

        assert_eq!(result, output);
    }

    #[test]
    fn reader_limit() {
        let input = "0123456\n0123456".as_bytes();
        let mut reader = Reader::new(input, Separator::Newline);
        let mut buffer = Vec::new();

        assert_ok_eq!(reader.read(&mut buffer, 4), 4);
        assert_eq!(buffer.as_bstr(), "0123");

        buffer.clear();
        assert_ok_eq!(reader.read(&mut buffer, 4), 4);
        assert_eq!(buffer.as_bstr(), "456");

        buffer.clear();
        assert_ok_eq!(reader.read(&mut buffer, 4), 4);
        assert_eq!(buffer.as_bstr(), "0123");

        buffer.clear();
        assert_ok_eq!(reader.read(&mut buffer, 4), 3);
        assert_eq!(buffer.as_bstr(), "456");
    }
}
