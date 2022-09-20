use bstr::ByteSlice;
use bstr::B;
use std::ascii::escape_default;
use std::ffi::OsString;
use std::fmt::Display;
use std::fmt::Formatter;
use std::io;
use std::path::Path;
use thiserror::Error;

#[derive(Debug)]
enum Prefix {
    Ascii(u8),
    Unicode(char),
}

impl Prefix {
    fn from(slice: &[u8]) -> Option<Self> {
        slice.first().map(|byte| match slice.chars().next() {
            Some('\u{FFFD}') | None => Self::Ascii(*byte),
            Some(char) if char.is_ascii() => Self::Ascii(char as u8),
            Some(char) => Self::Unicode(char),
        })
    }
}

impl Display for Prefix {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ascii(value) => write!(fmt, "{}", escape_default(*value)),
            Self::Unicode(value) => write!(fmt, "{}", value),
        }
    }
}

enum RecordKind {
    SourcePath,
    DestPath,
}

struct Record {
    kind: RecordKind,
    path: OsString,
}

#[derive(Error, Debug)]
enum RecordError {
    #[error("Invalid UTF-8 encoding")]
    InvalidEncoding,
    #[error("Invalid path prefix '{0}'")]
    InvalidPrefix(Prefix),
    #[error("Empty line")]
    EmptyLine,
    #[error("Empty path")]
    EmptyPath,
}

#[derive(Error, Debug)]
enum PathErrorKind {
    #[error("{0}")]
    InvalidRecord(#[from] RecordError),
    #[error("There was no previous source path")]
    NoSourcePath,
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
}

#[derive(Error, Debug)]
#[error("Cannot process input line #{line}: {value}\n  {kind}")]
struct PathError {
    kind: PathErrorKind,
    line: usize,
    value: LinePreview,
}

#[derive(Debug)]
struct LinePreview {
    value: String,
    shortened: bool,
}

impl LinePreview {
    fn empty() -> Self {
        Self::from(B(""))
    }
}

impl From<&[u8]> for LinePreview {
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

impl Display for LinePreview {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> std::fmt::Result {
        if self.shortened {
            write!(fmt, "'{}'...", self.value)
        } else {
            write!(fmt, "'{}'", self.value)
        }
    }
}

impl TryFrom<&[u8]> for Record {
    type Error = RecordError;

    fn try_from(line: &[u8]) -> Result<Self, Self::Error> {
        let kind = match Prefix::from(line) {
            Some(Prefix::Ascii(b'<')) => RecordKind::SourcePath,
            Some(Prefix::Ascii(b'>')) => RecordKind::DestPath,
            Some(prefix) => return Err(RecordError::InvalidPrefix(prefix)),
            None => return Err(RecordError::EmptyLine),
        };
        // The only valid prefixes have always 1 byte length
        let (_, path) = line.split_at(1);
        if path.is_empty() {
            return Err(RecordError::EmptyPath);
        }
        match path.to_os_str() {
            Ok(os_str) => Ok(Self {
                kind,
                path: (*os_str).to_owned(),
            }),
            Err(_) => Err(RecordError::InvalidEncoding),
        }
    }
}

struct PathReader<I> {
    src_path: Option<OsString>,
    dst_path: Option<OsString>,
    iterator: I,
    line: usize,
}

// TODO: use sepio crate
impl<I> PathReader<I> {
    fn new(iterator: I) -> Self {
        Self {
            src_path: None,
            dst_path: None,
            iterator,
            line: 0,
        }
    }
}

impl<I: Iterator<Item = io::Result<Vec<u8>>>> PathReader<I> {
    pub fn read(&mut self) -> Result<Option<(&Path, &Path)>, PathError> {
        loop {
            match self.fetch()? {
                Some(RecordKind::DestPath) => {
                    let src_path = Path::new(
                        self.src_path
                            .as_ref()
                            .expect("Expected PathReader::src_path to be present"),
                    );
                    let dst_path = Path::new(
                        self.dst_path
                            .as_ref()
                            .expect("Expected PathReader::dst_path to be present"),
                    );
                    return Ok(Some((src_path, dst_path)));
                }
                Some(RecordKind::SourcePath) => {}
                None => return Ok(None),
            }
        }
    }

    fn fetch(&mut self) -> Result<Option<RecordKind>, PathError> {
        self.line += 1;
        match self.iterator.next() {
            Some(Ok(line)) => {
                self.process_line(line.as_ref())
                    .map(Some)
                    .map_err(|kind| PathError {
                        line: self.line,
                        kind,
                        value: LinePreview::from(line.as_ref()),
                    })
            }
            Some(Err(error)) => Err(PathError {
                line: self.line,
                kind: PathErrorKind::IoError(error),
                value: LinePreview::empty(),
            }),
            None => Ok(None),
        }
    }

    fn process_line(&mut self, line: &[u8]) -> Result<RecordKind, PathErrorKind> {
        let record = Record::try_from(line)?;
        match record.kind {
            RecordKind::SourcePath => {
                self.src_path.replace(record.path);
            }
            RecordKind::DestPath => {
                self.dst_path.replace(record.path);
                if self.src_path.is_none() {
                    return Err(PathErrorKind::NoSourcePath);
                }
            }
        }
        Ok(record.kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bstr::io::BufReadExt;
    use bstr::B;
    use claim::assert_ok_eq;
    use std::path::Path;

    #[test]
    fn path_reader() {
        let mut reader = PathReader::new(B("<a\n>b\n>c").byte_lines());
        assert_ok_eq!(reader.read(), Some((Path::new("a"), Path::new("b"))));
        assert_ok_eq!(reader.read(), Some((Path::new("a"), Path::new("c"))));
        assert_ok_eq!(reader.read(), None);
    }
}
