use crate::line::LineReader;
use bstr::ByteSlice;
use std::ffi::OsString;
use std::io;
use std::io::BufRead;
use std::path::Path;

const MAX_LINE: usize = 1024 * 1024;
const MAX_PREVIEW: usize = 30;

#[derive(Copy, Clone)]
enum PathKind {
    Source,
    Dest,
}

fn parse_line(line: &[u8]) -> Result<(PathKind, OsString), ErrorCause> {
    if let Some((first, tail)) = line.split_first() {
        let kind = match first {
            b'<' => PathKind::Source,
            b'>' => PathKind::Dest,
            _ => return Err(ErrorCause::InvalidPrefix),
        };
        if tail.is_empty() {
            return Err(ErrorCause::EmptyPath);
        }
        match tail.to_os_str() {
            Ok(value) => Ok((kind, value.to_owned())),
            Err(_) => Err(ErrorCause::InvalidEncoding),
        }
    } else {
        Err(ErrorCause::EmptyLine)
    }
}

fn preview_line(line: &[u8]) -> String {
    let mut preview = String::new();

    for char in line.chars() {
        preview.push(char);
        if preview.len() > MAX_PREVIEW {
            preview.pop();
            preview.push_str("...");
            break;
        }
    }

    preview
}

#[derive(Debug, thiserror::Error)]
enum ErrorCause {
    #[error("Invalid UTF-8 encoding")]
    InvalidEncoding,
    #[error("Invalid line prefix, expected '<' or '>'")]
    InvalidPrefix,
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
#[error("Failed to process line #{line}: {preview}\nCause: {cause}")]
pub struct Error {
    cause: ErrorCause,
    line: usize,
    preview: String,
}

pub struct PathReader<R> {
    inner: LineReader<R>,
    src: Option<OsString>,
    dst: Option<OsString>,
    line: usize,
}

impl<R> PathReader<R> {
    pub fn new(inner: LineReader<R>) -> Self {
        Self {
            inner,
            src: None,
            dst: None,
            line: 0,
        }
    }
}

impl<R: BufRead> PathReader<R> {
    pub fn read(&mut self) -> Result<Option<(&Path, &Path)>, Error> {
        loop {
            self.line += 1;

            let buffer = match self.inner.read(MAX_LINE) {
                Ok(Some(line)) => line,
                Ok(None) => return Ok(None),
                Err(error) => {
                    return Err(Error {
                        cause: ErrorCause::IoError(error),
                        line: self.line,
                        preview: String::new(),
                    });
                }
            };

            if buffer.len() >= MAX_LINE {
                return Err(Error {
                    cause: ErrorCause::LineOverflow,
                    line: self.line,
                    preview: preview_line(buffer),
                });
            }

            match parse_line(buffer) {
                Ok((PathKind::Source, path)) => {
                    self.src.replace(path);
                    continue; // Wait for the next dst path
                }
                Ok((PathKind::Dest, path)) => {
                    self.dst.replace(path);
                }
                Err(cause) => {
                    return Err(Error {
                        cause,
                        line: self.line,
                        preview: preview_line(buffer),
                    })
                }
            }

            match (&self.src, &self.dst) {
                (Some(src), Some(dst)) => {
                    return Ok(Some((Path::new(src), Path::new(dst))));
                }
                (None, Some(_)) => {
                    return Err(Error {
                        cause: ErrorCause::NoSourcePath,
                        line: self.line,
                        preview: preview_line(buffer),
                    })
                }
                _ => unreachable!("Expected dst instruction to be present"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line::Separator;
    use claim::assert_ok_eq;
    use std::path::Path;
    use test_case::test_case;

    #[test_case("<x\n<a\n>bc\n>def", Separator::Newline ; "newline")]
    #[test_case("<x\0<a\0>bc\0>def", Separator::Null    ; "null")]
    fn reader(input: &str, separator: Separator) {
        let line_reader = LineReader::new(input.as_bytes(), separator);
        let mut reader = PathReader::new(line_reader);
        assert_ok_eq!(reader.read(), Some((Path::new("a"), Path::new("bc"))));
        assert_ok_eq!(reader.read(), Some((Path::new("a"), Path::new("def"))));
        assert_ok_eq!(reader.read(), None);
    }
}
