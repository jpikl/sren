use crate::io::Reader as LineReader;
use bstr::ByteSlice;
use bstr::B;
use std::ascii::escape_default;
use std::fmt::Display;
use std::fmt::Formatter;
use std::io;
use std::io::BufRead;
use std::path::Path;

const MAX_LINE: usize = 1024 * 1024;

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
#[error("Cannot process input line #{line}: {preview}\n  {kind}")]
struct Error {
    kind: ErrorKind,
    line: usize,
    preview: Preview,
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

    fn kind(&self) -> Result<Kind, Error> {
        match Prefix::from(self.buf()) {
            Some(Prefix::Ascii(b'<')) => Ok(Kind::SourcePath),
            Some(Prefix::Ascii(b'>')) => Ok(Kind::DestPath),
            Some(prefix) => Err(self.error(ErrorKind::InvalidPrefix(prefix))),
            None => Err(self.error(ErrorKind::EmptyLine)),
        }
    }

    fn path(&self) -> Result<&Path, Error> {
        match self.buf.split_first() {
            Some((_, path)) => match path.to_os_str() {
                Ok(value) => Ok(Path::new(value)),
                Err(_) => Err(self.error(ErrorKind::InvalidEncoding)),
            },
            None => Err(self.error(ErrorKind::EmptyPath)),
        }
    }

    fn error(&self, kind: ErrorKind) -> Error {
        Error {
            kind,
            line: self.line,
            preview: Preview::from(self.buf()),
        }
    }
}

enum Kind {
    SourcePath,
    DestPath,
}

#[derive(Debug)]
enum Prefix {
    Ascii(u8),
    Unicode(char),
}

impl Prefix {
    fn from(slice: &[u8]) -> Option<Self> {
        slice.first().map(|byte| {
            if byte.is_ascii() {
                Self::Ascii(*byte)
            } else {
                match slice.chars().next() {
                    Some('\u{FFFD}') | None => Self::Ascii(*byte),
                    Some(char) => Self::Unicode(char),
                }
            }
        })
    }
}

impl Display for Prefix {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ascii(value) => escape_default(*value).fmt(fmt),
            Self::Unicode(value) => write!(fmt, "{}", *value),
        }
    }
}

#[derive(Debug)]
struct Preview {
    value: String,
    shortened: bool,
}

impl Preview {
    fn empty() -> Self {
        Self::from(B(""))
    }
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

struct Reader<R> {
    inner: LineReader<R>,
    src: Option<Instruction>,
    dst: Option<Instruction>,
    line: usize,
}

impl<R> Reader<R> {
    fn new(inner: LineReader<R>) -> Self {
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
                Kind::SourcePath => {
                    // Swap src and dst
                    let src = self.src.take();
                    let dst = self.dst.take();
                    src.and_then(|src| self.dst.replace(src));
                    dst.and_then(|dst| self.src.replace(dst));
                }
                Kind::DestPath => match (&self.src, &self.dst) {
                    (Some(src), Some(dst)) => {
                        return Ok(Some((src.path()?, dst.path()?)));
                    }
                    _ => {
                        return Err(Error {
                            kind: ErrorKind::NoSourcePath,
                            line: self.line,
                            preview: Preview::empty(),
                        });
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::Separator;
    use bstr::B;
    use claim::assert_ok_eq;
    use std::path::Path;

    #[test]
    fn reader() {
        let line_reader = LineReader::new(B("<a\n>b\n>c"), Separator::Newline);
        let mut reader = Reader::new(line_reader);
        assert_ok_eq!(reader.read(), Some((Path::new("a"), Path::new("b"))));
        assert_ok_eq!(reader.read(), Some((Path::new("a"), Path::new("c"))));
        assert_ok_eq!(reader.read(), None);
    }
}
