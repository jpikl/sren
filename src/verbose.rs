use crate::TransferMode;
use once_cell::sync::Lazy;
use std::io::Result;
use std::path::Path;
use termcolor::Color;
use termcolor::ColorSpec;
use termcolor::WriteColor;

static SRC_COLOR: Lazy<ColorSpec> = Lazy::new(|| color_spec(Color::Blue));
static DST_COLOR: Lazy<ColorSpec> = Lazy::new(|| color_spec(Color::Cyan));
static SUCCESS_COLOR: Lazy<ColorSpec> = Lazy::new(|| color_spec(Color::Green));
static FAILURE_COLOR: Lazy<ColorSpec> = Lazy::new(|| color_spec(Color::Red));

fn color_spec(fg: Color) -> ColorSpec {
    let mut spec = ColorSpec::new();
    spec.set_fg(Some(fg));
    spec
}

pub struct Logger<W> {
    writer: W,
}

impl<W> Logger<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: WriteColor> Logger<W> {
    pub fn begin(&mut self, src: &Path, dst: &Path, mode: TransferMode) -> Result<()> {
        let action = match mode {
            TransferMode::Move => "Moving",
            TransferMode::Copy => "Copying",
        };
        write!(self.writer, "{} '", action)?;
        self.writer.set_color(&SRC_COLOR)?;
        write!(self.writer, "{}", src.to_string_lossy())?;
        self.writer.reset()?;
        write!(self.writer, "' to '")?;
        self.writer.set_color(&DST_COLOR)?;
        write!(self.writer, "{}", dst.to_string_lossy())?;
        self.writer.reset()?;
        write!(self.writer, "' ... ")
    }

    pub fn success(&mut self) -> Result<()> {
        self.writer.set_color(&SUCCESS_COLOR)?;
        write!(self.writer, "OK")?;
        self.writer.reset()?;
        writeln!(self.writer)
    }

    pub fn failure(&mut self) -> Result<()> {
        self.writer.set_color(&FAILURE_COLOR)?;
        write!(self.writer, "ERROR")?;
        self.writer.reset()?;
        writeln!(self.writer)
    }
}
