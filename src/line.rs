use std::io::BufRead;
use std::io::Read;
use std::io::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Separator {
    Newline,
    Null,
}

impl Separator {
    pub fn as_byte(&self) -> u8 {
        match self {
            Separator::Newline => b'\n',
            Separator::Null => b'\0',
        }
    }

    pub fn remove_trail(&self, buffer: &mut Vec<u8>) {
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

pub struct LineReader<R> {
    inner: R,
    separator: Separator,
    buffer: Vec<u8>,
}

impl<R> LineReader<R> {
    pub fn new(inner: R, separator: Separator) -> Self {
        Self {
            inner,
            separator,
            buffer: Vec::new(),
        }
    }
}

impl<R: BufRead> LineReader<R> {
    pub fn read(&mut self, max_len: usize) -> Result<Option<&[u8]>> {
        self.buffer.clear();
        self.inner
            .by_ref()
            .take(max_len as u64)
            .read_until(self.separator.as_byte(), &mut self.buffer)
            .map(|length| {
                if length > 0 {
                    self.separator.remove_trail(&mut self.buffer);
                    Some(self.buffer.as_slice())
                } else {
                    None
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bstr::ByteSlice;
    use bstr::ByteVec;
    use bstr::B;
    use claim::assert_ok_eq;

    #[test]
    fn separator_as_byte() {
        assert_eq!(Separator::Newline.as_byte(), b'\n');
        assert_eq!(Separator::Null.as_byte(), b'\0');
    }

    #[test]
    fn separator_remove_trail() {
        let mut buf: Vec<u8> = Vec::new();
        buf.push_str("abc");

        Separator::Newline.remove_trail(&mut buf);
        assert_eq!(buf.as_bstr(), "abc");

        buf.push_str("\n");
        Separator::Newline.remove_trail(&mut buf);
        assert_eq!(buf.as_bstr(), "abc");

        buf.push_str("\r\n");
        Separator::Newline.remove_trail(&mut buf);
        assert_eq!(buf.as_bstr(), "abc");

        Separator::Null.remove_trail(&mut buf);
        assert_eq!(buf.as_bstr(), "abc");

        buf.push_str("\0");
        Separator::Null.remove_trail(&mut buf);
        assert_eq!(buf.as_bstr(), "abc");
    }

    #[test]
    fn reader_newline() {
        let input = "a\nbc\r\ndef\nghij".as_bytes();
        let mut reader = LineReader::new(input, Separator::Newline);

        assert_ok_eq!(reader.read(5), Some(B("a")));
        assert_ok_eq!(reader.read(5), Some(B("bc")));
        assert_ok_eq!(reader.read(4), Some(B("def")));
        assert_ok_eq!(reader.read(3), Some(B("ghi")));
        assert_ok_eq!(reader.read(3), Some(B("j")));
        assert_ok_eq!(reader.read(1), None);
    }

    #[test]
    fn reader_null() {
        let input = "a\0bc\0def\0ghij".as_bytes();
        let mut reader = LineReader::new(input, Separator::Null);

        assert_ok_eq!(reader.read(5), Some(B("a")));
        assert_ok_eq!(reader.read(5), Some(B("bc")));
        assert_ok_eq!(reader.read(4), Some(B("def")));
        assert_ok_eq!(reader.read(3), Some(B("ghi")));
        assert_ok_eq!(reader.read(3), Some(B("j")));
        assert_ok_eq!(reader.read(1), None);
    }
}
