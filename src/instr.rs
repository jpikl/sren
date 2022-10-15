use crate::input::Reader as LineReader;
use bstr::ByteSlice;
use std::ascii::escape_default;
use std::fmt::Display;
use std::fmt::Formatter;
use std::io;
use std::io::BufRead;
use std::path::Path;

const MAX_LINE: usize = 1024 * 1024;

enum InstructionKind {
    Source,
    Dest,
}

struct Instruction {
    buf: Vec<u8>,
    line: usize,
}

impl Instruction {
    fn new() -> Self {
        Self {
            buf: Vec::new(),
            line: 0,
        }
    }

    fn set_line(&mut self, line: usize) {
        self.line = line;
    }

    fn fetch<R: BufRead>(&mut self, reader: &mut LineReader<R>) -> Result<usize, Error> {
        self.buf.clear();

        match reader.read(&mut self.buf, MAX_LINE) {
            Ok(length) => {
                if length >= MAX_LINE {
                    Err(self.error(ErrorKind::LineOverflow))
                } else {
                    Ok(length)
                }
            }
            Err(error) => Err(self.error(ErrorKind::IoError(error))),
        }
    }

    fn buf(&self) -> &[u8] {
        self.buf.as_bytes()
    }

    fn kind(&self) -> Result<InstructionKind, Error> {
        match Prefix::from(self.buf()) {
            Some(Prefix::Byte(b'<')) => Ok(InstructionKind::Source),
            Some(Prefix::Byte(b'>')) => Ok(InstructionKind::Dest),
            Some(prefix) => Err(self.error(ErrorKind::InvalidPrefix(prefix))),
            None => Err(self.error(ErrorKind::EmptyLine)),
        }
    }

    fn path(&self) -> Result<&Path, Error> {
        match self.buf.split_first() {
            Some((_, path)) if path.is_empty() => Err(self.error(ErrorKind::EmptyPath)),
            Some((_, path)) => match path.to_os_str() {
                Ok(value) => Ok(Path::new(value)),
                Err(_) => Err(self.error(ErrorKind::InvalidEncoding)),
            },
            _ => Err(self.error(ErrorKind::EmptyLine)),
        }
    }

    fn error(&self, kind: ErrorKind) -> Error {
        Error {
            kind,
            line: self.line,
            preview: self.buf().into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum ErrorKind {
    #[error("Invalid UTF-8 encoding")]
    InvalidEncoding,
    #[error("Invalid line prefix '{0}', expected '<' or '>'")]
    InvalidPrefix(Prefix),
    #[error("Empty line")]
    EmptyLine,
    #[error("Empty path")]
    EmptyPath,
    #[error("Line is bigger than {} bytes", MAX_LINE)]
    LineOverflow,
    #[error("There was no previous source path")]
    NoSourcePath,
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
}

#[derive(Debug, thiserror::Error)]
#[error("{kind}\nSource line #{line}: {preview}")]
pub struct Error {
    kind: ErrorKind,
    line: usize,
    preview: Preview,
}

#[derive(Debug, PartialEq, Eq)]
enum Prefix {
    Byte(u8),
    Char(char),
}

impl Prefix {
    fn from(slice: &[u8]) -> Option<Self> {
        slice.first().map(|byte| {
            if byte.is_ascii() {
                Self::Byte(*byte)
            } else {
                match slice.chars().next() {
                    Some('\u{FFFD}') | None => Self::Byte(*byte),
                    Some(char) => Self::Char(char),
                }
            }
        })
    }
}

impl Display for Prefix {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Byte(value) => escape_default(*value).fmt(fmt),
            Self::Char(value) => write!(fmt, "{}", *value),
        }
    }
}

#[derive(Debug)]
struct Preview {
    value: String,
    shortened: bool,
}

impl From<&[u8]> for Preview {
    fn from(slice: &[u8]) -> Self {
        let mut value = String::new();
        let mut shortened = false;

        for char in slice.chars() {
            value.push(char);
            if value.len() > 60 {
                value.pop();
                shortened = true;
                break;
            }
        }

        Self { value, shortened }
    }
}

impl Display for Preview {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> std::fmt::Result {
        if self.shortened {
            write!(fmt, "'{}'...", self.value)
        } else {
            write!(fmt, "'{}'", self.value)
        }
    }
}

pub struct Reader<R> {
    inner: LineReader<R>,
    src: Option<Instruction>,
    dst: Option<Instruction>,
    line: usize,
}

impl<R> Reader<R> {
    pub fn new(inner: LineReader<R>) -> Self {
        Self {
            inner,
            src: None,
            dst: None,
            line: 0,
        }
    }
}

impl<R: BufRead> Reader<R> {
    pub fn read(&mut self) -> Result<Option<(&Path, &Path)>, Error> {
        loop {
            self.line += 1;

            let dst = self.dst.get_or_insert_with(Instruction::new);
            dst.set_line(self.line);

            if dst.fetch(&mut self.inner)? == 0 {
                return Ok(None);
            }

            match dst.kind()? {
                InstructionKind::Source => {
                    // Swap src and dst
                    let src = self.src.take();
                    let dst = self.dst.take();
                    src.and_then(|src| self.dst.replace(src));
                    dst.and_then(|dst| self.src.replace(dst));
                }
                InstructionKind::Dest => {
                    return match (&self.src, &self.dst) {
                        (Some(src), Some(dst)) => Ok(Some((src.path()?, dst.path()?))),
                        (None, Some(dst)) => Err(Error {
                            kind: ErrorKind::NoSourcePath,
                            line: self.line,
                            preview: dst.buf().into(),
                        }),
                        _ => unreachable!("Expected dst instruction to be present"),
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Separator;
    use bstr::B;
    use claim::assert_ok_eq;
    use std::path::Path;
    use test_case::test_case;

    #[test_case(B(""),   None                      ; "empty")]
    #[test_case(B("a"),  Some(Prefix::Byte(b'a'))  ; "byte")]
    #[test_case(B("ab"), Some(Prefix::Byte(b'a'))  ; "byte plus")]
    #[test_case(B("á"),  Some(Prefix::Char('á'))   ; "char")]
    #[test_case(B("áb"), Some(Prefix::Char('á'))   ; "char plus")]
    #[test_case(&[195],  Some(Prefix::Byte(195))   ; "invalid unicode")]
    fn prefix_from(input: &[u8], prefix: Option<Prefix>) {
        assert_eq!(Prefix::from(input), prefix)
    }

    #[test_case(Prefix::Byte(b'a'), "a")]
    #[test_case(Prefix::Byte(b'\n'), "\\n")]
    #[test_case(Prefix::Byte(195), "\\xc3")]
    #[test_case(Prefix::Char('á'), "á")]
    fn prefix_display(prefix: Prefix, output: &str) {
        assert_eq!(prefix.to_string(), output);
    }

    #[test_case("<x\n<a\n>bc\n>def", Separator::Newline ; "newline")]
    #[test_case("<x\0<a\0>bc\0>def", Separator::Null    ; "null")]
    fn reader(input: &str, separator: Separator) {
        let line_reader = LineReader::new(input.as_bytes(), separator);
        let mut reader = Reader::new(line_reader);
        assert_ok_eq!(reader.read(), Some((Path::new("a"), Path::new("bc"))));
        assert_ok_eq!(reader.read(), Some((Path::new("a"), Path::new("def"))));
        assert_ok_eq!(reader.read(), None);
    }
}
